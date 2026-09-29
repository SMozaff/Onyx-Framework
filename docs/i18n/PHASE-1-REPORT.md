# ONYX i18n Phase 1 Report

## Scope

Phase 1 built the canonical localization foundation and nothing else. The
contract is `docs/i18n/PHASE-0.5-DECISIONS.md`, Phase 1 Contract P1-1 … P1-13.

Executed exactly: P1-1, P1-2, P1-3, P1-4, P1-5, P1-6, P1-7, P1-8, P1-9, P1-10,
P1-11, P1-12, P1-13.

Not executed, by instruction: application string migration, `mobile-android/`
changes, Rust changes, Android resource generation, `metadata/glossary.json`,
literal-audit widening beyond `web-ui/src/pages/Dashboard`, OpenAPI error
contract changes, `ShellError` changes, `NotificationAggregate` changes,
visual RTL/typography work, Farsi wording improvements.

## Deliverables

| Item | Decision | Artifact | State |
| --- | --- | --- | --- |
| P1-1 | Locale registry | `shared/i18n/metadata/locales.json` | Created |
| P1-2 | Direction from registry | `shared/i18n/react/I18nContext.tsx` + 4 generated copies | Done |
| P1-3 | Translation schema | `shared/i18n/schema/translation.schema.json` | Created |
| P1-4 | Generator checks | `scripts/sync-i18n.mjs` | Done |
| P1-5 | Missing-key fallback | `shared/i18n/react/I18nContext.tsx` + 4 generated copies | Done |
| P1-6 | Formatting abstraction | `shared/i18n/react/formatting.ts`, `scripts/verify/verify-i18n-runtime.mjs` | Done |
| P1-7 | Thin-client compliance | no new browser-storage write in `web-ui/src/` | Honoured |
| P1-8 | Pluralization | `shared/i18n/react/plural.ts` | Done |
| P1-9 | Switcher mounting | 6 files across 3 surfaces | Done |
| P1-10 | Dangling keys | `en.json`, `fa.json`, `web-ui/src/pages/Dashboard/index.tsx` | Done |
| P1-11 | Boundaries | `i18n.boundaries.json` | Created |
| P1-12 | Documentation | `shared/i18n/README.md` | Created |
| P1-13 | CI wiring | `.github/workflows/ci.yml` `i18n` job | Done |

## P1-1 — Locale registry

`shared/i18n/metadata/locales.json` is the single source of locale metadata:
`direction`, `default`, `displayName`, and `formatLocale` per locale.

```text
en → ltr,  default: true,  English,  en-US
fa → rtl,  default: false, فارسی, fa-IR
```

`scripts/sync-i18n.mjs` embeds the registry into every generated dictionary, so
applications receive registry changes through regeneration rather than by
hand-editing generated files.

## P1-2 — Direction reads from the registry

`shared/i18n/react/I18nContext.tsx` no longer contains a locale-specific
direction branch. Removed:

```text
document.documentElement.dir = locale === "fa" ? "rtl" : "ltr"
dir: locale === "fa" ? "rtl" : "ltr"
isRTL: locale === "fa"
```

Direction now flows registry → `metadata.direction` → `document.documentElement.dir`,
`dir`, and `isRTL`. `DEFAULT_LOCALE` derives from `defaultLocale`, and
`detectInitialLocale()` uses a registry-driven `matchBrowserLocale()` instead of
the hardcoded `nav.startsWith("fa")` check. Locale validation is registry-based
via `localeIds`, so a future locale requires only registry and catalog changes.

## P1-3 — Translation schema

`shared/i18n/schema/translation.schema.json` enforces the ratified convention:

- `type: object`, `minProperties: 1`;
- `propertyNames.pattern`: exactly two segments, namespace ∈ the 14 ratified
  namespaces, leaf begins with a lowercase ASCII letter and contains only
  ASCII letters and digits — so `snake_case` is prohibited;
- `patternProperties` requiring non-empty string values;
- `additionalProperties: false`;
- one explicit `common.total_items` property, documented in the schema as the
  single grandfathered Phase 1 exception.

