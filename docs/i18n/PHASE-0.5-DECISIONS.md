# ONYX i18n Phase 0.5 Decisions

**Date:** 2026-09-29
**Phase:** 0.5 — Baseline Verification & Architecture Decisions
**Verified against:** `main` @ `f74832b` (CI run `36577581791`), and historical runs `36572636871` (`d69604a`), `36466833937` (`67409f5`), `36465446630` (`f487592`), `36268538073` (`644e1596`)
**Governing inputs:** `ONYX_Multilingual_Infrastructure_Migration_Plan.md`, `ONYX_Full_Application_Manifest_2026-09-13.md`, `docs/i18n/PHASE-0-DISCOVERY-REPORT.md`, `docs/DECISIONS.md`, `docs/api/openapi.json`

This file is the authoritative decision record for Phase 1. Where this file and the Phase 0 report disagree, this file wins.

---

## Baseline

**DECISION — Phase 0.5 baseline classification: `VERIFIED RED — i18n-related (partially repaired)`.**

Executed evidence, GitHub Actions run `36577581791` on `f74832b`:

| Gate | Before (`d69604a`) | After (`f74832b`) | Class |
|---|---|---|---|
| `i18n` job (catalog parity + determinism + literal audit) | **DID NOT EXIST** | **SUCCESS** | New, green |
| web — Lint | **FAIL** | **SUCCESS** | Repaired |
| web — Type check | SKIPPED | **SUCCESS** | Repaired |
| web — Unit & integration tests | SKIPPED | **FAIL** | Pre-existing, i18n-caused, unresolved |
| `mobile-android-kotlin` — Gradle assembleDebug | **FAIL** | **FAIL** | Pre-existing, root cause undetermined |

The localization contract gate is **VERIFIED GREEN via CI** and now runs independently of the frontend build chain.

Two defects remain red. Both are documented below and neither blocks Phase 1 (see *Blocking Issues*).

**DECISION — the last fully-green baseline is `644e1596` (2026-09-26).** Every red gate since then was introduced by the i18n commit series (`f487592` → `d69604a`).

---

## Canonical Source

**DECISION — `shared/i18n/` is canonical. Retain it in place. Do not relocate it to the plan's §5 target structure.**

Canonical (hand-authored, authoritative):

```
shared/i18n/en.json              149 keys, flat, 2-segment, string values
shared/i18n/fa.json              149 keys, flat, 2-segment, string values
shared/i18n/react/I18nContext.tsx     LocaleProvider / useI18n / translateStatic
shared/i18n/react/LanguageSwitcher.tsx
shared/i18n/react/rtl.css
scripts/sync-i18n.mjs            sole generator + sole parity/drift auditor
```

Generated (never hand-edit):

```
web-ui/src/i18n/{dictionaries.generated.ts, I18nContext.tsx, LanguageSwitcher.tsx, rtl.css}
mobile-pwa/src/i18n/{…same 4}
crates/bins/desktop-shell/ui/src/i18n/{…same 4}
crates/bins/admin-shell/ui/src/i18n/{…same 4}
```

Rationale: this source is already deterministic, already byte-verified in CI, already consumed by four platforms, and contains zero third-party i18n dependencies. The plan §2 directs extending rather than replacing an equivalent existing structure.

**DECISION — the plan's §5 sub-structure is adopted additively, without relocating catalogs.**

Phase 1 creates (does not move anything):

```
shared/i18n/schema/translation.schema.json
shared/i18n/metadata/locales.json
shared/i18n/README.md
i18n.boundaries.json
```

`shared/i18n/metadata/glossary.json` is **deferred to the Localization Quality Program** (plan §39). It is explicitly *not* Phase 1.

**DECISION — NOT canonical, must not become canonical:** `mobile-android/app/src/main/res/values*/strings.xml`. These are a hand-authored second catalog. They become generated output in Phase 2.

---

## Translation Key Format

**DECISION — RETAIN FLAT KEYS. 2-segment dotted format. Established ONYX convention wins over the plan's examples.**

Evidence (executed against `en.json`):

- All 149 keys are **exactly 2 segments** — zero exceptions.
- 14 namespaces, all single lowercase words: `app, nav, auth, common, status, dashboard, missions, tasks, notifications, approvals, reports, files, settings, language`.
- Every leaf is **lowerCamelCase**. Zero snake_case keys exist anywhere.
- Every value is a **string**. Zero non-string values.

