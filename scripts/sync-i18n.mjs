#!/usr/bin/env node
/**
 * Syncs shared/i18n/{en,fa}.json (+ React runtime) into every web frontend:
 *   web-ui, mobile-pwa, desktop-shell/ui, admin-shell/ui
 *
 * Generates src/i18n/dictionaries.generated.ts from the canonical catalogs and
 * locale registry so no tsconfig change (resolveJsonModule) is needed anywhere.
 *
 * Validation performed before any write:
 *   locale-registry shape, translation-schema conformance, EN/FA key parity,
 *   interpolation-placeholder parity and brace syntax, canonical key ordering,
 *   unknown namespaces, pluralization structure, terminology-model agreement,
 *   and unresolved literal translation keys.
 *
 * Usage: node scripts/sync-i18n.mjs [--check] [--audit-scope=<path>] [--sort-catalogs]
 */
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, existsSync, readdirSync } from "node:fs";
import { join, dirname, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const check = process.argv.includes("--check");
const auditArg = process.argv.find((arg) => arg.startsWith("--audit-scope="));
const auditScope = auditArg ? auditArg.slice("--audit-scope=".length) : null;
const sortCatalogs = process.argv.includes("--sort-catalogs");
const failures = [];

function readJsonDocument(relativePath) {
  try {
    return JSON.parse(readFileSync(join(root, relativePath), "utf8"));
  } catch (error) {
    console.error(`i18n JSON error in ${relativePath}: ${error.message}`);
    process.exit(1);
  }
}

function reportFailure(message) {
  failures.push(message);
  console.error(message);
}

const en = readJsonDocument("shared/i18n/en.json");
const fa = readJsonDocument("shared/i18n/fa.json");
const locales = readJsonDocument("shared/i18n/metadata/locales.json");
const translationSchema = readJsonDocument("shared/i18n/schema/translation.schema.json");

function validateLocaleRegistry(registry) {
  if (typeof registry !== "object" || registry === null || Array.isArray(registry)) {
    reportFailure("i18n locale registry error: shared/i18n/metadata/locales.json must be an object.");
    return [];
  }
  const localeIds = Object.keys(registry);
  if (localeIds.length === 0) {
    reportFailure("i18n locale registry error: at least one locale is required.");
  }
  const defaults = localeIds.filter((locale) => registry[locale]?.default === true);
  if (defaults.length !== 1) {
    reportFailure(`i18n locale registry error: exactly one locale must have "default": true (found ${defaults.length}).`);
  }
  for (const locale of localeIds) {
    const metadata = registry[locale];
    if (!/^[a-z]{2,3}(?:-[A-Za-z]{2,8})?$/.test(locale)) {
      reportFailure(`i18n locale registry error: unsupported locale identifier "${locale}".`);
    }
    if (typeof metadata !== "object" || metadata === null || Array.isArray(metadata)) {
      reportFailure(`i18n locale registry error: metadata for "${locale}" must be an object.`);
      continue;
    }
    if (metadata.direction !== "ltr" && metadata.direction !== "rtl") {
      reportFailure(`i18n locale registry error: "${locale}" must set direction to "ltr" or "rtl".`);
    }
    if (typeof metadata.displayName !== "string" || metadata.displayName.length === 0) {
      reportFailure(`i18n locale registry error: "${locale}" must have a non-empty displayName.`);
    }
    try {
      Intl.getCanonicalLocales(metadata.formatLocale);
    } catch {
      reportFailure(`i18n locale registry error: "${locale}" must have a valid BCP 47 formatLocale.`);
    }
  }
  return localeIds;
}

function validateCatalogAgainstSchema(catalogName, catalog, schema) {
  if (typeof catalog !== "object" || catalog === null || Array.isArray(catalog)) {
    reportFailure(`i18n schema error: ${catalogName} must be a JSON object.`);
    return;
  }
  const propertyPatterns = schema?.patternProperties;
  if (
    schema?.type !== "object" ||
    typeof schema?.propertyNames?.pattern !== "string" ||
    typeof propertyPatterns !== "object" ||
    propertyPatterns === null ||
    schema?.additionalProperties !== false
  ) {
    reportFailure("i18n schema error: the validator supports this schema's object/propertyNames/patternProperties shape only.");
    return;
  }
  let namePattern;
  try {
    namePattern = new RegExp(schema.propertyNames.pattern);
  } catch {
    reportFailure("i18n schema error: propertyNames.pattern is not a valid regular expression.");
    return;
  }
  const patternEntries = Object.entries(propertyPatterns).map(([pattern, valueSchema]) => {
    try {
      return [new RegExp(pattern), valueSchema];
    } catch {
      reportFailure(`i18n schema error: invalid pattern property "${pattern}".`);
      return null;
    }
  }).filter(Boolean);
  const explicitEntries = Object.entries(schema.properties ?? {});

  for (const [key, value] of Object.entries(catalog)) {
    if (!namePattern.test(key) && !Object.prototype.hasOwnProperty.call(schema.properties ?? {}, key)) {
      reportFailure(`i18n schema error: ${catalogName} has an invalid key "${key}".`);
      continue;
    }
    const applicable = [
      ...explicitEntries.filter(([property]) => property === key).map(([, valueSchema]) => valueSchema),
      ...patternEntries.filter(([pattern]) => pattern.test(key)).map(([, valueSchema]) => valueSchema),
    ];
    if (applicable.length === 0) {
      reportFailure(`i18n schema error: ${catalogName} has an additional key "${key}".`);
      continue;
    }
    const valid = applicable.every((valueSchema) => {
      if (typeof valueSchema !== "object" || valueSchema === null) return false;
      if (valueSchema.type !== "string" || typeof value !== "string") return false;
      return !(typeof valueSchema.minLength === "number" && value.length < valueSchema.minLength);
    });
    if (!valid) {
      reportFailure(`i18n schema error: ${catalogName}["${key}"] must be a non-empty string.`);
    }
  }
}


const localeIds = validateLocaleRegistry(locales);
validateCatalogAgainstSchema("shared/i18n/en.json", en, translationSchema);
validateCatalogAgainstSchema("shared/i18n/fa.json", fa, translationSchema);

function catalogKeys(catalog) {
  return typeof catalog === "object" && catalog !== null && !Array.isArray(catalog)
    ? Object.keys(catalog).sort()
    : [];
}

const enKeys = catalogKeys(en);
const faKeys = catalogKeys(fa);
const missingInFa = enKeys.filter((k) => !(k in fa));
const missingInEn = faKeys.filter((k) => !(k in en));
if (missingInFa.length > 0 || missingInEn.length > 0) {
  reportFailure("i18n key mismatch:");
  if (missingInFa.length > 0) reportFailure(`  missing in fa.json: ${missingInFa.join(", ")}`);
  if (missingInEn.length > 0) reportFailure(`  missing in en.json: ${missingInEn.join(", ")}`);
}

function interpolationPlaceholders(text) {
  const placeholders = new Set();
  if (typeof text !== "string") return [];
  for (const match of text.matchAll(/\{([A-Za-z_][A-Za-z0-9_]*)\}/g)) {
    placeholders.add(match[1]);
  }
  return [...placeholders].sort();
}

for (const key of enKeys) {
  if (!(key in fa)) continue;
  const enPlaceholders = interpolationPlaceholders(en[key]);
  const faPlaceholders = interpolationPlaceholders(fa[key]);
  if (enPlaceholders.join("\0") !== faPlaceholders.join("\0")) {
    reportFailure(
      `i18n interpolation mismatch for "${key}": en {${enPlaceholders.join(", ")}} != fa {${faPlaceholders.join(", ")}}`,
    );
  }
}

// Phase 2 (P2-5): placeholder parity alone cannot see a placeholder that is
// malformed in *both* locales, so brace shape is validated independently.
function validatePlaceholderSyntax(catalogName, catalog) {
  for (const [key, value] of Object.entries(catalog)) {
    if (typeof value !== "string") continue;
    const malformed = [];
    for (const match of value.matchAll(/\{([^{}]*)\}/g)) {
      if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(match[1])) {
        malformed.push(`{${match[1]}}`);
      }
    }
    const withoutPlaceholders = value.replace(/\{[^{}]*\}/g, "");
    if (withoutPlaceholders.includes("{") || withoutPlaceholders.includes("}")) {
      malformed.push("unbalanced brace");
    }
    if (malformed.length > 0) {
      reportFailure(
        `i18n malformed interpolation in ${catalogName}["${key}"]: ${malformed.join(", ")} (placeholders must be {camelCaseName})`,
      );
    }
  }
}

