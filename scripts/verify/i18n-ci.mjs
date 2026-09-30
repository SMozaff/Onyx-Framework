#!/usr/bin/env node
/**
 * Phase 3 localization CI gate.
 *
 * This is deliberately separate from the catalog generator: it audits
 * user-facing source debt and maintains a shrink-only baseline while the
 * application surfaces are migrated in later phases.
 *
 * Usage:
 *   node scripts/verify/i18n-ci.mjs
 *   node scripts/verify/i18n-ci.mjs --report=artifacts/i18n-report.json
 *   node scripts/verify/i18n-ci.mjs --update-baseline
 */
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync, mkdirSync, existsSync, readdirSync } from "node:fs";
import { join, dirname, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const baselinePath = join(root, "i18n-baseline.json");
const updateBaseline = process.argv.includes("--update-baseline");
const reportArg = process.argv.find((arg) => arg.startsWith("--report="));
const reportPath = reportArg ? join(root, reportArg.slice("--report=".length)) : null;

const surfaces = [
  "web-ui/src",
  "mobile-pwa/src",
  "crates/bins/desktop-shell/ui/src",
  "crates/bins/admin-shell/ui/src",
];

const sourceExtensions = new Set([".ts", ".tsx"]);
const ignoredDirectories = new Set(["node_modules", "dist", "build", "coverage", ".git", "i18n"]);
const ignoredFiles = /(^|\.)(test|spec)\.[cm]?[jt]sx?$/;

function walk(relativeRoot) {
  const output = [];
  const absoluteRoot = join(root, relativeRoot);
  const stack = [absoluteRoot];
  while (stack.length) {
    const dir = stack.pop();
    if (!existsSync(dir)) continue;
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (ignoredDirectories.has(entry.name)) continue;
      const full = join(dir, entry.name);
      if (entry.isDirectory()) stack.push(full);
      else if (sourceExtensions.has(entry.name.slice(entry.name.lastIndexOf("."))) && !ignoredFiles.test(entry.name)) {
        output.push(full);
      }
    }
  }
  return output.sort();
}

function stripComments(source) {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/(^|\s)\/\/.*$/gm, "$1");
}

function unescape(raw, quote) {
  return raw.replace(/\\(?:u\{([0-9a-fA-F]+)\}|u([0-9a-fA-F]{4})|x([0-9a-fA-F]{2})|(.))/g,
    (match, braced, short, hex, character) => {
      if (braced !== undefined) return String.fromCodePoint(Number.parseInt(braced, 16));
      if (short !== undefined) return String.fromCharCode(Number.parseInt(short, 16));
      if (hex !== undefined) return String.fromCharCode(Number.parseInt(hex, 16));
      const map = { n: "\n", r: "\r", t: "\t", b: "\b", f: "\f", v: "\v", "0": "\0" };
      return map[character] ?? (character === quote ? quote : character);
    });
}

function addViolation(list, kind, file, value, detail) {
  const normalized = value.replace(/\s+/g, " ").trim();
  if (normalized.length < 3) return;
  const signature = [kind, file, normalized].join("\0");
  const fingerprint = createHash("sha256").update(signature).digest("hex");
  list.push({ fingerprint, kind, file, value: normalized, detail });
}

function auditFile(file) {
  const relative = file.slice(root.length + 1).split(sep).join("/");
  const source = stripComments(readFileSync(file, "utf8"));
  const violations = [];

  // JSX text nodes.
  for (const match of source.matchAll(/>\s*([A-Za-z][^<>{}\n]{2,})\s*</g)) {
    addViolation(violations, "jsx-text", relative, match[1], "literal JSX text");
  }

  // User-facing JSX attributes. Keep this intentionally conservative.
  for (const match of source.matchAll(/\b(placeholder|aria-label|title|alt|label|helperText)\s*=\s*(["'])([\s\S]*?)\2/g)) {
    const value = match[3];
    if (!value.includes("{")) {
      addViolation(violations, "jsx-attribute", relative, value, match[1]);
    }
  }

  // Common presentation/error notification APIs. This deliberately does not
  // flag generic Error() or console output because those can be diagnostics.
  const callPattern = /\b(toast\.(?:success|error|warning|info)|showToast|notify|setError|setSuccess|setWarning|setInfo|window\.(?:alert|confirm|prompt))\s*\(\s*(["'])([\s\S]*?)\2/g;
  for (const match of source.matchAll(callPattern)) {
    addViolation(violations, "presentation-call", relative, unescape(match[3], match[2]), match[1]);
  }

  return violations;
}

function readBaseline() {
  if (!existsSync(baselinePath)) return new Set();
  const parsed = JSON.parse(readFileSync(baselinePath, "utf8"));
  if (!Array.isArray(parsed.entries)) throw new Error("i18n-baseline.json must contain an entries array.");
  return new Set(parsed.entries.map((entry) => entry.fingerprint));
}

const violations = surfaces.flatMap((surface) => walk(surface).flatMap(auditFile));
violations.sort((a, b) => a.file.localeCompare(b.file) || a.kind.localeCompare(b.kind) || a.value.localeCompare(b.value));

const current = new Map(violations.map((entry) => [entry.fingerprint, entry]));
const baseline = readBaseline();
const newViolations = violations.filter((entry) => !baseline.has(entry.fingerprint));
const resolvedBaseline = [...baseline].filter((fingerprint) => !current.has(fingerprint));

const report = {
  schemaVersion: 1,
  generatedBy: "scripts/verify/i18n-ci.mjs",
  surfaces,
  summary: {
    currentViolations: violations.length,
    baselineViolations: baseline.size,
    newViolations: newViolations.length,
    resolvedBaselineViolations: resolvedBaseline.length,
  },
  violations,
  newViolations,
  resolvedBaselineViolations: resolvedBaseline,
};

if (reportPath) {
  mkdirSync(dirname(reportPath), { recursive: true });
  writeFileSync(reportPath, JSON.stringify(report, null, 2) + "\n");
}

console.log("i18n Phase 3 audit:");
console.log(JSON.stringify(report.summary));
if (violations.length) {
  for (const entry of violations) {
    console.log(`I18N_VIOLATION ${entry.fingerprint} ${entry.kind} ${entry.file}: ${entry.value}`);
  }
}
if (resolvedBaseline.length) {
  console.log(`I18N_BASELINE_RESOLVED ${resolvedBaseline.length}`);
}

if (updateBaseline) {
  const next = {
    schemaVersion: 1,
    policy: "Shrink-only baseline. Entries are line-independent fingerprints of pre-existing localization debt.",
    generatedFrom: process.env.GITHUB_SHA ?? "manual",
    entries: violations.map(({ fingerprint, kind, file, value, detail }) => ({ fingerprint, kind, file, value, detail })),
  };
  writeFileSync(baselinePath, JSON.stringify(next, null, 2) + "\n");
  console.log(`Wrote ${baselinePath} with ${next.entries.length} entries.`);
  process.exit(0);
}

if (newViolations.length > 0) {
  console.error(`i18n Phase 3 audit failed: ${newViolations.length} violation(s) are not in i18n-baseline.json.`);
  process.exit(1);
}

console.log("i18n Phase 3 audit passed: no new localization debt.");