Canonical convention, to be documented in `shared/i18n/README.md` and enforced by schema in Phase 1:

```
<namespace>.<lowerCamelCaseLeaf>

app.name                 nav.missions             common.save
auth.signIn              status.pending           missions.noMissionsBody
dashboard.activeMissions notifications.acknowledge language.title
```

Rules:
1. Exactly one namespace segment and one leaf segment. Never three or more.
2. Namespace ∈ the 14 listed. A new namespace requires a schema change.
3. Leaf is lowerCamelCase. **snake_case is prohibited.**
4. Values are strings. Structure is expressed by key count, not nesting.
5. Keys are **semantic**, never visible English text. `common.save`, never `"Save"`.

**IMPORTANT — this DECIDES AGAINST the migration plan's §4.2/§8 examples.** The plan shows `common.actions.save`, `missions.status.pending`, `errors.permission_denied`, `notifications.connection_lost`. ONYX has established `common.save`, `status.pending`, and no `errors` namespace at all. Phase 1 keeps the ONYX convention. Renaming 149 keys across 16 generated files and 38 call sites to match the plan's illustration would be a large, behavior-neutral, high-risk change with no functional benefit.

Android resource names are derived deterministically: `migrations.status.pending` → `missions_status_pending` (dots to underscores). Android keeps its existing underscore names in Phase 2 as the generator's output format; it does not redefine the canonical key.

---

## Locale Registry

**DECISION — create `shared/i18n/metadata/locales.json` in Phase 1 and make it the single source of direction/default/formatting metadata.**

Required fields per locale:

```json
{
  "en": { "direction": "ltr", "default": true,  "displayName": "English", "formatLocale": "en-US" },
  "fa": { "direction": "rtl", "default": false, "displayName": "فارسی",    "formatLocale": "fa-IR" }
}
```

Phase 1 **must** remove the hardcoded `locale === "fa" ? "rtl" : "ltr"` from `I18nContext.tsx:47-48,63` and read from the registry instead. Direction is currently duplicated in two independent places (`I18nContext.tsx` and Android `LocaleHelper.kt:30`); a third locale requires editing both. The registry is the precondition for plan §26's synthetic-locale test and plan §10's readiness criterion.

**DECISION — the Rust workspace does not get a locale registry in Phase 1.** No Rust crate currently loads a catalog. Building a Rust catalog loader is Phase 2 work and depends on DR-1/DR-7 outcomes.

---

## Generated Resources

**DECISION — the canonical source is hand-authored; every platform artifact is generated and byte-verified in CI.**

Phase 1 does **not** generate Android resources. Generation scope in Phase 1 is exactly the four React targets already handled by `scripts/sync-i18n.mjs`.

Phase 2 adds Android generation (`values/strings.xml`, `values-fa/strings.xml`) and, if DR-5 resolves, per-app RTL layers.

**DECISION — determinism requirement is unchanged and already verified.** Phase 0 established all four generated dictionaries are byte-identical (md5 `480f61e023a4851ff31d97091957a6fd`); CI run `36577581791` step `i18n.4` re-confirmed `node scripts/sync-i18n.mjs --check` passes.

---

## Missing Translation Policy

**Current behaviour (executed + statically verified):** `I18nContext.tsx:63`

```ts
const hit = active[key] ?? fallback[key];
if (hit === undefined) return key;
```

Fallback chain is active locale → English → **raw key string rendered to the user**, silently. Two keys currently reach this path in production (`approvals.reviewApproval`, `common.total_items` — both verified absent from both catalogs by extracting all 38 distinct call-site keys and diffing).

**DECISION — three-tier policy.**

1. **Resolution order is unchanged:** active locale → English. English fallback is retained and is *not* an error.
2. **An unresolved key must never be silently rendered.** Phase 1 changes `return key` to `return en[key] ?? ''` **and** adds an unresolved-key check to `sync-i18n.mjs` that fails CI.
3. **Development vs production diverge on the missing key itself, not on fallback.** In both, a missing key renders empty rather than leaking `missions.someKey` to a user. The CI check is what makes the failure visible; runtime must not be the detector.

**DECISION — the two dangling keys are resolved in Phase 1, not Phase 0.5.**