validatePlaceholderSyntax("shared/i18n/en.json", en);
validatePlaceholderSyntax("shared/i18n/fa.json", fa);

// Phase 2 (P2-13): canonical, editor-independent ordering. Namespaces follow
// the ratified order; leaves sort lexicographically inside their namespace.
const CANONICAL_NAMESPACE_ORDER = [
  "app",
  "nav",
  "auth",
  "common",
  "status",
  "dashboard",
  "missions",
  "tasks",
  "notifications",
  "approvals",
  "reports",
  "files",
  "settings",
  "language",
];

function canonicalSortKey(key) {
  const namespace = key.slice(0, key.indexOf("."));
  const rank = CANONICAL_NAMESPACE_ORDER.indexOf(namespace);
  return `${String(rank === -1 ? CANONICAL_NAMESPACE_ORDER.length : rank).padStart(3, "0")}|${key}`;
}

// Deliberately not localeCompare: catalog ordering must be identical on every
// machine regardless of the runner's ICU build.
function compareCanonicalKeys(a, b) {
  const left = canonicalSortKey(a);
  const right = canonicalSortKey(b);
  if (left < right) return -1;
  if (left > right) return 1;
  return 0;
}

function validateCanonicalOrdering(catalogName, catalog) {
  // Object.keys preserves the order the keys appear in the file, which is what
  // deterministic ordering means here. catalogKeys() would pre-sort and hide
  // exactly the defect this check exists to find.
  const actual = typeof catalog === "object" && catalog !== null && !Array.isArray(catalog)
    ? Object.keys(catalog)
    : [];
  if (actual.length === 0) return;
  const expected = [...actual].sort(compareCanonicalKeys);
  if (actual.join("\n") !== expected.join("\n")) {
    // A --sort-catalogs run is the sanctioned way to fix this, so the failure
    // is reported as a note and normalized instead of blocking the run.
    if (sortCatalogs) {
      console.log(`i18n ordering note in ${catalogName}: rewriting in canonical order.`);
      return;
    }
    const firstDivergence = expected.findIndex((key, index) => actual[index] !== key);
    reportFailure(
      `i18n ordering error in ${catalogName}: keys are not in canonical order (namespace order ${CANONICAL_NAMESPACE_ORDER.join(", ")}, then leaf). First divergence at "${expected[firstDivergence] ?? "?"}". Run: node scripts/sync-i18n.mjs --sort-catalogs`,
    );
  }
}

