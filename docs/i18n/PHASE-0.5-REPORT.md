# Phase 0.5 — Baseline Verification & Architecture Decisions

**Document type:** Baseline verification, architecture decision record, and Phase 1 contract
**Phase:** 0.5
**Verified against:** `main` @ `f74832b` — CI run [`36577581791`](https://github.com/SMozaff/Onyx-Framework/actions/runs/36577581791)
**Historical runs consulted:** [`36572636871`](https://github.com/SMozaff/Onyx-Framework/actions/runs/36572636871) (`d69604a`), [`36466833937`](https://github.com/SMozaff/Onyx-Framework/actions/runs/36466833937) (`67409f5`), [`36465446630`](https://github.com/SMozaff/Onyx-Framework/actions/runs/36465446630) (`f487592`), [`36268538073`](https://github.com/SMozaff/Onyx-Framework/actions/runs/36268538073) (`644e1596`)
**Companion:** `docs/i18n/PHASE-0.5-DECISIONS.md` (authoritative decision record)
**Status:** Phase 0.5 complete. Phase 1 not started.

---

## Executive Summary

Phase 0's central hypothesis was **correct and is now verified**: ONYX already has a working localization system, and Phase 1 must extend it rather than replace it. The canonical source (`shared/i18n/`, 149 flat keys) is verified green by GitHub Actions and required no changes.

**What Phase 0 got wrong, and what this phase corrected:**

| Phase 0 claim | Phase 0.5 finding |
|---|---|
| Notification schema change is gated by `verify_serialization.sh` golden fixtures | **FALSE.** That script fails *by design*; `tests/golden/` does not exist and it never runs in CI. No fixture gate exists. |
| Notification change needs a persistence/schema migration | **FALSE.** Notifications are a JSONB blob in the **generic `aggregates` table**. No `notifications` table exists. It is a JSON shape change, not DDL. |
| "`web-ui` likely does not compile" (from static `no-undef` inference) | **Partly unprovable.** Type-check never ran at `d69604a` — lint failed first. Now verified: lint and type-check both pass after the fix. |
| i18n gate blocked by type-check ordering | **TRUE but misattributed.** The gate was blocked by *lint* at `d69604a`, by *unit tests* at `f487592`, and by both thereafter. |

**Two baseline defects were found and one was repaired.** The remaining red gate was traced to its exact root cause.

**The most important discovery is not a defect but a conflict.** The `web-ui` feature-scope audit — which implements binding ruling **T6-D12** (no persisted state in the thin client) — fails on the i18n locale persistence. This is a genuine conflict between two governing documents, and Phase 0.5 deliberately did **not** resolve it unilaterally.

**DECISION — Phase 1 is safe to begin with documented exceptions.** Every architecture question Phase 1 could have answered by guessing is now answered by evidence.

---

## Phase 0 Findings Re-verified

| # | Phase 0 finding | Verdict | Evidence |
|---|---|---|---|
| F1 | `shared/i18n/{en,fa}.json`, 149 keys each | **CONFIRMED** | 149 keys each; CI run `36577581791` job `i18n` step 4 `success` |
| F1 | Parity enforcement exists | **CONFIRMED** | `sync-i18n.mjs:22-31`; CI step 4 green |
| F1 | Hand-rolled `LocaleProvider` | **CONFIRMED** | `shared/i18n/react/I18nContext.tsx`, mounted at all four app roots |
| F1 | RTL CSS | **CONFIRMED, and refined** | One shared file; 2/17 rules effective in 3 of 4 apps → now DECIDED as hybrid |
| F1 | Deterministic `sync-i18n.mjs` | **CONFIRMED** | Four generated dictionaries byte-identical; CI step 4 green |
| F1 | CI validation exists | **CONFIRMED but was inert** | It never executed for the entire i18n commit series |
| F1 | ~38 distinct `t()` keys consumed | **CONFIRMED** | 38 distinct literal keys extracted across 4 apps |
| **F1-a** | **Missing `useI18n()` imports break the build** | **PARTLY WRONG** | Lint did fail (bisection-proven). Type-check was never reached at `d69604a`, so the TS failure was never actually observed |
| F2 | `NotificationAggregate` holds English prose in replicated state | **CONFIRMED** | `notification-domain/src/lib.rs:13-33` |
| **F2-a** | **…and that change is gated by golden fixtures** | **FALSE — CORRECTED** | `verify_serialization.sh` header: *"this check fails, on purpose"*; `tests/golden/` absent; not in CI |
| **F2-b** | **…and needs a persistence migration** | **FALSE — CORRECTED** | `job_runner.rs:400-406` inserts into generic `aggregates`; no `notifications` table in any migration |
| F3 | English substring matching is load-bearing | **CONFIRMED** | `command.rs:1348,1357` |
| F4 | Two dangling keys render raw text | **CONFIRMED** | `I18nContext.tsx:63`; `approvals.reviewApproval`, `common.total_items` absent from both catalogs |
| F5 | Android: 0 of 41 keys referenced; `LocaleHelper` dead | **CONFIRMED** | `LocaleHelper` has exactly 2 self-references; `grep -rc "stringResource\|R\.string\."` → 0 |
| F6 | Manifest D1: `mobile-pwa/` absent | **CONFIRMED FALSE** | `mobile-pwa/` exists, 40 src files, and is **absent from CI entirely** |
| F7 | Manifest D2: Flutter `mobile/` present | **CONFIRMED FALSE** | `ls mobile` → absent |
| F8 | `ShellError` consumed by exact English match | **CONFIRMED and refined** | `Login.tsx:250` matches `error.message`; but `ShellError` **already has** `#[serde(tag="kind")]` — a stable discriminator |
| F9 | No font file exists anywhere | **CONFIRMED** | Zero `.ttf`/`.woff`/`.otf` in repository |
| **F10** | **`docs/DECISIONS.md` may contain conflicting i18n decisions** | **NONE FOUND** | See below |

---

## Web Baseline

### A. Does Web compile / type-check?

**Classification: `VERIFIED GREEN` (after Phase 0.5 repair).**

| Commit | Run | Lint | Type check | Unit tests | i18n gate |
|---|---|---|---|---|---|
| `644e1596` | 36268538073 | success | success | success | success |
| `f487592` | 36465446630 | **success** | success | **FAILURE** | skipped |
| `67409f5` | 36466833937 | **FAILURE** | skipped | skipped | skipped |
| `d69604a` | 36572636871 | **FAILURE** | skipped | skipped | skipped |
| `f74832b` | **36577581791** | **success** | **success** | **FAILURE** | standalone job **success** |

This is a clean **bisection** of the lint regression: `web` Lint passed at `f487592` and failed at `67409f5`. That commit's complete diffstat includes exactly three one-line additions to `web-ui/src/**`:

```
web-ui/src/pages/Approvals/index.tsx       |  1 +
web-ui/src/pages/Notifications/index.tsx   |  1 +
web-ui/src/pages/Tasks/index.tsx           |  1 +
```

all of the form `const { t } = useI18n();`. Executed static confirmation: **`t` is never called in any of the three files** (0 occurrences each), and `useI18n` is not imported in any of them. Under `web-ui/.eslintrc.cjs` that is two errors per file — `no-undef` (from `eslint:recommended`, which the config does not disable for TS) and `@typescript-eslint/no-unused-vars` — and `npm run lint` runs with `--max-warnings=0`.

**Note on Phase 0's claim:** at `d69604a` type-check was **skipped**, so Phase 0's statement that web-ui "does not compile" was a static inference that CI never confirmed. It is now moot: lint and type-check both pass at `f74832b`. The lint evidence is nonetheless strong corroboration that the defect was real, since `no-undef` and TS2304 would have fired on the same unresolved identifier.

**Additional finding — the unit-test failure has an independent, earlier origin.** At `f487592` lint *passed* and tests *failed*. So there are two distinct regressions in the i18n series, not one. See *CI Architecture* below.

### B. Is the i18n CI gate reachable?

**Was: no. Now: yes.**

Executed proof of non-reachability — the gate is step 10 of the `web` job, sequenced after lint (5), type-check (6), and unit tests (7). In runs `36465446630`, `36466833937`, and `36572636871`, steps 10-13 were all `skipped`. **The localization gate has not executed once since the i18n work began.**

The gate has no dependency on any of those steps: `sync-i18n.mjs` reads only `shared/i18n/{en,fa}.json` and the generated `*/src/i18n/` artifacts.

**DECISION — extract the gate into a standalone `i18n` job with no `needs:`.** Implemented in Phase 0.5 and **verified**: run `36577581791` job `i18n`, steps 4 and 5 both `success`.

### C. Baseline classification

**`VERIFIED RED — i18n-related`, with the localization contract sub-gate now `VERIFIED GREEN`.**

Full job-level result for run `36577581791` on `f74832b`:

| Job | Conclusion |
|---|---|
| `i18n` (new) | **success** |
| `check` (Rust) | **success** |
| `deploy-check` | **success** |
| `web` | **failure** — unit tests (BLOCKER B-2) |
| `mobile-android-kotlin` | **failure** — Gradle assembleDebug (BLOCKER B-1) |
| `native-ui-evidence` | skipped — declares `needs: web` |
| `load-smoke` | not completed at time of writing; unrelated to localization |

- Localization contract gate: **VERIFIED GREEN** (standalone, CI-executed).
- `web` Lint + type-check: **VERIFIED GREEN** (repaired).
- Rust workspace: **VERIFIED GREEN** — confirms the Phase 0.5 changes did not disturb the Rust side (they do not touch it).
- `web` unit tests: **VERIFIED RED — i18n-caused** (BLOCKER B-2).
- `mobile-android-kotlin` Gradle: **VERIFIED RED — cause undetermined** (BLOCKER B-1).

---

## docs/DECISIONS.md Findings

**DECISION — `docs/DECISIONS.md` (2,848 lines) contains no localization, i18n, translation, locale, Persian/Farsi, RTL, language, or typography decision. Phase 0's U-2 is resolved: nothing there conflicts with the Phase 0.5 canonical-source recommendation.**

Executed search across the full document for `i18n|locali[sz]|translat|locale|persian|farsi|RTL|language|font|typography|bilingual` returned exactly **one** hit — and it is a false positive (`"return type, translated to UnitOfWorkError::CommitFailed"` at line 1169).

### Rulings that *do* constrain Phase 1

Four binding rulings materially shape this phase. None is about localization; all constrain it.

**RULING E1 — `DomainError` Serialization Shape** (Increment 1, binding).
Requires `#[serde(tag = "code", content = "safe_details")]` with per-variant `#[serde(rename = "SCREAMING_CASE")]`, with `category` and `retryability` as **computed methods**, not stored fields, and `correlation_id` attached by a `DomainErrorResponse` wrapper at the API boundary. 8 codes with ruled retryability and category tables.

**This is exactly the architecture the migration plan §19 wants.** Phase 0.5 did not need to design an error scheme — it already exists and is ruled. Phase 0 under-reported this as merely "a good precedent to reuse"; it is in fact the *binding* shape.

**RULING T6-R1 — OpenAPI freeze** (Team 6, 2026-08-05, binding).
*"Team 6 owns the OpenAPI freeze. `docs/api/openapi.json`, version 1.0.0."*

Executed confirmation: `CommandError.required` = `["code","category","retryability","safe_details","correlation_id"]`, with `category` and `retryability` as closed enums. `web-ui/tests/unit/openapi-contract.test.ts` asserts the frozen required-field lists and runs in CI.

**CONSEQUENCE — DECISION: the HTTP error wire shape is frozen. No phase may change it.** Presentation maps from `error.code`. This removes any latitude Phase 1 might otherwise have had over the error contract.

**RULING T6-D12 — thin-client contract, no persisted state.**
*"all Team 6 queries, login mutations, and operational mutations set `networkMode: "always"`… there is no optimistic mutation, persisted mutation, or automatic offline replay."*

**This is what BLOCKER B-2 collides with.** See below.

**RULING T6-D1 — `NotificationAggregate` placement** (superseded).
Ruled that `NotificationAggregate` would be *"defined inside Team 6's command route module."* Executed verification: the **only** definition in the workspace is `crates/domains/notification-domain/src/lib.rs:15`; the API route module defines none. **The crate-based implementation superseded this ruling in practice.** Phase 0.5 records this rather than treating T6-D1 as binding.

**RULING W1 — wire protocol `DomainEventEnvelope` ↔ `SyncMessage`** (Team 5, binding).
Relevant to DR-1: notifications replicate through this path, so a `NotificationAggregate` shape change is a replicated-state concern and must remain wire-compatible during any transition window.

**RULING (Document Hierarchy)** — governs future ambiguity:
1. `team_prompts/PROMPT_X.md` — authoritative
2. `onyx_handover_v1.0.yaml` — authoritative
3. Blueprint docx — reference
4. Increment docs — historical

Executed check: **neither `team_prompts/` nor `onyx_handover_v1.0.yaml` exists in this repository.** The authoritative sources the hierarchy names are absent. **INFERENCE (medium-high): the migration plan and this Phase 0.5 record are, in practice, the governing documents for localization architecture** — which is why this phase resolves explicitly rather than deferring to a hierarchy that cannot be consulted.

---

## Canonical Localization Architecture

**DECISION — retain `shared/i18n/` as canonical, in place.**

```
shared/i18n/{en,fa}.json         ← CANONICAL (hand-authored, 149 keys)
shared/i18n/react/*              ← CANONICAL runtime
scripts/sync-i18n.mjs            ← CANONICAL generator/auditor
*/src/i18n/*                     ← GENERATED (4 apps × 4 files, never hand-edit)
mobile-android/.../strings.xml   ← NON-CANONICAL second catalog → generated in Phase 2
Rust                             ← no catalog exists; Phase 2
```

Phase 1 adds, without relocating anything: `schema/translation.schema.json`, `metadata/locales.json`, `README.md`, and `i18n.boundaries.json`.

**DECISION — no competing system is introduced. No third-party i18n library is added.** Executed re-confirmation: zero hits for `i18next|react-intl|lingui|FormattedMessage|IntlProvider` across all four frontends; zero i18n entries in any `Cargo.toml`.

---

## Flat vs Nested Keys

**DECISION — RETAIN FLAT KEYS.** The established ONYX convention is 100% consistent and must be ratified, not replaced.

Executed analysis of all 149 keys:

| Property | Result |
|---|---|
| Segment count | **149/149 are exactly 2 segments.** Zero exceptions. |
| Namespaces | 14: `app, nav, auth, common, status, dashboard, missions, tasks, notifications, approvals, reports, files, settings, language` |
| Leaf casing | **149/149 lowerCamelCase.** Zero snake_case keys exist. |
| Value types | **149/149 strings.** Zero non-string values. |

Ratified convention: `<namespace>.<lowerCamelCaseLeaf>` — e.g. `common.save`, `status.pending`, `missions.noMissionsBody`.

**DECISION — this DECIDES AGAINST the migration plan's illustrations.** Plan §4.2/§8 show `common.actions.save`, `missions.status.pending`, `errors.permission_denied`, `notifications.connection_lost`. ONYX has `common.save`, `status.pending`, and no `errors` namespace at all. Phase 1 keeps ONYX's convention. Renaming 149 keys across 16 generated files and 38 call sites to match a document illustration is a large, behavior-neutral, high-risk change with no functional benefit, and would violate "do not invent a new naming scheme if one already exists."

Evidence supporting retention over nesting:

- Migration cost of converting: 16 generated files, 38 call sites, plus the CI parity check rewritten — **all for zero functional gain**.
- Determinism is already proven: four generated dictionaries byte-identical; `sync-i18n.mjs --check` green in CI.
- Flat JSON is directly consumable from Rust via `include_str!` with **zero dependencies**; a nested catalog would require a traversal library or a bespoke walker.
- Nesting buys authoring ergonomics only, and is a maintainer-preference question, not a correctness one.

**DECISION — Android resource names are derived, not redefined:** `missions.status.pending` → `missions_status_pending`. Deterministic and lossless.

---

## Notification Architecture

**DECISION — the code+params direction is correct. The implementation is Phase 7. The cost is substantially lower than Phase 0 estimated.**

Two Phase 0 conclusions were factually wrong and are corrected here:

1. **No golden-fixture gate exists.** `scripts/verify/verify_serialization.sh` documents in its own header that it *"fails, on purpose"* because `tests/golden/` does not exist and no `--test golden_fixtures` harness exists. Executed: `ls tests/golden` → absent; `grep golden_fixtures crates/` → no hits. The script is not run in CI.
2. **No persistence migration is needed.** Executed: `job_runner.rs:400-406` runs `INSERT INTO aggregates (id, aggregate_type, organization_id, version, lifecycle_epoch, authority_epoch, state, updated_at) VALUES ($1, 'notification', …)`. There is **no `notifications` table** in any SQLite or Postgres migration. Notification state is a JSONB blob. Changing its shape is a **JSON change, not DDL**.

Target architecture (unchanged from the plan, now confirmed against evidence):

```
Domain event:  { code: "staff_loan.ending_soon", params: {…}, metadata: {…} }
     ↓  (replicated via W1 wire format)
Presentation:  code → translation key
     ↓
Rendering:     localized per client locale
```

Compatibility requirements for Phase 7, recorded so the implementing agent does not have to re-derive them:

- **Additive.** `#[serde(default)] title/message: Option<String>` must still deserialize legacy rows, falling back to a `notification.legacy` code.
- **Replicated-state.** Notifications sync to peers via the W1 path; both shapes must coexist during a transition window.
- `priority` and `status` are bare `String` today, compared by string literal (`notification-domain/src/lib.rs:82,94`). Converting to enums belongs to the same Phase 7 change.
- Consumers to update together: `job_runner.rs:205-209,273-276`; `push_delivery.rs:323-325`; every client notification list.

**DR-3 (Web Push) remains coupled to DR-1 and is explicitly NOT decided in Phase 0.5.** Server-side per-locale pre-rendering and client-side pre-rendering from `code+params` are both architecturally defensible; the choice depends on DR-1's final shape. Deferring without guessing.

---

## Error Architecture

**DECISION — machine code at the wire, localized text at the presentation edge. The wire shape does not change, because it is frozen by binding ruling T6-R1.**

Phase 0 correctly identified the hazard. Executed confirmation:

```rust
// crates/bins/api-server/src/routes/command.rs:1348
crate::CommandError::Domain(message) if message.contains("not permitted") => …
// crates/bins/api-server/src/routes/command.rs:1357
if message.contains("already acknowledged") || message.contains("not pending") => …
```

These select HTTP status and error code. Rewording any domain `#[error("…")]` silently changes API behaviour.

**DECISION — the minimal safe refactor is to match typed `DomainError` variants instead of substrings, and it changes no wire shape.** The 8 codes and their mapping tables are already ruled (E1). Assigned to **Phase 8**, and it is a **correctness prerequisite that must precede any error-prose work**.

**DECISION — `safe_details.message` prose removal is out of Phase 1 and out of Phase 0.5's authority.** Executed: 27 distinct hardcoded English literals plus unbounded dynamic prose ride in `safe_details.message` alongside the code. Removing them changes wire semantics and would require an explicit contract amendment to `openapi.json` under T6-R1. Assigned to Phase 8.

**DECISION — no error code registry is introduced in Phase 1.** 57 codes are bare literals at 77 call sites; a registry is a Phase 8 concern.

**Noted but explicitly out of scope:** `security-adapter/src/user_store.rs:37,108` substring-match **Postgres driver** messages. Not localization-related; must not be touched under the i18n migration.

---

## ShellError

**DECISION — RESOLVED. `ShellError` does not need migration to `{code, params}`. It already has a stable discriminator.**

Executed finding that changes Phase 0's framing: `ShellError` is declared

```rust
#[serde(tag = "kind", content = "message")]
#[serde(rename_all = "camelCase")]
enum ShellError { Command(String), Query(String), Storage(String), InvalidArgument(String), Auth(String) }
```

`#[serde(tag = "kind")]` already emits `kind: "auth" | "command" | "query" | "storage" | "invalidArgument"` — a stable, language-neutral code. And the UI **already branches on it**: `Login.tsx:66,71` test `shellError?.kind === "auth"`.

The actual defect is narrower than Phase 0 described: **one** site matches prose.

```ts
// crates/bins/desktop-shell/ui/src/pages/Login.tsx:250
return error.message === "Invalid username or password";
```

against Rust-produced `ShellError::Auth("Invalid username or password")` (`lib.rs:80`). Localizing the Rust string would break credential-rejection detection and misroute users to the "could not reach server" message.

**DECISION — Phase 8, narrow fix:** replace the message equality with a `kind`-based check, introducing a finer discriminator if `auth` conflates credential rejection with transport failure (it does — that conflation is the real defect). Mark `message` diagnostics-only. **Zero wire-shape change; `ShellError` is not in `openapi.json`.**

**Does not block Phase 1** — Phase 1 does not touch `ShellError`.

---

## RTL Architecture

**DECISION — hybrid. A canonical direction contract plus per-platform adapters. Retire the single shared stylesheet as the *mechanism*, but not in Phase 1.**

Executed basis: `shared/i18n/react/rtl.css` is 68 lines, 17 `html[dir="rtl"]`-guarded rules, copied verbatim into all four React apps. Its effectiveness per app: `web-ui` ~13/17; `mobile-pwa`, `desktop-shell/ui`, `admin-shell/ui` **2/17** — the remaining rules target `.sidebar`, `.workspace`, `.toast`, `.skip-link`, `.data-row`, `.dialog-actions` and similar, which those apps do not use (they use `onyx-*` classes and Tailwind utilities).

A single shared stylesheet cannot serve both a BEM-CSS app and Tailwind apps. That is a structural fact, not a matter of taste.

Split of responsibilities:

- **Canonical contract:** `locales.json.direction` is the single authority for writing direction; `document.documentElement.dir` is set from it. This is what is shared.
- **Platform adapters:** each app owns its directional CSS/logic.

**DECISION — Phase 1 changes only the direction *source*, not the direction *implementation*.** `locales.json` becomes the single source; the existing `rtl.css` is retained unchanged. Per-app RTL generation is Phase 2/9; visual mirroring is Phase 9.

**DECISION — typography remains unwired and out of scope.** Executed: zero font binaries in the repository; `Vazirmatn` exists only as a CSS font-family *name*. Embedding requires a licensing decision (U-4) plus Phase 9.

---

## Missing Translation Behaviour

**Current behaviour (executed + statically verified):**

```ts
// shared/i18n/react/I18nContext.tsx:62-64
const hit = active[key] ?? fallback[key];
if (hit === undefined) return key;
```

Chain: active locale → English → **raw key rendered to the user, silently**. Two keys currently reach this path in production: `approvals.reviewApproval` (`web-ui/src/pages/Dashboard/index.tsx:17`) and `common.total_items` (`web-ui/src/pages/Missions/index.tsx:21`). In Farsi, users see the literal strings `approvals.reviewApproval` and `common.total_items` rendered on screen.

**DECISION — three-tier policy for Phase 1:**

1. **Resolution order unchanged** — active locale → English. English fallback is retained and is not an error.
2. **An unresolved key must never render raw.** `t()` returns `en[key] ?? ''` instead of the key.
3. **Development and production do not differ in fallback; they differ in detection.** In both, an unresolved key renders empty. Visibility comes from a CI check that fails on unresolved keys — runtime must not be the detector, and a user-facing page must never be.

**DECISION — the two dangling keys are fixed in Phase 1, not here.** `approvals.reviewApproval` → repoint to the existing `missions.reviewApproval` (no new translation). `common.total_items` → requires new Farsi copy, which is translation authoring and outside Phase 0.5.

---

## CI Architecture

Three distinct defects, all now characterized.

**Defect 1 — the localization gate was unreachable. REPAIRED AND VERIFIED.**

The gate validates only the canonical catalog and generated artifacts. It ran as step 10 of the `web` job, after lint, type-check, and unit tests. Executed: it was **skipped in all three** i18n-series runs. It had never executed.

Fix: standalone `i18n` job, no `needs:`. Verified green in run `36577581791` (steps 4 and 5 `success`).

**Defect 2 — lint regression. REPAIRED AND VERIFIED.**

Three dead `const { t } = useI18n();` lines, `t` never used, `useI18n` never imported. Bisection-proven: `web` Lint green at `f487592`, red at `67409f5`, and that commit added exactly those three lines to `web-ui/src/**`. Removed. `web` Lint and type-check now both pass.

**Defect 3 — feature-scope audit rejects the i18n locale persistence. NOT REPAIRED — BLOCKER B-2.**

Executed, from CI artifact `onyx-web-quality-f74832b5749c888c44441203904282e18ee7e5c4` (`test-reports/junit.xml`), retrieved via the GitHub API:

- 11 test suites, 145 tests. **Exactly 1 failure.**
- `tests/feature-audit/excluded-features.test.ts :: v1 feature scope audit > contains no excluded feature implementation` — `expected [ Array(1) ] to deeply equal []`.
- The single violation is `web-ui/src/i18n/I18nContext.tsx:50` → `localStorage.setItem(LOCALE_STORAGE_KEY, locale);`
- Confirmed by static scan: no other file under `web-ui/src` matches `localStorage.setItem`, and none of the other four excluded patterns (`OfflineQueue`, `uploadFile`, `createBlueprint`, `MeetingChat`) match anything. Exactly 1 violation, consistent with the report.

**The conflict.** `excluded-features.test.ts:21` bans `/localStorage\.setItem/i` anywhere under `web-ui/src/`. That ban implements **DECISIONS.md T6-D12**, which forbids *persisted mutation* in the thin client. A UI locale preference is not operational domain state, and migration plan §14 requires locale persistence. **The audit's regex over-broadly implements the ruling.**

**Why Phase 0.5 did not fix it.** Amending a governance-derived audit requires an explicit ruling. The correct remedy is not unique — whitelist the i18n directory, narrow the regex to domain keys, or relocate the write. My operating contract forbids silently overriding existing project decisions. This is surfaced as a decision for the project owner.

**Remaining CI work, deferred to Phase 3 per plan ownership:** `scripts/ci-pipeline.sh` still has no i18n step, and its gate set diverges from `ci.yml` in both directions. `verify_error_exhaustiveness.sh` — which would catch the C3 substring hazard — is not run in CI. Android `values`/`values-fa` parity is unenforced.

---

## Decisions Resolved

| ID | Decision | Basis |
|---|---|---|
| **DR-4** | **RETAIN FLAT KEYS**, 2-segment `<namespace>.<lowerCamelCase>`, snake_case prohibited | 149/149 consistent; no reason to churn 16 files + 38 call sites |
| **DR-7** | **ShellError needs no `{code, params}` migration** — `#[serde(tag="kind")]` is already a stable code; fix one prose-match site in Phase 8 | `lib.rs:59-67`, `Login.tsx:250` |
| **DR-1 (dir.)** | Notification target is `{code, params, metadata}`; implementation Phase 7 | Plan §19 + `notification-domain` shape |
| **DR-1 (cost)** | **CORRECTED** — no golden-fixture gate, no DDL migration | `verify_serialization.sh` header; `job_runner.rs:400-406` |
| **RTL** | **Hybrid** — canonical direction contract + per-platform adapters | 2/17 rule effectiveness in 3 apps |
| **Missing key** | Never render raw key; CI is the detector; English fallback retained | `I18nContext.tsx:63` |
| **Error wire** | **Frozen** — T6-R1 binds it; no phase may change it | `openapi.json` + `openapi-contract.test.ts` |
| **Canonical** | `shared/i18n/` retained in place; §5 sub-structure added additively | Plan §2 |
| **CI** | Standalone `i18n` job, no `needs:` — **implemented and verified** | Gate skipped in 3 consecutive runs |
| **U-2** | **`docs/DECISIONS.md` has no localization decision** | 1 false-positive hit in 2,848 lines |
| **U-1** | **RESOLVED**: web lint + type-check green; unit tests red from a different, earlier cause | Run `36577581791` |
| **T6-D1** | Superseded in practice by `notification-domain` | Single definition site |

## Decisions Deferred

| ID | Deferred to | Why not blocking |
|---|---|---|
| DR-1 (impl) | Phase 7 | Direction fixed; no Phase 1 file touches it |
| DR-2 (`migration-tool` CLI scope) | Phase 6 | Phase 1 does not touch the CLI |
| DR-3 (Web Push rendering) | Phase 7 | Coupled to DR-1's shape |
| DR-5 (per-app RTL generation) | Phase 2/9 | Phase 1 retains the shared file |
| DR-6 (enum wire casing) | Phase 8 / defer | Not required for localization |
| DR-7 (impl) | Phase 8 | Not touched by Phase 1 |
| U-3 (governance mobile docs) | Phase 7 | Android locale work is Phase 7 |
| U-4 (Farsi font licensing) | Deferred | Typography is Phase 9 |
| U-5 (Rust localization surface) | Phase 2 | No Phase 1 file touches Rust |
| U-6 (visual snapshots under RTL) | Phase 9 | Playwright snapshots are Phase 4+ |
| U-7 (`platform-contracts-ext` duplicate) | Phase 3 | Not localization-critical |

---

## Remaining Blockers

**BLOCKER B-1 — `mobile-android-kotlin` Gradle assembleDebug. Cause undetermined.**
Verified: fails at `f487592`, `67409f5`, `d69604a`, `f74832b`; green at `644e1596`. The only Android changes in that window are `LocaleHelper.kt`, `values/strings.xml`, `values-fa/strings.xml`. All three verified well-formed: valid XML, 41 valid unique resource names per file matching `[a-zA-Z_][a-zA-Z0-9_]*`, no unescaped `&`/`<`, no API above minSdk 29. **Root cause NOT VERIFIED** — job logs require admin rights; the download endpoint returned HTTP 403. **Impact on Phase 1: none.** Owner: Phase 2.

**BLOCKER B-2 — feature-scope audit vs. locale persistence. Genuine ruling conflict, not unilaterally resolved.**
Detailed above. **Impact on Phase 1: none for the i18n design**, but Phase 1 is constrained by contract item **P1-7**: introduce no new `localStorage`/`sessionStorage` write under `web-ui/src/`. Owner: project owner, before Phase 4 surface migration begins (which will need this resolved to move strings).

**Not blockers, recorded for traceability:** DR-1/DR-3/DR-5/DR-6/DR-7 implementations (owned, later phases); U-3…U-7 (owned, later phases); the two dangling keys (Phase 1 item P1-10).

---

## Production Code Changes

Two changes, both minimal, both recorded in commit `f74832b`.

### Change 1 — remove three dead `useI18n()` declarations (3 lines removed, 0 added)

| File | Line | Change |
|---|---|---|
| `web-ui/src/pages/Approvals/index.tsx` | 12 | removed `const { t } = useI18n();` |
| `web-ui/src/pages/Notifications/index.tsx` | 9 | removed `const { t } = useI18n();` |
| `web-ui/src/pages/Tasks/index.tsx` | 10 | removed `const { t } = useI18n();` |

**Why necessary:** bisection-proven CI lint regression; the declaration never had any effect (`t` was never called). Removing it restores the last-known-good state.
**Why it belongs in Phase 0.5:** Phase 0.5's mandate is to establish a verified baseline; a red baseline makes every later verification unreliable.
**Why it does not expand scope:** no user-visible string was changed, no component was migrated, no behavior altered. The imports will be re-added when these surfaces are actually migrated in Phase 4.
**Verification:** `web` Lint and Type check both `success` in run `36577581791`.

*Alternative considered and rejected:* adding the missing imports. It would not have fixed lint — `t` is unused, so `@typescript-eslint/no-unused-vars` would still fail. Completing the migration instead is Phase 4 work and was correctly excluded.

### Change 2 — add a standalone `i18n` CI job (22 lines added, 0 removed)

`.github/workflows/ci.yml`: new `i18n` job with no `needs:`, running `node scripts/sync-i18n.mjs --check` and `--check --audit-scope=web-ui/src/pages/Dashboard`.

**Why necessary:** the localization contract gate had never executed in three consecutive runs because it was sequenced behind frontend lint and tests it does not depend on.
**Why it belongs in Phase 0.5:** without it, Phase 1 has no enforceable gate and cannot be verified.
**Why it does not expand scope:** YAML configuration only. No application code, no string, no behavior. The existing `web`-job i18n step is retained deliberately as harmless redundancy.
**Verification:** job `i18n` `success` (steps 4 and 5) in run `36577581791`.

**Explicitly NOT changed:** no catalog, no generated file, no translation, no `NotificationAggregate`, no `ShellError`, no error contract, no `mobile-android/` file, no test file, no audit file. The `docs/api/openapi.json` freeze is untouched.

---

## Verification Performed

### EXECUTED (GitHub Actions)

| Verification | Run | Result |
|---|---|---|
| Standalone `i18n` job: `sync-i18n.mjs --check` | 36577581791 | **PASS** |
| Standalone `i18n` job: literal audit, `web-ui/src/pages/Dashboard` | 36577581791 | **PASS** |
| `web` lint (`--max-warnings=0`) | 36577581791 | **PASS** |
| `web` type check (`tsc -b`) | 36577581791 | **PASS** |
| `web` unit + integration tests | 36577581791 | **FAIL** — 1/145, `feature-audit` |
| Rust `check` job (fmt, clippy, build, tests, docs, contract verification) | 36577581791 | **PASS** |
| `deploy-check` job (Helm ×4, Terraform, Docker) | 36577581791 | **PASS** |
| `mobile-android-kotlin` Gradle assembleDebug | 36577581791 | **FAIL** — BLOCKER B-1 persists |
| CI job/step graph on `d69604a` | 36572636871 | web FAIL@Lint; i18n gate skipped |
| CI job/step graph on `67409f5` | 36466833937 | web FAIL@Lint |
| CI job/step graph on `f487592` | 36465446630 | web Lint PASS, FAIL@Tests; android FAIL |
| CI job/step graph on `644e1596` | 36268538073 | **ALL GREEN** |
| `junit.xml` from CI artifact | 36577581791 | 145 tests, 1 failure, root cause identified |

### STATICALLY VERIFIED (inspection, no execution)

`docs/DECISIONS.md` read in full (2,848 lines) — zero localization decisions; rulings E1, T6-R1, T6-D12, T6-D1, W1 and the Document Hierarchy extracted. Catalog structure (149 keys: segment counts, namespaces, casing, value types). Missing-key fallback path and both dangling keys. `NotificationAggregate` shape and its two producers and one Web Push consumer. JSONB `aggregates` storage and absence of a `notifications` table. `verify_serialization.sh` design intent and absence of `tests/golden/`. `ShellError` serde tagging and its single prose-match consumer. `openapi.json` `CommandError` schema and its CI assertion. Android resource validity (XML well-formedness, name syntax, uniqueness, escaping). ESLint config and which rules the three dead lines violated. `ci.yml` job dependency graph and step ordering. The `i18n` job's YAML validity and absence of `needs:`.

## Verification Not Performed

**NOT EXECUTED — no local build, type-check, lint, unit test, E2E test, Gradle, or Cargo command was run at any point in this phase.** All build and test verification was performed through GitHub Actions, per repository policy.

**NOT VERIFIED — `mobile-android-kotlin` Gradle failure root cause.** Job logs require repository admin rights; the logs endpoint returned HTTP 403. Static elimination of the three changed files was completed and ruled none of them out. **BLOCKER B-1 remains open.**

**NOT VERIFIED — whether `web-ui` type-check failed at `d69604a`.** Lint failed first, so type-check was never reached. Phase 0's static inference that it would fail was never CI-confirmed. Moot now: type-check passes at `f74832b`.

**NOT VERIFIED — the repository is NOT fully green at `f74832b`.** Five of six jobs completed: `i18n` **PASS**; Rust `check` **PASS**; `deploy-check` **PASS**; `web` **FAIL** (unit tests — BLOCKER B-2); `mobile-android-kotlin` **FAIL** (Gradle — BLOCKER B-1). `native-ui-evidence` was `skipped` because it declares `needs: web`, so B-2 suppresses it. `load-smoke` had not completed when this report was written; it is unrelated to localization and Phase 0.5 does not depend on it. **A reader must re-check the final run conclusion before treating the repository as green.**

**NOT VERIFIED — the correct remedy for BLOCKER B-2.** Deliberate: requires an explicit ruling.

**NOT APPLICABLE — U-6** (visual snapshot survival under RTL) requires Phase 9.

---

## Phase 1 Readiness

### SAFE TO BEGIN PHASE 1 WITH DOCUMENTED EXCEPTIONS

Every architecture question Phase 1 could have answered by guessing is now answered by evidence:

| Question | Answer | Status |
|---|---|---|
| Is there existing infrastructure? | Yes — `shared/i18n/`, verified green | DECIDED |
| Is the canonical source sound? | Yes; retain in place | DECIDED |
| Flat or nested keys? | Flat, 2-segment, ratified | DECIDED |
| Where does direction live? | New `locales.json` registry | DECIDED |
| What is generated vs authored? | 4 React targets generated; Android in Phase 2 | DECIDED |
| What happens on a missing key? | Never render raw; CI is the detector | DECIDED |
| RTL architecture? | Hybrid: canonical direction + per-platform adapters | DECIDED |
| Notification boundary? | `{code, params, metadata}`; Phase 7 | DECIDED (direction) |
| Error boundary? | Frozen by T6-R1; map from `error.code` | DECIDED |
| ShellError? | Already has a stable `kind` code; narrow fix in Phase 8 | DECIDED |
| CI ordering? | Standalone `i18n` job | **IMPLEMENTED + VERIFIED** |
| Does `docs/DECISIONS.md` conflict? | No localization decision exists | RESOLVED |

**The gate Phase 1 needs exists and is proven to run.** That was the single highest-risk unknown, and it is closed.

**Known non-blocking issues, explicitly assigned:**

1. **BLOCKER B-2** (feature-audit vs. locale persistence) — constrains Phase 1 via contract item **P1-7**: introduce no new browser-storage write under `web-ui/src/`. Must be resolved by the project owner before Phase 4 surface migration.
2. **BLOCKER B-1** (Android Gradle, cause unknown) — Phase 1 does not touch `mobile-android/`; must be resolved before Phase 2.
3. `web` unit tests remain red from B-2. Phase 1's changes verify through the standalone `i18n` job, which runs independently.

**What would have made this NOT SAFE:** an unverified or broken baseline (now verified green for the i18n gate); an undecided canonical source (now decided); an unresolved key-format question (now decided); a `docs/DECISIONS.md` conflict (none found); or notification/error boundaries left to guesswork (both decided in direction).

---

## Required Phase 1 Scope

**Authoritative list: `docs/i18n/PHASE-0.5-DECISIONS.md` → "Phase 1 Contract" (P1-1 … P1-13).**

In summary, Phase 1 must:

1. Create `shared/i18n/metadata/locales.json`; make direction read from it (P1-1, P1-2).
2. Create `shared/i18n/schema/translation.schema.json` enforcing the ratified 2-segment convention (P1-3).
3. Add three CI checks to `sync-i18n.mjs`: schema, interpolation parity, unresolved keys (P1-4).
4. Stop `t()` rendering raw keys (P1-5) and fix the two dangling keys (P1-10).
5. Build the formatting abstraction (P1-6) and pluralization support (P1-8) — **capability only, no call-site migration**.
6. Mount `LanguageSwitcher` in the three apps where `fa` is unreachable (P1-9).
7. Create `i18n.boundaries.json` (P1-11) and `shared/i18n/README.md` (P1-12).
8. Wire new checks into the standalone `i18n` job (P1-13).
9. Add **no** new browser-storage write under `web-ui/src/` (P1-7).

**Explicitly out of scope for Phase 1:** migrating user-facing strings; touching `mobile-android/`; touching Rust; Android generation; `metadata/glossary.json`; widening the literal-audit scope; changing the OpenAPI error contract; modifying `ShellError` or `NotificationAggregate`; RTL visual work; typography; Farsi wording.

**Verification obligation:** every Phase 1 change must be validated by GitHub Actions through the standalone `i18n` job. Local execution must not be substituted.

**Phase 1 has not been started.**