- `approvals.reviewApproval` → repoint the call site to the existing `missions.reviewApproval`. No new translation required.
- `common.total_items` → requires new Farsi copy. **Translation authoring is out of Phase 0.5 scope** and belongs to Phase 1's catalog work.

---

## RTL Architecture

**DECISION — hybrid: a canonical direction contract, plus per-platform RTL adapters. Do not keep one shared stylesheet as the mechanism.**

Evidence: `shared/i18n/react/rtl.css` is 68 lines with 17 `html[dir="rtl"]`-guarded rules, copied verbatim into all four React apps. It is effective in `web-ui` (13 of 17 rules hit real class names) and **2 of 17** effective in `mobile-pwa`, `desktop-shell/ui`, and `admin-shell/ui`, because those apps use different markup (`onyx-*` classes, Tailwind utilities). A single shared stylesheet demonstrably cannot serve both a BEM-CSS app and Tailwind apps.

Split:

- **Canonical direction contract:** `locales.json` `direction` field is the single authority. `document.documentElement.dir` is set from it. This is the shared contract.
- **Platform adapters:** each app owns its own directional CSS/logic. In Phase 1 the existing shared `rtl.css` is **retained as-is** (it is harmless where inert, and removing it is Phase 9 work). Per-app RTL layer generation is Phase 2/Phase 9.

**DECISION — Phase 1 does not perform RTL work beyond reading direction from `locales.json`.** Layout mirroring is Phase 9 per the plan.

**DECISION — typography remains unwired.** No `.ttf`/`.woff`/`.otf` exists anywhere in the repository; `Vazirmatn` is only a CSS font-family *name* in the five `rtl.css` copies. Font embedding is a licensing decision (deferred) plus Phase 9.

---

## Notification Boundary

**DECISION — the target architecture is code + params. But the migration is NOT Phase 1, and the cost is far lower than Phase 0 estimated.**

Target:

```
Domain event:  { code: "staff_loan.ending_soon", params: { … }, metadata: {…} }
     ↓
Presentation:  map code → translation key
     ↓
Rendering:     localized, per client locale
```

**Phase 0.5 corrections to Phase 0's assessment:**

1. **Phase 0 stated the change is gated by `scripts/verify/verify_serialization.sh` golden fixtures. This is incorrect.** Executed inspection: that script's own header states it *"fails, on purpose"* because `tests/golden/` does not exist and no `--test golden_fixtures` harness exists. `ls tests/golden` → absent. The script is **not run in CI**. There is therefore **no golden-fixture gate to satisfy.**
2. **Phase 0 implied a persistence/schema migration. There is none.** Notifications are stored as a JSONB `state` blob in the **generic `aggregates` table** (`job_runner.rs:400-406`: `INSERT INTO aggregates (…, aggregate_type, …, state, …) VALUES (…, 'notification', …, $3, …)`). There is **no `notifications` table** in any migration. A notification shape change is a **JSON shape change**, not DDL.
3. **DECISIONS.md T6-D1 is superseded by the current code.** T6-D1 ruled that `NotificationAggregate` would be "defined inside Team 6's command route module". Executed: the only definition in the workspace is `crates/domains/notification-domain/src/lib.rs:15`. The crate-based implementation won.

**DECISION — DR-1: RESOLVED as direction, DEFERRED as implementation. Owning phase: Phase 7.**

Required shape (for Phase 7, not implemented now):

```rust
pub struct NotificationAggregate {
    …                         // identity, epochs, versions — unchanged
    pub message_code: String,       // "staff_loan.ending_soon"
    pub message_params: serde_json::Value,
    pub title_code: String,
    pub title_params: serde_json::Value,
    pub priority: NotificationPriority,   // enum, replacing bare String
    pub status:   NotificationStatus,     // enum, replacing bare String
}
```

Compatibility requirements for Phase 7:

- **Additive, not breaking.** `#[serde(default)] title: Option<String>` / `message: Option<String>` must still deserialize legacy rows, falling back to a `notification.legacy` code.
- Legacy rows already exist in `aggregates.state` and are replicated to peers, so this is a **replicated-state** concern: the sync wire format (`W1` ruling, `sync-transport`) must be able to carry both shapes during a transition window.
- Consumers to update in the same change: `worker/src/job_runner.rs:205-209,273-276` (producers), `worker/src/push_delivery.rs:323-325` (Web Push OS body), and every client notification list.
- `priority` and `status` are bare `String` today and are compared by string literal (`notification-domain/src/lib.rs:82,94`). Converting them to enums is in scope for Phase 7, not Phase 0.5.

