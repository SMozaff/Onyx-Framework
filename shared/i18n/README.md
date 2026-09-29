# ONYX Canonical Localization System

`shared/i18n/` is the canonical localization source. It is retained in place.
Do not relocate it, replace it with a third-party localization framework, or
treat generated platform copies as editable sources.

The authoritative Phase 1 contract is
`docs/i18n/PHASE-0.5-DECISIONS.md`.

## Canonical source

```text
shared/i18n/
├── en.json
├── fa.json
├── metadata/
│   └── locales.json
├── react/
│   ├── I18nContext.tsx
│   ├── LanguageSwitcher.tsx
│   ├── formatting.ts
│   ├── plural.ts
│   └── rtl.css
├── schema/
│   └── translation.schema.json
└── README.md
```

Only English (`en`) and Persian (`fa`) are supported in Phase 1.

## Catalogs

```text
shared/i18n/en.json
shared/i18n/fa.json
```

Both catalogs must contain the same keys. Key parity is CI-enforced.

Values must be non-empty strings. Catalogs must remain flat objects; nested
translation objects are prohibited.

## Key convention

Canonical keys use exactly two segments:

```text
<namespace>.<lowerCamelCaseLeaf>
```

Examples:

```text
common.save
nav.missions
auth.signIn
status.pending
missions.noMissionsBody
notifications.acknowledge
language.title
```

Rules:

1. Exactly two segments.
2. The namespace must be one of the 14 ratified namespaces below.
3. The leaf must begin with a lowercase ASCII letter and contain only ASCII
   letters and digits.
4. `snake_case` is prohibited for new keys.
5. Values are strings.
6. Keys are semantic identifiers, not visible English text.
7. Do not rename the catalog merely for architectural aesthetics.

`common.total_items` is the sole grandfathered Phase 1 syntax exception. It
was required to resolve an existing unresolved call site. New snake_case keys
must fail schema validation.

## Known namespaces

```text
app
nav
auth
common
status
dashboard
missions
tasks
notifications
approvals
reports
files
settings
language
```

A new namespace requires a schema change and catalog updates in both locales.

## Locale registry

```text
shared/i18n/metadata/locales.json
```

The registry is the single source of locale metadata. It supplies:

- locale identifiers;
- text direction;
- default locale;
- display names;
- `Intl` formatting locales.

Current mapping:

```text
en → ltr, default, English, en-US
fa → rtl, non-default, فارسی, fa-IR
```

`I18nContext.tsx` must not hardcode direction with locale-specific branches.
Direction flows as:

```text
locale
  ↓
locales.json
  ↓
direction
  ↓
document.documentElement.dir
```

The generator embeds the registry in every generated dictionary. Generated
applications therefore receive registry updates through regeneration, not
through hand editing.

## Generated resources

The following React applications consume the canonical system:

```text
web-ui/src/i18n/
mobile-pwa/src/i18n/
crates/bins/desktop-shell/ui/src/i18n/
crates/bins/admin-shell/ui/src/i18n/
```

Each generated directory contains:

```text
dictionaries.generated.ts
I18nContext.tsx
LanguageSwitcher.tsx
formatting.ts
plural.ts
rtl.css
```

`dictionaries.generated.ts` contains the canonical catalogs and the generated
locale registry. The other five files are verbatim copies of canonical React
sources.

Never edit generated files directly. Change the canonical JSON, registry,
React source, or generator, then regenerate all four targets.

## Formatting and pluralization

`formatting.ts` provides the canonical locale-aware formatting API:

```text
formatDate
formatDateTime
formatRelativeTime
formatNumber
formatPercent
formatCurrency
formatDuration
```

Formatting resolves its behavior through `locales.json → formatLocale`. There
are no per-language formatting branches; calendars, digits, units, and list
conjunctions come from `Intl`.

`plural.ts` provides locale-aware plural categories through
`Intl.PluralRules`. There are no English- or Persian-specific plural rules.
Existing hardcoded plural rendering in application components is surface
migration and was intentionally not changed in Phase 1.

## Generator

```text
scripts/sync-i18n.mjs
```

The generator is the sole mechanism for producing and checking generated
localization resources.

Canonical commands:

```bash
node scripts/sync-i18n.mjs --check
node scripts/sync-i18n.mjs --check --audit-scope=web-ui/src/pages/Dashboard
```

The first command validates and checks generated-resource determinism. The
second adds the existing scoped user-facing literal audit.

Authoritative executable verification occurs through GitHub Actions. Local
commands are for authoring and regeneration, not for claiming that CI passes.

## Validation

`--check` enforces:

- well-formed locale-registry metadata;
- translation-schema conformance for both catalogs;
- exact EN/FA key parity;
- interpolation-placeholder parity for every shared key;
- absence of unresolved static translation keys at application call sites;
- byte-identical generated dictionaries and runtime copies.

The scoped audit additionally ensures that JSX text in
`web-ui/src/pages/Dashboard` uses translation keys rather than hardcoded
user-facing literals.

Unresolved-key detection covers static calls obtained from `useI18n()` and
direct `translateStatic()` calls. Dynamically constructed keys, template
literals containing interpolations, translation calls in tests and fixtures,
and generated runtime copies are intentionally outside that static scan.
Runtime rendering must therefore return an empty string—not a raw key—if a
key is nevertheless absent.

Missing translations resolve as:

```text
active locale
    ↓
English fallback
    ↓
empty rendering
```

CI detects missing keys. Runtime must not expose identifiers such as
`missions.someKey` to users.

## Boundaries

Localization boundaries are inventoried in:

```text
i18n.boundaries.json
```

Belongs to localization:

- user-visible application copy;
- accessibility labels and other assistive-technology text supplied through
  localized components;
- locale-aware dates, times, numbers, currencies, durations, relative times,
  and plural categories;
- locale direction metadata.

Must never be localized:

- API paths, HTTP methods, JSON field names, and wire identifiers;
- domain enum values and other machine-readable state;
- frozen error `code`, `category`, `retryability`, and correlation fields;
- cryptographic hashes, UUIDs, numeric protocol values, and locale-sensitive
  formatting of non-display data;
- logs and diagnostics unless explicitly intended for end users.

All Phase 1 surfaces remain `unmanaged` because Phase 1 builds the foundation
without migrating application screens.

## CI

Localization is enforced by the standalone `i18n` job in:

```text
.github/workflows/ci.yml
```

The job has no dependency on frontend linting, type-checking, tests, builds,
or unrelated jobs. It verifies canonical catalogs, schema validity, EN/FA
parity, interpolation parity, deterministic generation, unresolved literal
translation keys, and the existing enforced literal audit.

The `web` job retains its own redundant localization step for frontend
context. The standalone `i18n` job is authoritative.