The schema deliberately rejects the migration plan's `§4.2/§8` illustrations
(`common.actions.save`, `missions.status.pending`, `errors.permission_denied`),
consistent with the decision record's explicit ruling to keep the ONYX
two-segment convention.

## P1-4 — Generator checks

`scripts/sync-i18n.mjs` now performs, before any write:

1. `validateLocaleRegistry` — exactly one default locale, BCP 47 `formatLocale`,
   `ltr`/`rtl` direction, non-empty `displayName`, locale-id shape.
2. `validateCatalogAgainstSchema` — validates `en.json` and `fa.json` against the
   schema's `object`/`propertyNames`/`patternProperties` shape.
3. EN/FA key parity in both directions.
4. Interpolation-placeholder parity for every shared key.
5. `scanUnresolvedLiteralKeys` — fails if a static translation key is absent
   from `en.json`.

Findings are aggregated into `failures[]` and reported together, so one run
surfaces every problem instead of only the first.

Unresolved-key detection resolves callees from `const { … } = useI18n()`
destructuring (including aliases), `import { t } from …/i18n/I18nContext`
(including aliases), and the `useI18n().t` member form. Comments are stripped
before scanning, and only static string literals are considered. The scan
deliberately excludes `node_modules`, `dist`, `build`, `coverage`, `tests`,
`__tests__`, `__mocks__`, `.git`, each surface's generated `src/i18n` directory,
and `*.test.*` / `*.spec.*` files. Template literals containing `${…}` and
dynamically constructed keys are out of scope because they cannot be resolved
without executing the application; P1-5's runtime behaviour is the safety net
for those.

Current result: 55 literal keys across 130 application source files, all
resolving; 150 catalog keys.

## P1-5 — Missing-key fallback

`t()` no longer returns the raw key. Resolution order is unchanged
(active locale → English), and an unresolved key now renders an empty string.
Interpolation behaviour is untouched. CI is the detector, per the contract.

## P1-6 — Formatting abstraction

`shared/i18n/react/formatting.ts` exposes `createFormatters(formatLocale)`
returning `formatDate`, `formatDateTime`, `formatRelativeTime`, `formatNumber`,
`formatPercent`, `formatCurrency`, and `formatDuration`.

The locale is a parameter, normally `localeMetadata[locale].formatLocale`, and
the implementation contains no per-language branches: calendars, digits,
plural-sensitive units, and list conjunctions all come from `Intl`. Callers may
pass no options and still get correct locale behaviour; `style` and `currency`
are applied after caller options so a stray option cannot silently change the
formatter's contract. `formatDuration` decomposes into localized unit phrases
joined by `Intl.ListFormat`.

The 21 existing bare `toLocaleString()` call sites were not routed, as
instructed; that is surface migration.

Unit tests: `scripts/verify/verify-i18n-runtime.mjs`, run by the standalone
`i18n` job. Ten tests cover API completeness, locale parameterization (en-US vs
fa-IR), date/relative-time rendering, percent/currency/number output, duration
decomposition, `Intl.PluralRules` agreement, `other`-fallback behaviour, and
`TypeError`/`RangeError` input validation. TypeScript is loaded with
`module.stripTypeScriptTypes`, so the check needs no build step and no
additional dependency.

The decision record's P1-13 checklist does not mention formatting tests, and no
JavaScript test runner covers `shared/i18n`. The test was therefore placed in
the authoritative `i18n` job rather than in `web-ui/tests/`: a test in the `web`
job could never be shown green while blocker B-2 keeps that job red, so it
would not satisfy the verification obligation. Adding this step is a
documented addition to the P1-13 step list, not a scope expansion — P1-6
explicitly requires that Phase 1 "provides and unit-tests the API".

## P1-7 — Thin-client compliance

No new `localStorage`/`sessionStorage` write was introduced under `web-ui/src/`,
and no second persistence mechanism was added.

The single `localStorage.setItem(LOCALE_STORAGE_KEY, locale)` in
`web-ui/src/i18n/I18nContext.tsx` is the sanctioned exception. It is unchanged
from the pre-Phase-1 baseline — the line appears as unchanged context in
`git diff`, and the identical failure appears at base commit `51112f0`. It
remains the subject of blocker B-2 and was deliberately not touched.