**DECISION — DR-3 (Web Push) is DEFERRED to Phase 7 and is coupled to DR-1.** Web Push OS notification chrome is rendered by the operating system; ONYX supplies the body. Server-side per-locale pre-rendering vs. client pre-rendering from `code+params` cannot be decided until DR-1's shape is fixed. Phase 0.5 does not decide it.

---

## Error Boundary

**DECISION — the API error wire contract is FROZEN and must not change. This is not a Phase 0.5 preference; it is a binding constraint.**

Evidence:

- **DECISIONS.md T6-R1 (binding, 2026-08-05):** *"Team 6 owns the OpenAPI freeze. `docs/api/openapi.json`, version 1.0.0."*
- `docs/api/openapi.json` `components.schemas.CommandError.required` = `["code","category","retryability","safe_details","correlation_id"]`, with `category` and `retryability` as **closed enums**.
- `web-ui/tests/unit/openapi-contract.test.ts` asserts the frozen required-field lists in CI.

**Consequence: no Phase 1, Phase 2, or any later phase may change the HTTP error shape.** Presentation must map from `error.code`.

**DECISION — the architecture is: machine code at the wire, localized text at the presentation edge.** The correct chain, and it is already 80% built:

```
DomainError (platform-contracts)  → 8 stable #[serde(rename)] codes + .code()/.category()/.retryability()
     ↓
ApiError { code, category, retryability, safe_details, correlation_id }   ← FROZEN, matches OpenAPI 1.0.0
     ↓
Presentation maps error.code → translation key
     ↓
Localized text
```

**DECISION — `safe_details.message` English prose is NOT to be removed in Phase 1 and does not block it.** Removing it is a wire-semantics change requiring an explicit contract amendment, which Phase 0.5 does not have authority to make. It is assigned to Phase 8.

**DECISION — C3 (English-substring control flow) is a CORRECTNESS prerequisite, assigned to Phase 8, and MUST precede any error-prose work.**

Executed evidence of the hazard:

```rust
// crates/bins/api-server/src/routes/command.rs:1348
crate::CommandError::Domain(message) if message.contains("not permitted") => …
// crates/bins/api-server/src/routes/command.rs:1357
if message.contains("already acknowledged") || message.contains("not pending") => …
```

These select HTTP status and error code. Rewording any `#[error("…")]` in a domain crate silently changes API behaviour.

Minimal safe refactor (Phase 8, not now): match on the typed `DomainError` variants already available from `platform-contracts` (8 codes) instead of substring matching. **This changes no wire shape** — it only makes the code that populates `code` robust.

Same class of hazard, out of scope for i18n but noted: `crates/infrastructure/security-adapter/src/user_store.rs:37` (`message.contains("unique")`) and `:108` (foreign-key constraint substring) classify **Postgres driver** messages. Not localization-related; do not touch under the i18n migration.

`scripts/verify/verify_error_exhaustiveness.sh` exists and is designed to catch exactly this pattern, but **is not run in CI**. Recommendation to Phase 3: wire it.

---

## ShellError

**Current state (executed inspection):**

```rust
// crates/bins/desktop-shell/src/lib.rs:59-67
#[serde(tag = "kind", content = "message")]
#[serde(rename_all = "camelCase")]
enum ShellError { Command(String), Query(String), Storage(String),
                  InvalidArgument(String), Auth(String) }
```

`ShellError` crosses the Tauri IPC boundary into the desktop React UI. It is **not** part of the frozen OpenAPI HTTP contract — it is a Tauri-local transport.

**Two distinct problems found:**

1. **Rust emits English prose that the UI matches by exact string equality:**
   - `desktop-shell/src/lib.rs:80` → `ShellError::Auth("Invalid username or password")`
   - `crates/bins/desktop-shell/ui/src/pages/Login.tsx:250` → `return error.message === "Invalid username or password";`
   - `Login.tsx:72` → `setError("Invalid username or password.")` (hardcoded English fallback)

   If anyone localizes the Rust string, the credential-rejection branch breaks and every user sees the "could not reach server" message instead.