const unknownNamespaces = new Set();
for (const key of [...enKeys, ...faKeys]) {
  const namespace = key.slice(0, key.indexOf("."));
  if (!CANONICAL_NAMESPACE_ORDER.includes(namespace)) unknownNamespaces.add(namespace);
}
if (unknownNamespaces.size > 0) {
  reportFailure(
    `i18n unknown namespace(s): ${[...unknownNamespaces].sort().join(", ")}. Add the namespace to CANONICAL_NAMESPACE_ORDER and the schema before using it.`,
  );
}

validateCanonicalOrdering("shared/i18n/en.json", en);
validateCanonicalOrdering("shared/i18n/fa.json", fa);

// Phase 2 (P2-10): quantities must stay compatible with the Phase 1
// pluralization API rather than encoding singular/plural inside the string.
const FORBIDDEN_PLURAL_PATTERNS = [/\(\s*s\s*\)/i, /\(\s*es\s*\)/i, /\{\s*count\s*\}s\b/i, /\bif count\b/i];

function validatePluralizationStructure(catalogName, catalog) {
  for (const [key, value] of Object.entries(catalog)) {
    if (typeof value !== "string") continue;
    for (const pattern of FORBIDDEN_PLURAL_PATTERNS) {
      if (pattern.test(value)) {
        reportFailure(
          `i18n pluralization error in ${catalogName}["${key}"]: hard-coded plural form "${value}". Use an invariant noun phrase and let pluralCategory/selectPluralForm select the form.`,
        );
        break;
      }
    }
  }
}

validatePluralizationStructure("shared/i18n/en.json", en);
validatePluralizationStructure("shared/i18n/fa.json", fa);