The regenerated copies in the other three surfaces inherit the same line from
the canonical source; those surfaces are not scanned by B-2.

## P1-8 — Pluralization

`shared/i18n/react/plural.ts` provides `pluralCategory(locale, count)` via
`Intl.PluralRules` and `selectPluralForm`, which requires `other` and falls back
to it when a category is untranslated. `I18nContext` exposes `pluralCategory`
bound to the active locale. There are no English- or Persian-specific rules.

The three existing hardcoded English plural sites were not refactored, as
instructed.

## P1-9 — Switcher mounting

`LanguageSwitcher` was mounted in six files, making `fa` reachable in the three
surfaces where it was previously unreachable:

```text
mobile-pwa/src/components/Layout.tsx
mobile-pwa/src/pages/Login/index.tsx
crates/bins/desktop-shell/ui/src/components/Layout/MainLayout.tsx
crates/bins/desktop-shell/ui/src/pages/Login.tsx
crates/bins/admin-shell/ui/src/components/Layout/MainLayout.tsx
crates/bins/admin-shell/ui/src/pages/Login.tsx
```

`web-ui` already mounted it in `MainLayout.tsx` and `Sidebar.tsx` and was left
alone.

## P1-10 — Dangling keys

```text
web-ui/src/pages/Dashboard/index.tsx
  approvals.reviewApproval → missions.reviewApproval

shared/i18n/en.json    common.total_items → "Total items"
shared/i18n/fa.json    common.total_items → "تعداد کل موارد"
```

`common.total_items` is the one grandfathered `snake_case` key, required to
resolve an existing call site at `web-ui/src/pages/Missions/index.tsx`. It is
declared explicitly in the schema and documented in the README so it cannot
spread.

## P1-11 — Boundaries

`i18n.boundaries.json` inventories 11 surfaces, all `unmanaged`, each with real
repository paths: the four React frontends, `mobile-android/app/src/main`, the
desktop and admin Rust hosts, the API presentation boundary, the worker
notification boundary, the mobile FFI boundary, and the migration-tool CLI.
The document records what belongs to localization and what must never be
localized, including the frozen error wire fields.

## P1-12 — Documentation

`shared/i18n/README.md` documents the canonical source tree, catalogs, the
two-segment key convention with the 14 namespaces and the grandfathered
exception, the locale registry and direction flow, the canonical/generated
split, the formatting and pluralization APIs, the generator and its commands,
every enforced check, the boundaries, and the fact that CI is authoritative.

## P1-13 — CI wiring

The standalone `i18n` job in `.github/workflows/ci.yml` has no `needs:` and is
therefore independent of `web`, lint, type-check, and test jobs. Verified by
inspecting the job definition and by the run below, where `i18n` passed while
`web` failed in the same run.

Steps:

1. `node scripts/sync-i18n.mjs --check` — registry, schema, parity,
   interpolation, unresolved keys, determinism.
2. `node scripts/sync-i18n.mjs --check --audit-scope=web-ui/src/pages/Dashboard`
   — existing enforced literal audit.
3. `node --test scripts/verify/verify-i18n-runtime.mjs` — formatting and
   pluralization unit tests.

The `web` job retains its own redundant localization step for frontend context;
the standalone job remains authoritative.

## GitHub Actions verification

No local build, type-check, lint, test, Cargo, Gradle, or Playwright execution
was used. Local activity was limited to static inspection, `git diff`,
`node --check` syntax inspection, and running the generator.

**Authoritative run: `36599451087` at commit `4fba188`.**

| Job | Result |
| --- | --- |
| `i18n` | **success** |
| `check` | success |
| `deploy-check` | success |
| `load-smoke` | success |
| `web` | failure — pre-existing B-2 |
| `mobile-android-kotlin` | failure — pre-existing B-1 |
| `native-ui-evidence` | skipped — declares `needs: web`, suppressed by B-2 |

Run conclusion: `failure`, caused only by the two pre-existing blockers below.

`i18n` job steps, all `success`:

```text
1  success  Set up job
2  success  Run actions/checkout@v7
3  success  Run actions/setup-node@v7
4  success  Verify canonical catalogs, schema, parity, interpolation, unresolved keys, and generated-resource determinism
5  success  Audit user-facing literals in enforced surfaces
6  success  Unit-test the canonical formatting and pluralization API
```

Unit test result from that job: `# tests 10`, `# pass 10`, `# fail 0`.

Supporting runs, all at the same Phase 1 content:

- `36596242539` (`ceb85d6`) — `i18n` success, `check` success,
  `deploy-check` success, `load-smoke` success.
- `36598656064`, `36599159245` — intermediate `i18n` failures used to converge
  the new test (a `Buffer.from` encoding bug, then two incorrect test
  expectations). Both were test-harness defects, not implementation defects, and
  were fixed before the final run.

### Baseline comparison

The two failing jobs fail identically at the pre-Phase-1 baseline run
`36579053876` (commit `51112f0`):

- `web` — failed at `Unit and integration tests`;
  `tests/feature-audit/excluded-features.test.ts` reported
  `web-ui/src/i18n/I18nContext.tsx: /localStorage\.setItem/i`, with
  `Test Files 1 failed | 9 passed | 1 skipped (11)` at both the baseline and the
  Phase 1 run. Identical.
- `mobile-android-kotlin` — failed at `Gradle assembleDebug` at both the
  baseline and the Phase 1 run. Identical.

`mobile-android/` has no Phase 1 diff, so Phase 1 cannot have caused B-1.

## Pre-existing failures (not Phase 1 regressions)

### B-1 — `mobile-android-kotlin`

`Gradle assembleDebug` fails. Root cause remains undetermined; CI log retrieval
requires administrative rights (HTTP 403). Phase 1 did not touch
`mobile-android/`. Owner: Phase 2. Not claimed fixed.

### B-2 — `web`

`tests/feature-audit/excluded-features.test.ts` bans
`localStorage.setItem` under `web-ui/src/`, and the sanctioned locale-persistence
write in `I18nContext.tsx` matches it. This is the direct consequence of binding
ruling T6-D12 (thin client) versus locale persistence. It requires a
project-owner ruling before Phase 4.

Not resolved here. No new storage write was added, the test was not modified,
and the offending call was not removed, because each of those would either
violate P1-7 or silently discard locale persistence.

## Deviations

1. **P1-13 gained a third step.** The formatting/pluralization unit test
   required by P1-6 was added to the `i18n` job rather than to `web-ui/tests/`,
   because a test in the `web` job could never be reported green while B-2 keeps
   that job red. Rationale is recorded under P1-6 above.
2. **`common.total_items` is snake_case.** P1-3 prohibits `snake_case`; P1-10
   mandates this exact key. P1-10 wins for this single key, which is declared
   explicitly in the schema and documented in the README.

Neither deviation weakens a frozen contract, and no additional work was
performed beyond P1-1 … P1-13.

## Not verified

- **Visual RTL behaviour.** Direction now flows from the registry, but no visual
  RTL, typography, or snapshot verification was performed. Typography is Phase 9.
- **Persian translation quality.** No Farsi wording was changed or reviewed.
- **Unresolvable-by-design keys.** Dynamically constructed translation keys and
  `${…}` template literals are outside the static scan and are unverified.
- **B-1 root cause.** Undetermined.
- **B-2 resolution.** Requires a project-owner ruling.
- **`native-ui-evidence`.** Skipped because it declares `needs: web`; its
  behaviour under B-2 is unverified.

## Phase status

**PHASE 1 COMPLETE WITH DOCUMENTED EXCEPTIONS**

P1-1 through P1-13 are implemented. The authoritative `i18n` job is green in run
`36599451087`, with the formatting and pluralization unit tests passing 10 of
10, and `check` (lint, type-check, tests) and `deploy-check` green in the same
run.

The exceptions are: the two pre-existing blockers B-1 and B-2, which are
byte-for-byte identical to the pre-Phase-1 baseline and are owned by later
phases or by a project-owner ruling; and the two deviations recorded above. No
Phase 1 contract item is unimplemented.

Phase 2 has not been started.