2. **`ShellError` already has the stable code Phase 0.5 would add.** `#[serde(tag = "kind")]` produces `kind: "auth" | "command" | "query" | "storage" | "invalidArgument"`, and the UI already branches on `shellError?.kind === "auth"` (`Login.tsx:66,71`). The one prose-matching site (`Login.tsx:250`) is the sole exception.

**DECISION — DR-7: RESOLVED. Do NOT migrate `ShellError` to `{code, params}`. It already has the required stable discriminator.**

Owning phase: **Phase 8** (Rust/native presentation boundary), and the only change needed is narrow:

- Replace `Login.tsx:250`'s message equality with a `kind`-based check (or a new finer-grained discriminator if "auth" conflates credential rejection with transport failure — that conflation is the actual defect).
- Leave `message` carrying prose for logs; mark it diagnostics-only.
- **Zero wire-shape change.** `ShellError` is not in `openapi.json`.

**This does not block Phase 1.** `ShellError` is not touched by canonical-source, key-format, registry, or schema work.

---

## CI Ordering

**DECISION — the localization contract gate must be a standalone job with no `needs:`. IMPLEMENTED AND VERIFIED in Phase 0.5.**

Rationale: `node scripts/sync-i18n.mjs --check` validates only `shared/i18n/{en,fa}.json` and the generated `*/src/i18n/` artifacts. It has **no dependency** on any frontend compiling, linting, or passing tests. It previously ran as step 10 of the `web` job, after lint, type-check, and unit tests — so an unrelated frontend regression skipped it entirely. Executed evidence: the gate **did not run** for the entire i18n commit series (`f487592`, `67409f5`, `d69604a`), because the `web` job failed earlier at each.

Change made: new `i18n` job in `.github/workflows/ci.yml`, `needs:` absent. Verified green in run `36577581791` (steps 4 and 5 both `success`).

**DECISION — the `web` job retains its own i18n step.** Deliberate redundancy; harmless; preserves the audit in the frontend context. The standalone job is authoritative.

**DECISION — remaining CI work is deferred to Phase 3** per the plan's phase ownership, specifically:
- `scripts/ci-pipeline.sh` still contains **no** i18n step, and its gate set diverges from `ci.yml` in both directions (it runs `bundle-check` and not `lint`/`test:browser`). Align them in Phase 3.
- Add Android `values`/`values-fa` parity check in Phase 2.
- Add interpolation-parity check to `sync-i18n.mjs` in Phase 1.
- Add unresolved-key check in Phase 1.
- Wire `verify_error_exhaustiveness.sh` into CI (Phase 3).

---

## Phase Ownership

| Item | Owning phase | Rationale |
|---|---|---|
| Locale registry (`locales.json`), read direction from it | **Phase 1** | Precondition for a third locale |
| `translation.schema.json` + CI schema check | **Phase 1** | Makes malformed catalogs fail CI |
| Interpolation-parity check | **Phase 1** | Passes today (0 mismatches); locks it in |
| Unresolved-key check + non-leaking `t()` fallback | **Phase 1** | Stops raw keys reaching users |
| Formatting abstraction (`formatDate`/`Number`/`RelativeTime`/…) | **Phase 1** | Must exist before surfaces migrate |
| Pluralization support; remove 3 hardcoded English plural sites | **Phase 1** | Plan §10 forbids per-component plural rules |
| Fix `approvals.reviewApproval`, add `common.total_items` | **Phase 1** | Closes the two dangling keys |
| Mount `LanguageSwitcher` in mobile-pwa / desktop-shell / admin-shell | **Phase 1** | `fa` is currently unreachable in 3 of 4 apps |
| `i18n.boundaries.json` surface registry | **Phase 1** | Autonomous migration queue |
| `shared/i18n/README.md` key conventions | **Phase 1** | Documents the ratified convention |
| Android `strings.xml` generation | **Phase 2** | Requires a name-mapping layer |
| Per-app RTL layer generation | **Phase 2 / 9** | Phase 9 for visual mirroring |
| Rust catalog loader | **Phase 2** | Nothing in Rust reads a catalog today |
| CI baseline (`i18n-baseline.json`), audit scope widening | **Phase 3** | Plan §21–22 |
| `i18n:report` JSON output; `ci-pipeline.sh` alignment | **Phase 3** | Plan §29 |
| `mobile-pwa` wired into CI | **Before Phase 5** | 44 dormant string selectors will fail at once otherwise |
| `NotificationAggregate` → code+params | **Phase 7** | DR-1 |
| Web Push rendering model | **Phase 7** | DR-3, coupled to DR-1 |
| English-substring → typed error matching | **Phase 8** | C3; correctness prerequisite |
| `safe_details.message` prose removal | **Phase 8** | Requires contract amendment |
| `ShellError` consumer fix | **Phase 8** | DR-7 |
| Farsi font embedding | **Deferred** | Licensing decision unresolved |
| Glossary / terminology | **Deferred** | Plan §39, Localization Quality Program |