// Phase 2 (P2-9): the terminology model is small on purpose. It is checked for
// internal consistency and for agreement with the catalogs, never used to
// rewrite values.
function validateTerminology(model) {
  if (typeof model !== "object" || model === null || Array.isArray(model)) {
    reportFailure("i18n terminology error: shared/i18n/metadata/terminology.json must be an object.");
    return;
  }
  const concepts = model.concepts;
  if (!Array.isArray(concepts) || concepts.length === 0) {
    reportFailure("i18n terminology error: terminology.json must declare a non-empty concepts array.");
    return;
  }
  const ids = new Set();
  const catalogs = { en, fa };
  for (const concept of concepts) {
    if (typeof concept !== "object" || concept === null) {
      reportFailure("i18n terminology error: every terminology concept must be an object.");
      continue;
    }
    if (typeof concept.id !== "string" || concept.id.length === 0) {
      reportFailure("i18n terminology error: every terminology concept needs a non-empty id.");
      continue;
    }
    if (ids.has(concept.id)) {
      reportFailure(`i18n terminology error: duplicate concept id "${concept.id}".`);
      continue;
    }
    ids.add(concept.id);
    for (const locale of Object.keys(catalogs)) {
      const term = concept[locale];
      if (typeof term !== "string" || term.trim().length === 0) {
        reportFailure(`i18n terminology error: concept "${concept.id}" needs a non-empty ${locale} term.`);
        continue;
      }
      const present = Object.values(catalogs[locale]).some((value) =>
        typeof value === "string" && value.toLocaleLowerCase().includes(term.trim().toLocaleLowerCase()),
      );
      if (!present) {
        reportFailure(
          `i18n terminology error: the ${locale} term "${term}" for concept "${concept.id}" does not appear in shared/i18n/${locale}.json.`,
        );
      }
    }
  }
}

validateTerminology(readJsonDocument("shared/i18n/metadata/terminology.json"));


const translationCallSurfaces = [
  "web-ui/src",
  "mobile-pwa/src",
  "crates/bins/desktop-shell/ui/src",
  "crates/bins/admin-shell/ui/src",
];
const translatableSourceExtensions = new Set([".ts", ".tsx"]);
const excludedSourceDirectories = new Set([
  "node_modules",
  "dist",
  "build",
  "coverage",
  "tests",
  "__tests__",
  "__mocks__",
  ".git",
]);

function stripSourceComments(source) {
  let output = "";
  let index = 0;
  let quote = null;
  while (index < source.length) {
    const character = source[index];
    const next = source[index + 1] ?? "";
    if (quote !== null) {
      output += character;
      if (character === "\\" && next !== "") {
        output += next;
        index += 2;
        continue;
      }
      if (character === quote) quote = null;
      index += 1;
      continue;
    }
    if (character === "/" && next === "/") {
      while (index < source.length && source[index] !== "\n") index += 1;
      continue;
    }
    if (character === "/" && next === "*") {
      index += 2;
      while (index < source.length && !(source[index] === "*" && source[index + 1] === "/")) index += 1;
      index += 2;
      continue;
    }
    if (character === "'" || character === '"' || character === "`") quote = character;
    output += character;
    index += 1;
  }
  return output;
}

function unescapeStringLiteral(raw, quote) {
  return raw.replace(
    /\\(?:u\{([0-9a-fA-F]+)\}|u([0-9a-fA-F]{4})|x([0-9a-fA-F]{2})|(.))/g,
    (match, braced, short, hex, character) => {
      if (braced !== undefined) return String.fromCodePoint(Number.parseInt(braced, 16));
      if (short !== undefined) return String.fromCharCode(Number.parseInt(short, 16));
      if (hex !== undefined) return String.fromCharCode(Number.parseInt(hex, 16));
      switch (character) {
        case "n": return "\n";
        case "r": return "\r";
        case "t": return "\t";
        case "b": return "\b";
        case "f": return "\f";
        case "v": return "\v";
        case "0": return "\0";
        case "\n": return "";
        case "\r": return "";
        default: return character === quote ? quote : character;
      }
    },
  );
}

function translationCallees(source) {
  const callees = new Set();
  for (const match of source.matchAll(/\bconst\s*\{([^}]*)\}\s*=\s*useI18n\s*\(\s*\)/g)) {
    for (const part of match[1].split(",")) {
      const [original, alias] = part.split(":").map((value) => value.trim()).filter(Boolean);
      if (original === "t") callees.add(alias ?? original);
    }
  }
  for (const match of source.matchAll(/\bimport\s*\{([^}]*)\}\s*from\s*["'][^"']*i18n\/I18nContext["']/g)) {
    for (const part of match[1].split(",")) {
      const [imported, local] = part.split(":").map((value) => value.trim()).filter(Boolean);
      if (imported === "t" || imported === "translateStatic") callees.add(local ?? imported);
    }
  }
  if (/\buseI18n\s*\(\s*\)\s*\.\s*t\s*\(/.test(source)) callees.add("useI18n\\(\\)\\.t");
  return callees;
}