---

## Blocking Issues

**BLOCKER B-1 — `mobile-android-kotlin` Gradle assembleDebug fails. Root cause NOT DETERMINED.**

- **Verified (executed):** failed at `f487592`, `67409f5`, `d69604a`, `f74832b`. Green at `644e1596`.
- **Verified (executed):** the only Android changes in that window are `LocaleHelper.kt`, `values/strings.xml`, `values-fa/strings.xml`, all added by `f487592`.
- **Verified (executed, static):** all three are well-formed XML; all 41 resource names in each file match `[a-zA-Z_][a-zA-Z0-9_]*`; no duplicate names; no unescaped `&`/`<` in values; `LocaleHelper.kt` imports `java.util.Locale` and only calls APIs present at minSdk 29.
- **NOT VERIFIED:** root cause. Job logs require repository admin rights; the download endpoint returned HTTP 403.
- **Impact on Phase 1: NONE.** Phase 1 does not touch `mobile-android/`.
- **Required action:** obtain admin access to CI job logs, or reproduce on a runner, before Phase 2 (Android generation). **Owner: Phase 2.**

**BLOCKER B-2 — `web-ui` feature-scope audit rejects the i18n locale persistence. A genuine conflict with a binding ruling; NOT resolved by Phase 0.5.**

- **Verified (executed, from CI artifact `onyx-web-quality-f74832b5749c888c44441203904282e18ee7e5c4`, `test-reports/junit.xml`):** the single failing test is `tests/feature-audit/excluded-features.test.ts :: v1 feature scope audit > contains no excluded feature implementation`, `expected [ Array(1) ] to deeply equal []`.
- **Verified (executed):** the one violation is `web-ui/src/i18n/I18nContext.tsx:50` → `localStorage.setItem(LOCALE_STORAGE_KEY, locale);`
- **The conflict:** `web-ui/tests/feature-audit/excluded-features.test.ts:21` bans `/localStorage\.setItem/i` anywhere under `web-ui/src/`. That ban implements **DECISIONS.md T6-D12** (binding): *"no optimistic mutation, persisted mutation, or automatic offline replay"* — i.e. the thin-client contract forbids persisting **operational domain state** in web `localStorage`.
- **Why it is a conflict and not a bug in the audit:** a UI locale preference is not operational domain state, and migration plan §14 requires locale persistence. The audit's regex over-broadly implements T6-D12.
- **Why Phase 0.5 did not fix it:** amending a governance-derived audit requires an explicit ruling. My operating contract forbids silently overriding existing project decisions, and the correct remedy is not unique (whitelist the locale key vs. narrow the regex to domain keys vs. move the call). This is a decision for the project owner.
- **Impact on Phase 1: NONE for the i18n design.** Phase 1's canonical-source, key-format, registry, schema, and check work all verify through the green `i18n` job. But see *Phase 1 Contract* constraint P1-7.
- **Recommended resolution (one line, once approved):** add `'i18n/I18nContext.tsx'` to an allow-list in the audit, or scope the regex to exclude the locale key.

---

## Deferred Decisions

| ID | Decision | Owning phase | Why it does not block Phase 1 |
|---|---|---|---|
| DR-1 (impl) | `NotificationAggregate` JSON shape | Phase 7 | Direction is fixed now; no Phase 1 file touches it |
| DR-2 | Is `migration-tool` CLI output in scope? | Phase 6 | Phase 1 does not touch the CLI |
| DR-3 | Web Push server-pre-render vs client-pre-render | Phase 7 | Depends on DR-1's shape |
| DR-5 | Per-app RTL generation mechanism | Phase 2/9 | Phase 1 retains the shared file unchanged |
| DR-6 | Normalize domain-enum wire casing (`Draft` → `draft`) | Phase 8 or DEFER | Not required for localization; would change a wire contract |
| DR-7 (impl) | `ShellError` consumer fix | Phase 8 | Not touched by Phase 1 |
| U-2 | `docs/DECISIONS.md` — **RESOLVED, see report §4** | — | Read in full this phase; no conflicting decision found |
| U-3 | `docs/governance/` mobile documents | Phase 7 | Android locale work is Phase 7 |
| U-4 | Farsi font licensing | Deferred | Typography is Phase 9 |
| U-5 | Rust localization surface | Phase 2 | No Phase 1 file touches Rust |
| U-6 | Do visual snapshots survive RTL/font change? | Phase 9 | Playwright snapshots are Phase 4+ |
| U-7 | Is `platform-contracts-ext` a live duplicate? | Phase 3 | Not localization-critical |