function findLiteralTranslationKeys(source, callees) {
  if (callees.size === 0) return [];
  const calleePattern = [...callees].sort((a, b) => b.length - a.length).join("|");
  // Only static string literals are checked. Template literals containing
  // ${...} and variable-derived keys are intentionally ignored because they
  // cannot be resolved without executing the application.
  const pattern = new RegExp(`(${calleePattern})\\s*\\(\\s*(['"\`])((?:\\\\.|(?!\\2)[^\\\\])*)\\2`, "g");
  const found = [];
  for (const match of source.matchAll(pattern)) {
    const quote = match[2];
    const raw = match[3];
    if (quote === "`" && raw.includes("${")) continue;
    found.push({ callee: match[1], key: unescapeStringLiteral(raw, quote) });
  }
  return found;
}

function isExcludedTranslationSource(relativePath) {
  const segments = relativePath.split("/");
  const fileName = segments[segments.length - 1] ?? "";
  if (segments.some((segment) => excludedSourceDirectories.has(segment))) return true;
  // Generated runtime copies are byte-verified separately; this scan targets
  // application call sites.
  if (segments[2] === "i18n") return true;
  return /(^|\.)(test|spec)\.[cm]?[jt]sx?$/.test(fileName);
}

function scanUnresolvedLiteralKeys() {
  const unresolved = [];
  let scannedFiles = 0;
  let literalKeys = 0;
  for (const surface of translationCallSurfaces) {
    const surfaceRoot = join(root, surface);
    const stack = [surfaceRoot];
    while (stack.length > 0) {
      const directory = stack.pop();
      if (!existsSync(directory)) continue;
      for (const entry of readdirSync(directory, { withFileTypes: true })) {
        const fullPath = join(directory, entry.name);
        if (entry.isDirectory()) {
          stack.push(fullPath);
          continue;
        }
        const extension = entry.name.slice(entry.name.lastIndexOf("."));
        if (!translatableSourceExtensions.has(extension)) continue;
        const relativePath = fullPath.slice(root.length + 1).split(sep).join("/");
        if (isExcludedTranslationSource(relativePath)) continue;
        scannedFiles += 1;
        const source = stripSourceComments(readFileSync(fullPath, "utf8"));
        for (const { callee, key } of findLiteralTranslationKeys(source, translationCallees(source))) {
          literalKeys += 1;
          if (!Object.prototype.hasOwnProperty.call(en, key)) {
            unresolved.push({ file: relativePath, callee, key });
          }
        }
      }
    }
  }
  unresolved.sort((a, b) => a.file.localeCompare(b.file) || a.key.localeCompare(b.key));
  if (unresolved.length > 0) {
    reportFailure("i18n unresolved translation keys:");
    for (const { file, callee, key } of unresolved.slice(0, 50)) {
      reportFailure(`  ${file}: ${callee}("${key}") is absent from shared/i18n/en.json`);
    }
    if (unresolved.length > 50) {
      reportFailure(`  ... and ${unresolved.length - 50} more unresolved keys`);
    }
  } else {
    console.log(`i18n unresolved-key scan passed: ${literalKeys} literal keys in ${scannedFiles} source files.`);
  }
}

scanUnresolvedLiteralKeys();

function buildGeneratedDictionary() {
  const defaultLocaleId = localeIds.find((locale) => locales[locale]?.default === true);
  if (defaultLocaleId === undefined) {
    throw new Error("i18n locale registry must designate exactly one default locale.");
  }
  return `// AUTO-GENERATED by scripts/sync-i18n.mjs — do not edit by hand.
// Source: shared/i18n/{en,fa}.json and shared/i18n/metadata/locales.json
export const localeIds = ${JSON.stringify(localeIds)} as const;
export type Locale = (typeof localeIds)[number];
export interface LocaleMetadata {
  direction: "ltr" | "rtl";
  default: boolean;
  displayName: string;
  formatLocale: string;
}
export const localeMetadata: Record<Locale, LocaleMetadata> = ${JSON.stringify(locales, null, 2)} as Record<Locale, LocaleMetadata>;
export const defaultLocale: Locale = "${defaultLocaleId}";
export const en: Record<string, string> = ${JSON.stringify(en, null, 2)} as Record<string, string>;
export const fa: Record<string, string> = ${JSON.stringify(fa, null, 2)} as Record<string, string>;
export const dictionaries = { en, fa } as const;
`;
}