---

## Phase 1 Contract

Phase 1 is expected to implement **exactly** the following. Nothing else.

**P1-1 — Create `shared/i18n/metadata/locales.json`** with `direction`, `default`, `displayName`, `formatLocale` per locale (§Locale Registry).

**P1-2 — Make direction read from the registry.** Remove the hardcoded `locale === "fa" ? "rtl" : "ltr"` from `shared/i18n/react/I18nContext.tsx` (lines 47-48 and 63) and source it from `locales.json`. Regenerate all four platform copies via `sync-i18n.mjs`.

**P1-3 — Create `shared/i18n/schema/translation.schema.json`** enforcing the ratified convention: exactly 2 segments, namespace ∈ the 14 known namespaces, lowerCamelCase leaf, snake_case prohibited, string values.

**P1-4 — Add three checks to `scripts/sync-i18n.mjs`:** (a) schema validation of both catalogs; (b) interpolation-placeholder parity; (c) unresolved-key detection — scan all `t('…')` call sites across the four React apps and fail if any literal key is absent from `en.json`.

**P1-5 — Fix the missing-key fallback.** In `I18nContext.tsx`, `t()` must not render a raw key. Resolution order stays active-locale → English; unresolved renders empty, with CI (P1-4c) as the detector.

**P1-6 — Build the formatting abstraction** (`formatDate`, `formatDateTime`, `formatRelativeTime`, `formatNumber`, `formatPercent`, `formatCurrency`, `formatDuration`), resolving locale from `locales.json.formatLocale`. **Do not** route the 21 existing bare `toLocaleString()` call sites — that is surface migration (Phase 4+). Phase 1 provides and unit-tests the API only.

**P1-7 — Respect the thin-client ruling T6-D12.** Do **not** introduce any new `localStorage`/`sessionStorage` write under `web-ui/src/`. Locale persistence via `I18nContext.tsx` is the one sanctioned exception and its status is pending BLOCKER B-2. Do not add a second persistence mechanism.

**P1-8 — Build pluralization support** in `I18nContext.tsx` with locale plural categories. Do **not** refactor the 3 existing hardcoded English plural sites (`web-ui/.../TodoTargets/ListCard.tsx:50`, `desktop-shell/ui/.../MainLayout.tsx:218`, `mobile-android/.../AppShell.kt:131`) — those are surface migration. Phase 1 provides the capability.

**P1-9 — Mount `LanguageSwitcher`** in `mobile-pwa`, `crates/bins/desktop-shell/ui`, and `crates/bins/admin-shell/ui`. `fa` is currently unreachable in all three.

**P1-10 — Resolve the two dangling keys.** Repoint `approvals.reviewApproval` → `missions.reviewApproval`; add `common.total_items` to both catalogs.

**P1-11 — Create `i18n.boundaries.json`** with every surface at `unmanaged`, per plan §36.

**P1-12 — Create `shared/i18n/README.md`** documenting the ratified key convention, the canonical/generated split, and the canonical commands.

**P1-13 — Wire the new checks into the standalone `i18n` CI job** (added in Phase 0.5), so they run independently of the `web` job.

**Out of scope for Phase 1 — do not do these:** migrate user-facing strings in any surface; touch `mobile-android/`; touch Rust; add Android generation; create `metadata/glossary.json`; widen the literal-audit scope beyond `web-ui/src/pages/Dashboard`; change the OpenAPI error contract; modify `ShellError`; modify `NotificationAggregate`; perform visual RTL or typography work; improve Farsi wording.

**Verification obligation for Phase 1:** every change must be validated by GitHub Actions, specifically the standalone `i18n` job, which is now proven to run independently. Do not substitute local execution.