function auditUserFacingLiterals(scope) {
  const extensions = new Set([".ts", ".tsx"]);
  const ignored = new Set(["node_modules", "dist", "build", ".git"]);
  const violations = [];

  function walk(dir) {
    if (!existsSync(dir)) return;
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (ignored.has(entry.name)) continue;
      const full = join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (extensions.has(entry.name.slice(entry.name.lastIndexOf(".")))) {
        const source = readFileSync(full, "utf8");
        const jsxText = />\s*([A-Za-z][^<>{}\n]{2,})\s*</g;
        for (const match of source.matchAll(jsxText)) {
          const value = match[1].trim();
          if (!value || value.startsWith("//")) continue;
          violations.push(`${full}: ${value}`);
        }
      }
    }
  }

  walk(join(root, scope));
  if (violations.length) {
    reportFailure("i18n literal audit failed. User-facing JSX text must use t(...) keys:");
    for (const violation of violations) reportFailure(`  ${violation}`);
    return;
  }
  console.log(`i18n literal audit passed: ${scope}`);
}

if (auditScope) auditUserFacingLiterals(auditScope);

if (sortCatalogs) {
  // Phase 2 (P2-13): rewrite the canonical catalogs in canonical order instead
  // of relying on a human to hand-sort 150 lines. Values are never modified.
  for (const [relativePath, catalog] of [
    ["shared/i18n/en.json", en],
    ["shared/i18n/fa.json", fa],
  ]) {
    const target = join(root, relativePath);
    const sorted = {};
    for (const key of Object.keys(catalog).sort(compareCanonicalKeys)) {
      sorted[key] = catalog[key];
    }
    const serialized = `${JSON.stringify(sorted, null, 2)}\n`;
    if (readFileSync(target, "utf8") !== serialized) {
      writeFileSync(target, serialized, "utf8");
      console.log(`sorted ${relativePath} (${Object.keys(sorted).length} keys)`);
    }
  }
}

let generated;
try {
  generated = buildGeneratedDictionary();
} catch (error) {
  reportFailure(`i18n generation error: ${error.message}`);
}
if (failures.length > 0) {
  reportFailure(`i18n validation failed with ${failures.length} error(s).`);
  process.exit(1);
}
console.log(
  `i18n validation passed: schema, EN/FA parity, interpolation parity, canonical ordering, pluralization structure, terminology, and unresolved-key scan (${enKeys.length} keys).`,
);

const targets = [
  "web-ui/src/i18n",
  "mobile-pwa/src/i18n",
  "crates/bins/desktop-shell/ui/src/i18n",
  "crates/bins/admin-shell/ui/src/i18n",
];

const staticFiles = ["I18nContext.tsx", "LanguageSwitcher.tsx", "formatting.ts", "plural.ts", "rtl.css"];

let changed = 0;
for (const rel of targets) {
  const dir = join(root, rel);
  mkdirSync(dir, { recursive: true });
  const genPath = join(dir, "dictionaries.generated.ts");
  const prev = existsSync(genPath) ? readFileSync(genPath, "utf8") : null;
  if (prev !== generated) {
    if (check) {
      reportFailure(`OUT OF SYNC: ${rel}/dictionaries.generated.ts (run: node scripts/sync-i18n.mjs)`);
      changed++;
    } else {
      writeFileSync(genPath, generated);
      console.log(`wrote ${rel}/dictionaries.generated.ts (${enKeys.length} keys)`);
    }
  }
  for (const f of staticFiles) {
    const src = join(root, "shared/i18n/react", f);
    const dst = join(dir, f);
    const srcText = readFileSync(src, "utf8");
    const prevText = existsSync(dst) ? readFileSync(dst, "utf8") : null;
    if (prevText !== srcText) {
      if (check) {
        reportFailure(`OUT OF SYNC: ${rel}/${f}`);
        changed++;
      } else {
        copyFileSync(src, dst);
        console.log(`copied ${rel}/${f}`);
      }
    }
  }
}

if (check && changed > 0) {
  reportFailure(`i18n generated resources are out of sync in ${changed} file(s).`);
  process.exit(1);
}
console.log(check ? "i18n in sync." : "i18n sync complete.");
