# ONYX Multilingual Infrastructure — Phase 0 Discovery Report

**Document type:** Repository-grounded localization discovery and boundary audit
**Phase:** 0 — Discovery, Inventory, Boundary Audit
**Scope:** Read-only audit. No production code was modified.
**Repository:** `SMozaff/Onyx-Framework`
**Snapshot examined:** `main` @ `d69604a` ("Merge pull request #18 from SMozaff/feat/i18n-root-migration")
**Report date:** 2026-09-29
**Basis:** repository files, git history, CI configuration, and the two governing documents named in the mission brief
**Status:** Discovery complete. No implementation performed.

---

## 0. Evidence Discipline

This report separates two categories throughout:

| Label | Meaning |
|---|---|
| **FACT** | Directly verified from repository files at the cited path/line, or by executing a read-only command whose output is quoted. |
| **INFERENCE** | A reasoned conclusion drawn from FACTs. Confidence is stated. |
| **RECOMMENDATION** | A proposed action. Not implemented. Requires decision. |
| **DECISION REQUIRED** | Evidence is insufficient to proceed safely. A human or architectural owner must choose. |

Counts reported as numbers were produced by `rg`/scripted scans and re-verified where they materially affect conclusions. Where a claim was made by a delegated audit and mattered to a finding, it was independently re-checked and is marked as such.

**Known limits of this report:**

- `web-ui/node_modules/` is not installed. `tsc`, `eslint`, `vitest`, and `playwright` were **not** executed. This is required by the repository's own CI-only policy (`i18n-dual-language-report.md`, "Build / Test Policy"). Consequently, all TypeScript build-status statements below are **static source inspection**, not compiler output.
- Rust, Gradle, and Android builds were not executed for the same reason.
- The Android module has no connected-device/emulator evidence in the manifest; instrumented-test behavior is unverified.

---

## 1. Executive Summary

### 1.1 Headline finding

ONYX **already has a localization system**. It was introduced in commits `f487592` ("Dual lang"), `67409f5` ("Farsi"), and refined by `cb5b896`..`00831c8` (`ci: add scoped user-facing literal audit`, `ci: enforce dashboard i18n migration`). It is a **flat-key JSON dictionary pair, a hand-rolled React context, a CSS RTL overlay, a deterministic codegen script, and a CI gate scoped to one directory.**

This directly contradicts the migration plan's Phase 1 premise, which assumes a greenfield canonical foundation must be built. **FACT:** `shared/i18n/en.json` and `shared/i18n/fa.json` exist, contain **149 keys each**, pass key-parity, and generate deterministically into four frontends. `node scripts/sync-i18n.mjs --check` exits 0.

**FACT:** all four generated dictionary files are byte-identical (md5 `480f61e023a4851ff31d97091957a6fd`), and all twelve generated/copied i18n files match `shared/i18n/react/` exactly.

The migration plan §2 explicitly instructs: *"If ONYX already has an equivalent localization structure, extend and consolidate it rather than creating a competing system."* **This report therefore recommends extension, not replacement.**

### 1.2 The real problem is adoption, not supply

| Metric | Value | Evidence |
|---|---:|---|
| Canonical catalog keys | **149** | `shared/i18n/en.json`, `shared/i18n/fa.json` |
| `t()` calls — web-ui | 52 | `grep -rhoE "t\(['\"][a-zA-Z._]+['\"]" web-ui/src \| wc -l` |
| `t()` calls — mobile-pwa | **2** | same command on `mobile-pwa/src` |
| `t()` calls — desktop-shell/ui | **2** | same command |
| `t()` calls — admin-shell/ui | **2** | same command |
| `t()` calls — mobile-android | **0** | no `stringResource`, no `R.string.*` in any `.kt` |
| `t()` calls — Rust | **0** | no i18n crate, no catalog loader, no i18n dependency in any `Cargo.toml` |
| Language switcher rendered — web-ui | 2 files | `MainLayout.tsx`, `Sidebar.tsx` |
| Language switcher rendered — other 3 apps | **0** | `grep -rl LanguageSwitcher <app>/src --include=*.tsx` excluding `i18n/` returns nothing |
| Android `values-fa/strings.xml` keys referenced | **0 of 41** | `grep -rc "stringResource\|R\.string\." mobile-android/.../*.kt` → 0 |

**INFERENCE (high confidence):** three of the four frontends and the entire Android client have the localization infrastructure fully wired at the root and **zero** user-visible adoption. In mobile-pwa, desktop-shell, and admin-shell, the only two `t()` calls are inside `LanguageSwitcher.tsx` — a component that is **never mounted**. Effective shipped coverage in those three apps is **zero user-visible strings**.

### 1.3 Three findings that block or reshape the plan

**F1 — `web-ui` does not compile (static inspection).**
Three files call `useI18n()` with no import of it:

```
web-ui/src/pages/Approvals/index.tsx:12       const { t } = useI18n();
web-ui/src/pages/Notifications/index.tsx:9    const { t } = useI18n();
web-ui/src/pages/Tasks/index.tsx:10           const { t } = useI18n();
```

No global `declare` exists (`web-ui/src/vite-env.d.ts` contains only a Vite client reference). `web-ui/tsconfig.app.json` sets `"strict": true` and `"include": ["src", "tests"]`. Under these settings these are `TS2304 Cannot find name 'useI18n'` errors. **FACT (re-verified against `git show HEAD:`, so this is committed state, not a worktree artifact):** all three lines are in commit `67409f5`. **INFERENCE (high confidence, not compiler-confirmed):** `npm run type-check` in the CI `web` job is currently failing, and because `type-check` precedes the i18n check at `.github/workflows/ci.yml:213-214` vs `:221-223`, the i18n gate is unreachable. **This must be confirmed in CI before Phase 1 begins.** This is a pre-existing defect, not introduced by this audit.

**F2 — Two keys referenced in code do not exist in the catalog.**
Re-verified by extracting all 38 distinct literal keys from all four frontends and diffing against `en.json`:

```
MISSING FROM CATALOG: ['approvals.reviewApproval', 'common.total_items']
```

- `approvals.reviewApproval` — used at `web-ui/src/pages/Dashboard/index.tsx:17`. The catalog has `missions.reviewApproval` (`shared/i18n/en.json:110`). Likely a typo for the existing key.
- `common.total_items` — used at `web-ui/src/pages/Missions/index.tsx:21`. Absent from both catalogs.

**Impact:** `I18nContext.tsx:63` (`if (hit === undefined) return key;`) renders the raw key text into the UI. **In Farsi, users see the literal strings `approvals.reviewApproval` and `common.total_items`.**

**F3 — `NotificationAggregate` carries pre-rendered English prose into replicated state.**

```rust
// crates/domains/notification-domain/src/lib.rs:13-33
pub struct NotificationAggregate {
    pub id: ObjectId,
    pub public_id: String,
    pub title: String,        // pre-rendered English prose
    pub message: String,      // pre-rendered English prose
    pub priority: String,     // bare String, not an enum
    pub status: String,       // bare String, not an enum
    ...
}
```

Producers write English directly: `crates/bins/worker/src/job_runner.rs:205-209` (`"Staff loan ending soon"` + a full sentence) and `:273-276`. It fans out to OS-level Web Push: `crates/bins/worker/src/push_delivery.rs:323-325`.

**Consequence:** notifications **cannot** be localized client-side without a replicated-state schema change. This is the single highest-severity structural finding. It is also gated by `scripts/verify/verify_serialization.sh` golden fixtures, so it is not a small edit.

### 1.4 Discrepancies between the plan/manifest and the repository

Reported, not silently resolved, as required.

| # | Source claim | Repository evidence | Severity |
|---|---|---|---|
| D1 | Manifest §10/§38.3: *"`mobile-pwa/` does not exist on current `main`"*, iOS Observer PWA is **SPECIFIED / NOT IMPLEMENTED** | `mobile-pwa/` **exists** — 40 `src` files, 2,530 lines, routes, tests, `package.json`, built `dist/` | **High** — manifest is obsolete; PWA is IMPLEMENTED |
| D2 | Manifest §9/§38.4: Flutter client in `mobile/lib/` is a **FROZEN REFERENCE** | `mobile/` **does not exist** in this snapshot | Medium — manifest stale in the opposite direction; Flutter retirement is effectively complete but unrecorded |
| D3 | Plan §5 target: `shared/i18n/schema/translation.schema.json`, `metadata/locales.json`, `metadata/glossary.json` | None exist. Catalog is flat-key, no schema, no locale registry, no glossary | Expected gap — but see §5 |
| D4 | Plan §19: API errors prefer `{"code": "...", "params": {...}}` | `ApiErrorBody` **already has** `code`/`category`/`retryability`/`safe_details`/`correlation_id` (`crates/bins/api-server/src/routes/mod.rs:745-750`). **But** 27 hardcoded English prose literals also ride in `safe_details.message`, and `map_command_error` dispatches on **English substring matching** | **High** — see §5.3 and R3 |
| D5 | Plan §20 expects `i18n-schema`, `i18n-parity`, `i18n-interpolation`, `i18n-generated`, `i18n-audit` checks | Only key-parity, generated-drift, and a single-directory literal audit exist | Medium |
| D6 | Plan §25/§17 expect Farsi typography wired | No font asset exists anywhere in the repo. `Vazirmatn` appears only as a **CSS font-family name** in the five copies of `rtl.css`; **no `.ttf`/`.woff`/`.otf` file exists in the repository** | Medium — see §6 |
| D7 | Plan §33 autonomous agent contract expects `i18n.boundaries.json` and `i18n:report` | Neither exists | Expected gap |

**No conflict was found** between the plan's architectural rules (§4, §13, §18, §19, §24, §30) and the existing implementation. The plan and the code are aligned in direction; the plan simply predates the i18n commits.

---

## 2. Platform Inventory

### 2.1 Applications

| Application | Path | Technology | UI tech | Build | Localization mechanism | User-visible language? | User-visible strings (approx.) |
|---|---|---|---|---|---|---|---:|
| **Web Remote Operator** | `web-ui/` | React 18 + TS + Vite | React/TSX | npm / Vite | `shared/i18n` → `src/i18n/` | **Yes — partial** | ~343 |
| **Mobile PWA / Observer** | `mobile-pwa/` | React + TS + Vite + SW | React/TSX + Tailwind | npm / Vite | `shared/i18n` → `src/i18n/` (provider only) | **Yes — 0% adopted** | ~205 |
| **Desktop / Staff Ops** | `crates/bins/desktop-shell/` | Tauri + Rust `client-composition` | React/TSX (embedded) + Rust | Cargo + npm | `shared/i18n` → `ui/src/i18n/` (provider only) | **Yes — 0% adopted** | ~175 |
| **Admin** | `crates/bins/admin-shell/` | Tauri + thin HTTP client | React/TSX (embedded) + Rust | Cargo + npm | `shared/i18n` → `ui/src/i18n/` (provider only) | **Yes — 0% adopted** | ~144 |
| **Android Operational Client** | `mobile-android/` | Kotlin 2.3.21 + Compose (AGP 8.13.2, minSdk 29, target/compile 36) | Jetpack Compose | Gradle 8.14.3 | `res/values*/strings.xml` + dead `LocaleHelper.kt` | **Yes — 0% adopted** | ~140 |
| **API Server** | `crates/bins/api-server/` | Rust 2021 / Axum 0.7 | none (HTTP) | Cargo | none | **Yes, indirectly** — `safe_details.message` prose | 27 prose literals |
| **Worker** | `crates/bins/worker/` | Rust | none | Cargo | none | **Yes, indirectly** — writes notification prose, Web Push bodies | 2 notification templates |
| **Sync Agent** | `crates/bins/sync-agent/` | Rust | none | Cargo | none | No (status only) | 0 |
| **Migration Tool** | `crates/bins/migration-tool/` | Rust CLI | none | Cargo | none | **Operator-facing CLI only** — see §4.3 | ~10 |
| **Cloud Relay** | `crates/bins/api-server/src/routes/relay.rs` + desktop `relay_socket.rs` | Rust, single-replica deployment | none | Cargo | none | No — errors surface as a boolean + hardcoded UI copy | 0 |
| **Flutter Client** | `mobile/` | — | — | — | — | **Does not exist** (see D2) | n/a |

### 2.2 Shared packages and libraries

| Path | Kind | Localization relevance |
|---|---|---|
| `shared/i18n/en.json`, `fa.json` | Canonical source | **149 keys each. Current authority for all localization.** |
| `shared/i18n/react/I18nContext.tsx` | Canonical runtime | `LocaleProvider`, `useI18n()`, `translateStatic()`. Copied verbatim to 4 apps. |
| `shared/i18n/react/LanguageSwitcher.tsx` | Canonical component | Copied to 4 apps; mounted in 1. |
| `shared/i18n/react/rtl.css` | Canonical CSS | Copied to 4 apps. **11 of 17 rules are inert per-app** (§6.3). |
| `scripts/sync-i18n.mjs` | Generator + auditor | Deterministic; byte-compare check; key parity; scoped literal audit. |
| `crates/kernel/platform-contracts/src/error.rs` | Error contract | 8 stable `#[serde(rename)]` codes + `.code()`/`.category()`/`.retryability()`. **The pattern the rest of the workspace should follow.** |
| `crates/kernel/platform-contracts-ext/src/error.rs` | Duplicate contract | Byte-identical duplicate of the above. Flagged — not a localization issue but relevant to code-authority decisions. |
| `crates/domains/*` (8 crates) | Domain | **Zero `anyhow!`/`bail!`/format-error macros.** `#[error("…")]` prose: 59 occurrences. Language-neutral *wire* format confirmed (§5.5). |
| `crates/mobile-core/`, `crates/mobile-android-jni/` | Native boundary | `{"success": false, "error": String}` envelope with raw prose; plus `null`/`-1` sentinels. No code vocabulary (§5.6). |

### 2.3 Relevant tests

| Suite | Path | i18n-relevant? |
|---|---|---|
| web-ui unit/integration | `web-ui/tests/{unit,integration,e2e,feature-audit,accessibility}` | **25 English-string selectors** — will break on localization |
| web-ui browser | `web-ui/tests/browser/ui-remediation.spec.ts` | 8 selectors + a visual snapshot that RTL/font changes will perturb |
| mobile-pwa tests | `mobile-pwa/tests/{component,browser,offline}` | ~44 English-string selectors — **but mobile-pwa is not built or tested in CI** |
| Android JVM tests | `mobile-android/app/src/test/` (6 files, 27 tests) | Zero locale/string assertions. `ApprovalsScreenSourceTest.kt` **pins the structure of `SettingsScreen.kt`** (any change to `OutlinedTextField` count fails the build). |
| Android instrumented | `mobile-android/app/src/androidTest/` (5 files, 14 tests) | `ApprovalsScreenTest.kt:49,88,89` hard-depend on 3 English literals — will fail on first externalization. |
| Rust | workspace-wide | 22 `assert_eq!` against capitalized literals; ~4 are error-prose-coupled (§8 R7) |

---

## 3. Existing Localization Systems

### 3.1 Complete inventory of localization artifacts

**FACT** — every tracked localization file in the repository (`git ls-files | grep -iE "i18n|locale|translation|strings.xml|LanguageSwitcher|rtl|font"`), 25 files:

```
shared/i18n/en.json                                 ← CANONICAL
shared/i18n/fa.json                                 ← CANONICAL
shared/i18n/react/I18nContext.tsx                   ← CANONICAL runtime
shared/i18n/react/LanguageSwitcher.tsx              ← CANONICAL component
shared/i18n/react/rtl.css                           ← CANONICAL CSS
scripts/sync-i18n.mjs                               ← CANONICAL generator/auditor
web-ui/src/i18n/{I18nContext.tsx,LanguageSwitcher.tsx,rtl.css,dictionaries.generated.ts}      ← generated
mobile-pwa/src/i18n/{… same 4}                                                                          ← generated
crates/bins/desktop-shell/ui/src/i18n/{… same 4}                                                        ← generated
crates/bins/admin-shell/ui/src/i18n/{… same 4}                                                          ← generated
mobile-android/app/src/main/res/values/strings.xml                                                      ← 41 keys, hand-authored, NOT generated
mobile-android/app/src/main/res/values-fa/strings.xml                                                   ← 41 keys, hand-authored, NOT generated
mobile-android/app/src/main/kotlin/com/onyx/util/LocaleHelper.kt                                        ← dead code, 0 call sites
i18n-dual-language-report.md                                                                             ← narrative, not build input
```

### 3.2 Duplication

**FACT — there is exactly one logical localization source with two hand-maintained artifacts outside it.**

Duplication exists at three levels:

1. **Generated copies (acceptable by design).** 16 files across 4 apps, all byte-identical to `shared/i18n/`, all verifiable with `--check`. This is codegen output, not a competing source.
2. **Android strings.xml (a true second catalog).** 41 keys, hand-authored, **not produced by `sync-i18n.mjs`** (`scripts/sync-i18n.mjs:76-81` lists only the 4 React targets). Key parity between `values/` and `values-fa/` is currently exact but **unenforced by any CI check**.
3. **Error-copy systems (4 divergent implementations, all bypass i18n).**

| File | Strings | Locale-aware? |
|---|---:|---|
| `web-ui/src/utils/errorHandler.ts` | 27 | No |
| `mobile-pwa/src/utils/errorHandler.ts` | 24 | No (divergent wording) |
| `crates/bins/desktop-shell/ui/src/utils/userFacingError.ts` | 11 | No |
| `crates/bins/admin-shell/ui/src/utils/errorHandler.ts` | 8 | No |

All four are static string maps — structurally translation-ready — and **all four call sites bypass the `LocaleProvider` context entirely**. The exported `translateStatic()` helper (`I18nContext.tsx:85`), which exists precisely for this case, has **zero callers**.

### 3.3 Obsolete / dead / partially-implemented

| Item | Evidence | Classification |
|---|---|---|
| `mobile-android/.../LocaleHelper.kt` | `grep 'LocaleHelper'` returns **only its own 2 internal lines**. `OnyxApplication.kt` has **no `onCreate` override**. `MainActivity.onCreate` does not call it. No `attachBaseContext`. | **Dead code.** Also uses deprecated `Resources.updateConfiguration` (API 25+) instead of `AppCompatDelegate.setApplicationLocales` / `createConfigurationContext`. |
| Android `values-fa/strings.xml` (41 keys) | 0 of 41 referenced | **Unreachable at runtime** |
| `LanguageSwitcher` in mobile-pwa, desktop-shell, admin-shell | Never imported by any file outside `i18n/` | **Unreachable** |
| `translateStatic()` | Exported; zero callers repo-wide | **Dead export** |
| `rtl.css` 11 of 17 rules in mobile-pwa | Targets `.sidebar`, `.workspace`, `.toast`, `.skip-link`, `.data-row`, `.dialog-actions`, `.detail-grid`, `.inline-details`, `.alert-banner`, `.projection-state`, `.sidebar-open` — **none exist in the PWA** (Tailwind utilities instead) | **Inert** |
| `rtl.css` 15 of 17 rules in both Tauri shells | Same class names; both apps use `onyx-` prefixed classes | **Inert** |
| `date-fns` in `mobile-pwa/package.json:26` | Declared, never imported | **Dead dependency** |
| `approvals.reviewApproval`, `common.total_items` | Referenced in code, absent from catalog | **Dangling references** (F2) |

### 3.4 Competing / third-party localization systems

**FACT: none.** `rg 'react-i18next|i18next|FormattedMessage|react-intl|lingui|@lingui|vue-i18n|IntlProvider|useTranslation' ` across all four frontends returns zero hits. No `package.json` declares an i18n library. No homegrown `translations.ts`/`messages.ts`/`strings.ts` exists outside the generated copies.

**FACT:** the Rust workspace has **no** i18n dependency in any `Cargo.toml` (`rg -i 'i18n|locale|fluent|icu|langid|rust-embed' Cargo.toml crates/**/Cargo.toml` → no matches). All 5 `include_str!` invocations in the workspace load SQL schema files; none loads a catalog.

---

## 4. User-Facing String Inventory

### 4.1 Class A — must become localized

Approximate totals, verified by scripted AST scan for JSX/TS literals and `rg` for Kotlin:

| Surface | Count | Notes |
|---|---:|---|
| `web-ui` JSX/TS user-visible literals | ~343 | 223 JSX text nodes, 29 attributes, 91 TS literals |
| `mobile-pwa` | ~205 | |
| `desktop-shell/ui` | ~175 | |
| `admin-shell/ui` | ~144 | |
| `mobile-android` (`Text("…")` + other sinks) | ~140 | 118 `Text("…")`, 0 `stringResource` |
| Rust reaching a UI (HTTP prose, Tauri IPC, notifications) | ~35 | §5 |
| **Total** | **~1,042** | |

Sub-strata that are frequently forgotten:

- **Accessibility text (a11y).** `aria-label` hardcoded in 6 web-ui files, 6 mobile-pwa files, and both Tauri shells. Example: `web-ui/src/components/Layout/MainLayout.tsx:88` `aria-label="Current organization"` — the key `common.organization` **already exists**. **INFERENCE (high): screen-reader output stays English in Farsi even on already-localized pages.**
- **Error/toast copy.** `web-ui/src/hooks/useCommand.ts` (22 literals), `web-ui/src/utils/errorHandler.ts` (27), `mobile-pwa/src/push/push.ts` (6 `PushSetupError` messages surfaced via `pushStore.message`).
- **Status label maps.** `web-ui/src/components/StatusBadge/index.tsx:3-19` (24 entries, English only); `mobile-pwa/src/components/StatusBadge.tsx:1-22` (20 entries — a near-duplicate with 4 entries missing). These render **domain enum values** and are therefore the exact place where the Domain→Presentation→Localization boundary must be honored (§7).
- **Native/OS surface.** `crates/bins/desktop-shell/tauri.conf.json:14` and `crates/bins/admin-shell/tauri.conf.json:14` — window `"title": "ONYX"` / `"ONYX Admin"`, **never changed at runtime** (0 `.set_title()` calls repo-wide). `productName` at `:2` in both.
- **HTML document metadata.** All four `index.html` hardcode `<html lang="en">` with no `dir` attribute. `LocaleProvider` overwrites `lang`/`dir` at `I18nContext.tsx:47-48`, so this is correct at runtime but is a **first-paint flash-of-LTR risk**.

### 4.2 Class B — must remain machine-readable

**Verified as already language-neutral. These must be explicitly protected from localization.**

| Category | Verified evidence |
|---|---|
| Domain enum wire values | `MissionStatus` (`crates/domains/mission-domain/src/state_machine.rs:11-13`) and `TaskStatus` (`crates/domains/work-domain/src/state_machine.rs:11-13`) serialize to identifiers, never prose. No `rename_all` in `crates/domains/` (so `Draft`, `InProgress`) — **machine-readable, but note the casing inconsistency in §5.5.** |
| `UserClass` | `crates/applications/security-application/src/ports/user_store.rs:71+` — **no `Serialize` derive at all**; hand-mapped to `"top_level_manager"`…`"staff"`, backed by a Postgres `CHECK` constraint. Doc comment: *"a fixed, lowercase wire format, not a free-text field a caller should be lenient about."* |
| API error codes | 57 distinct codes at 77 `ApiError::new` call sites, e.g. `"MOBILE_ACCESS_RESTRICTED"`, `"INVALID_CREDENTIALS"`. Machine-readable. |
| Tauri identifiers | `"com.onyx.platform"` / `"com.onyx.admin"` — key the OS keyring service (`desktop-shell/src/secure_storage/keyring_adapter.rs:24`). |
| `SyncMessageType` | `crates/transports/sync-transport/src/message.rs:253-287` — integer codec `as_u8`/`from_u8`, roundtrip-tested. |
| JNI symbol names | `crates/mobile-android-jni/src/lib.rs:696,703-704` — `"com/onyx/bridge/EventCallback"`, `"onEvent"`. |
| Conflict-resolution vocabulary | `"local"` / `"remote"` / `"escalate"` (`mobile-android-jni/src/lib.rs:408`) — the **one existing machine vocabulary that already crosses the FFI boundary.** Good precedent. |
| Kotlin wire strings | `user_store.rs:945` `vec!["staff","supervisor"]`; `Users.tsx:16-20` `top_level_manager` etc. **The admin UI also renders human labels for these at `Settings.tsx:77-81` — the wire value must not change, only the display.** |
| API paths, HTTP methods, JSON keys | Unchanged by this migration. |
| `UuidCodec.kt:32` `String.format(Locale.ROOT, "%02x", …)` | **Correctly pinned to `Locale.ROOT`** — hex must never be localized. Model behavior. |

### 4.3 Class C — ambiguous

| Item | Location | Why ambiguous | Confidence |
|---|---|---|---|
| `migration-tool` CLI output | `crates/bins/migration-tool/src/main.rs:25-98` | Operator-facing English. `println!("migrations applied and idempotency pass completed")`. Is the operator a "user" in the localization scope? **DECISION REQUIRED.** | Medium that it is in scope |
| `worker` startup logs | `crates/bins/api-server/src/routes/mod.rs:253,279,309,313` | Deployment diagnostics. Plan §13 says logs stay technical "unless explicitly intended for end users." **DECISION REQUIRED.** | High that they are out of scope |
| `ShellError` `.message` content | `crates/bins/desktop-shell/src/lib.rs:59-67` | The enum has a `kind` tag (good), but the content is raw prose. Does it need a code+params refactor, or should the UI keep mapping by `kind` and discard prose (which it already partly does)? **DECISION REQUIRED.** | Medium |
| `webui`/`ApiError.category` and `.retryability` | `routes/mod.rs:746-747` | Free-form strings (`"AUTHORITY"`, `"RETRYABLE"`, …), not enums. Machine-readable today; should they become a closed enum before clients key on them? | Medium |
| Web Push OS notification body | `crates/bins/worker/src/push_delivery.rs:323-325` | Rendered by the **operating system**, not ONYX. ONYX controls the string but cannot control OS chrome. Should `fa` notifications be pre-rendered server-side, or should push carry code+params and let the client pre-render? **DECISION REQUIRED** — interacts with F3. | High that a decision is needed |
| `Last fallback in `t()`: `if (hit === undefined) return key;` | `I18nContext.tsx:63` | Currently leaks raw keys to users (F2). Should this become a CI-failing condition instead of a silent render? **RECOMMENDATION: yes, once the baseline exists.** | High |

---

## 5. Current Locale Architecture — Feature-by-Feature

Assessed against the plan's requirements.

| Requirement | Status | Evidence |
|---|---|---|
| Canonical locale source | **PARTIAL** | `shared/i18n/{en,fa}.json` exist and are authoritative for the React apps. **Not** authoritative for Android (hand-authored second catalog) and **does not exist** for Rust. |
| Locale metadata / registry | **ABSENT** | No `locales.json`, no direction map, no display names, no completeness status. Direction is hardcoded in `I18nContext.tsx:47-48,63` and `LocaleHelper.kt:30` as `locale === "fa" ? "rtl" : "ltr"`. |
| Translation schema | **ABSENT** | Flat key/value object; no JSON Schema, no key-syntax validation, no value-type validation, no reserved-namespace enforcement. |
| Key conventions | **IMPLICIT** | Dotted, `<namespace>.<camelCase>` — `nav.skipToContent`, `missions.noMissionsBody`, `common.noSelection`. **Undocumented and unenforced.** 14 namespaces, all flat at root (no nesting). |
| Key parity | **ENFORCED ✅** | `scripts/sync-i18n.mjs:22-31` — bidirectional. Currently 149 == 149. |
| Interpolation | **IMPARTIAL** | Hand-rolled regex `interpolate()` (`I18nContext.tsx:29-32`), pattern `\{(\w+)\}`. **Only 1 of 149 keys uses it** (`approvals.pendingCount` = `"{count} pending"`). **Placeholder parity is NOT CI-enforced** — verified manually here: 0 mismatches across 149 keys. **Unknown placeholders are silently left in place** (`vars[k] !== undefined ? … : m`) — no failure signal. |
| Pluralization | **ABSENT** | No locale plural rules anywhere. Instead, **English-only pluralization is hardcoded in components**, which the plan §10 explicitly forbids: `web-ui/src/pages/TodoTargets/ListCard.tsx:50` `` `${count} item${count === 1 ? '' : 's'}` ``; `desktop-shell/ui/src/components/Layout/MainLayout.tsx:218` `{count} conflict{count === 1 ? "" : "s"}`; `mobile-android/.../AppShell.kt:131` `"${conflicts.size} synchronization conflict(s) require review"`; Android has **no `<plurals>` resources at all**. |
| Formatting abstraction | **ABSENT** | No shared formatter exists. 21 bare `toLocaleString()` display calls (11 web-ui, 9 mobile-pwa, 1 desktop) with **no locale argument**; 1 `date-fns` `formatDistanceToNow` (`web-ui/src/pages/Dashboard/components/ActivityFeed.tsx:7`) with no locale; hand-rolled number/percent rendering; Android has **zero** `DateFormat`/`NumberFormat` and renders raw `.toString()` counts. |
| RTL abstraction | **PARTIAL / MOSTLY INERT** | See §6. |
| Generated resources | **PARTIAL** | React: deterministic, byte-verified. Android: **not generated.** |
| Platform adapters | **PARTIAL** | React: 4 copies. Android: `LocaleHelper` (dead, deprecated API). Rust/Tauri: none. |
| Validation | **MINIMAL** | Parity + generated-drift + a regex literal audit scoped to **one directory** (§7). |
| CI checks | **PARTIAL** | One step in one job (§7). Absent from `scripts/ci-pipeline.sh` and from `release.yml`. |

### 5.3 The API error contract — verified partial compliance

The plan §19 requires code+params. **FACT:** `ApiErrorBody` (`crates/bins/api-server/src/routes/mod.rs:745-750`) is already correct in shape. **But three defects prevent relying on it:**

**D-a — Prose rides alongside the code.** 27 distinct hardcoded English literals are injected as `safe_details.message`, e.g.:

```rust
// crates/bins/api-server/src/routes/auth.rs:22-30
ApiError::new(
    StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS", "AUTHORITY", "NON_RETRYABLE",
    uuid::Uuid::new_v4().to_string(),
    json!({"message":"Invalid username or password"}),
)
```

Dynamic prose is unbounded: `routes/admin.rs:207` (`PasswordError::Policy(String)` passthrough) and `routes/relay.rs:441` (`format!("{what} must be a replica UUID")`).

**D-b — English substring matching is load-bearing control flow.**

```rust
// crates/bins/api-server/src/routes/command.rs:1348
crate::CommandError::Domain(message) if message.contains("not permitted") => …
// crates/bins/api-server/src/routes/command.rs:1357
if message.contains("already acknowledged") || message.contains("not pending") => …
```

These select the HTTP status and error code. **INFERENCE (high):** rewording any `#[error("…")]` string in the domain crates silently changes API behavior. This is the most dangerous coupling found in the repository. Note `scripts/verify/verify_error_exhaustiveness.sh` exists but **does not run in CI**, and these are exactly the guard clauses it is designed to catch.

**D-c — There is no code registry.** 57 codes are bare string literals at 77 call sites; no `enum`, no `mod error_codes`. A typo in a code string is not a compile error.

**Positive precedent to reuse:** `crates/kernel/platform-contracts/src/error.rs:19-24` already separates developer-facing `#[error("…")]` (Display) from client-facing `#[serde(rename = "…")]` (wire) — precisely the split the plan §18 prescribes. **RECOMMENDATION: make `DomainError` the model for the rest of the workspace.**

### 5.4 Rust user-visible strings

`anyhow!`/`bail!`/`.ok_or_else` counts: `api-server` 30, `worker` 23, `migration-tool` 6, `desktop-shell` 2, `client-composition` 2, others ≤3. **All 8 domain crates: 0.** The real surface is 59 `#[error("…")]` prose strings in domain `error.rs` files — which are developer-facing `Display`, and only become user-facing through D-b.

Highest-confidence user-visible Rust strings:

| file:line | Literal | Path to user |
|---|---|---|
| `api-server/src/routes/auth.rs:28` | `"Invalid username or password"` | HTTP 401 → all clients |
| `api-server/src/routes/auth.rs:222` | `"Mobile access is not enabled for your user class…"` | HTTP 403 |
| `api-server/src/routes/admin.rs:139,155,163,262,332,373,753` | username-exists / parent-missing / cycle / class-denied / privileges | HTTP 4xx |
| `desktop-shell/src/lib.rs:80` | `ShellError::Auth("Invalid username or password")` | **Tauri IPC → desktop UI**; consumed by `pages/Login.tsx:250` via exact-string `===` |
| `desktop-shell/src/lib.rs:83` | `format!("Could not reach the server: {msg}")` | Tauri IPC → desktop UI |
| `mobile-core/src/ffi_commands.rs:74-81` | `json!({"success": false, "error": error.to_string()})` | **JNI → Kotlin**; unbounded prose |

### 5.5 Domain language-neutrality — PASS, with one consistency flag

**FACT:** no domain enum serializes to prose. `rg 'rename_all' crates/domains/` → no matches, so `MissionStatus::Draft` serializes as `"Draft"` and `TaskStatus::InProgress` as `"InProgress"`. **INFERENCE (high):** these are machine-readable, so localization-neutrality **passes**. **Flag:** they coexist with two other casing conventions on the wire — `UserClass` snake_case and API DTO lower-case (`"active"`, `"paused"`, `"blocked"`, `"unacknowledged"`). This is a **consistency** issue, not a localization blocker; **DECISION REQUIRED** whether to normalize before migrating clients to key on these values.

### 5.6 The JNI/Ffi boundary

**FACT:** `crates/mobile-core/src/ffi_commands.rs:74-81` emits `{"success": false, "error": <raw prose>}`. Other FFI surfaces use `null` / `-1` sentinels (`ffi_commands.rs:60,65,68`; `ffi_files.rs:147-151`; `ffi_queries.rs:38`; `ffi_mobile.rs:80,99`; `ffi_events.rs:36`) — `null` conflates at least three distinct failure modes. **There is no error-code vocabulary on this boundary at all.**

**FACT:** `scripts/verify/verify_ffi_signatures.sh` freezes the cbindgen header shape. Any envelope change requires regenerating `crates/mobile-core/mobile-core.h` and updating that verifier's fixture.

---

## 6. RTL and Formatting Systems

### 6.1 `document.lang` / `document.dir`

**FACT:** set centrally by `LocaleProvider` (`I18nContext.tsx:47-48`) on locale change, and persisted to `localStorage["onyx.locale"]` (`:50`). All four apps mount `LocaleProvider` at the root (`web-ui/src/main.tsx:20`, `mobile-pwa/src/main.tsx:39`, `desktop-shell/ui/src/main.tsx:22`, `admin-shell/ui/src/main.tsx:22`). **This part is correct and consistent.**

### 6.2 The RTL CSS overlay is written for one app

**FACT:** `shared/i18n/react/rtl.css` is 68 lines with 17 `html[dir="rtl"]`-guarded rules. Because it is copied verbatim into all four apps, its effective coverage differs sharply:

| App | Rules that actually apply | Notes |
|---|---:|---|
| `web-ui` | ~13 of 17 | Only web-ui uses these class names. `web-ui/src/styles.css` has 15 physical-property occurrences; **2 are unmirrored** — `.data-row { text-align:left }` (`styles.css:61`) and the `margin-left:0` at the 820px breakpoint. |
| `mobile-pwa` | **2 of 17** | Tailwind utilities; none of the targeted classes exist. Only the font-family and heading letter-spacing rules fire. |
| `desktop-shell/ui` | **2 of 17** | Uses `onyx-*` classes. |
| `admin-shell/ui` | **2 of 17** | Same. |

**FACT — highest-impact RTL defect:** `crates/bins/desktop-shell/ui/src/components/Layout/MainLayout.tsx:123` pins the mobile drawer with `"fixed inset-y-0 left-0 transition-transform duration-200"` and animates `-translate-x-full`. The `rtl.css` override that would fix it (`.sidebar { inset: 0 0 0 auto }`, `translateX(100%)`) targets `.sidebar`, which **this app does not use**. **INFERENCE (high): in Farsi the desktop drawer slides in from the wrong side.**

**FACT:** both Tauri shells use 17 unconditional physical-direction Tailwind utilities (`text-left` ×5, `border-l` ×3, `pl-*` ×3, `left-0` ×1, `inset-y-0` ×1, `translate-x` ×2, `text-right` ×2) and **zero** logical equivalents (`ms-`, `me-`, `ps-`, `pe-`, `start-`, `end-` are unused). **None is RTL-guarded.**

**FACT — the one unconditional inline-style defect:** `web-ui/src/pages/TodoTargets/ListDetail.tsx:191` — `<ul style={{ margin: 0, paddingLeft: 20 }}>`, not RTL-guarded.

**FACT:** Android is **accidentally** RTL-clean — zero `TextAlign` usages, zero physical `padding(left=/right=)`, and the 3 `padding(start=/end=)` + 2 `Arrangement.End` uses are logical/RTL-aware. **But this is untested:** `LocalLayoutDirection` is never overridden anywhere, so nothing ever produces an RTL direction at runtime. `android:supportsRtl="true"` is set (`AndroidManifest.xml:59`) but cannot fire, since there is no `res/layout/` (100% Compose) and `LocaleHelper` is never called.

### 6.3 Formatting — no abstraction exists

**FACT:** the plan §11 requires `formatDate()`, `formatRelativeTime()`, etc. **None exist.**

| Category | web-ui | mobile-pwa | desktop | admin | Android |
|---|---:|---:|---:|---:|---:|
| Bare `toLocaleString()` (no locale arg) | 11 | 9 | 1 | 0 | 0 |
| `date-fns` (no locale arg) | 1 | 0 | 0 | 0 | 0 |
| `SimpleDateFormat`/`NumberFormat`/`DecimalFormat` | — | — | — | — | **0** |
| Hand-rolled English pluralization | 1 | 0 | 1 | 0 | 1 |

**Representative sites:** `web-ui/src/components/ProjectionState/index.tsx:81`; `web-ui/src/pages/Tasks/TaskDetail.tsx:3` (two calls on one line); `web-ui/src/pages/TodoTargets/ListDetail.tsx:214,218,242`; `web-ui/src/pages/StaffLoans/LoanCard.tsx:64,68`; `mobile-pwa/src/pages/Dashboard/index.tsx:77`; `crates/bins/desktop-shell/ui/src/pages/Notifications.tsx:171`.

**INFERENCE (high):** switching to `fa` today changes translated copy only. Dates, numbers, and relative times remain in the browser/OS default format with no Persian calendar, no Persian digits, and no Farsi month names.

### 6.4 Typography

**FACT:** `rtl.css:6-7` defines `--onyx-font-rtl: "Vazirmatn", Tahoma, "IRANSans", "Segoe UI", system-ui, sans-serif`. **No `.ttf`, `.woff`, or `.otf` file exists anywhere in the repository.** `Vazirmatn` is referenced in exactly 5 files (the canonical CSS plus its 4 copies). No `@font-face` is declared. Android has no `res/font/` directory, no downloadable-fonts XML, and no `ui-text-google-fonts` dependency.

**INFERENCE (high):** the Persian font stack currently resolves to whatever Tahoma or the OS fallback provides, if anything. Plan §25 ("typography must support locale-specific font configuration from the beginning", "embedded Farsi fonts must be registered") is **not met**.

---

## 7. Existing CI Validation

### 7.1 The one i18n gate

**FACT** — `.github/workflows/ci.yml:221-223`, inside the `web` job only:

```yaml
- name: Verify shared i18n artifacts
  working-directory: .
  run: node scripts/sync-i18n.mjs --check --audit-scope=web-ui/src/pages/Dashboard
```

It performs exactly three things (`scripts/sync-i18n.mjs`):
1. **Key parity** (`:22-31`) — bidirectional EN↔FA. Currently passing.
2. **Generated-drift byte compare** (`:84-117`) — `dictionaries.generated.ts` + 3 copied files, across 4 apps.
3. **Literal audit** (`:39-71`) — regex `/>\s*([A-Za-z][^<>{}\n]{2,})\s*</g` over `.ts`/`.tsx`.

### 7.2 Coverage gaps

| Gap | Detail |
|---|---|
| Audit scope = **1 directory** | `--audit-scope=web-ui/src/pages/Dashboard` — the only fully-migrated surface. ~548 literals elsewhere are unaudited. |
| Regex is narrow | `[^<>{}\n]{2,}` evades interpolations, templates, and multi-line JSX; it cannot see `placeholder=`, `aria-label=`, `title=`, `alt=`, or TS-literal error/toast copy — which is where most of the remaining volume lives. |
| No Rust check | Nothing in CI inspects Rust strings. |
| No Android check | `mobile-android` is built (`ci.yml:342-382`) but has no resource/locale lint; the 41↔41 `values`/`values-fa` parity is unenforced. |
| Not in `scripts/ci-pipeline.sh` | The local gate script runs 7 `web-ui` scripts (`ci-pipeline.sh:29`); **none is i18n-related.** |
| Not in `release.yml` | A release can ship with no i18n check. |
| **Divergence** | `ci-pipeline.sh` runs `bundle-check` and not `lint`/`test:browser`; `ci.yml` runs `lint`/`test:browser` and not `bundle-check`. **Neither is a superset of the other.** |
| **mobile-pwa absent from CI entirely** | No `working-directory: mobile-pwa` in any workflow. It is never built, linted, or tested — yet holds ~44 English-string test selectors that will all fail when wired in. |
| Zero i18n tests | No test asserts key resolution, locale switching, RTL application, or interpolation. |

### 7.3 What CI currently does for the other frontends

| App | In CI? | What runs |
|---|---|---|
| `web-ui` | ✅ Full | lint, type-check, vitest, a11y, feature-audit, **i18n check**, Playwright, build |
| `crates/bins/desktop-shell/ui` | ⚠️ Build only | `npm ci && npm run build` (`ci.yml:255-260`) |
| `crates/bins/admin-shell/ui` | ⚠️ Build only | `npm ci && npm run build` (`ci.yml:261-266`) |
| `mobile-pwa` | ❌ **Absent** | — |
| `mobile-android` | ✅ Build + unit tests | no i18n check |

---

## 8. Canonical-Source Assessment

### 8.1 Evaluation of each candidate

| Candidate | Current authority | Duplication | Consumers | Migration complexity | Generation feasible? | Information at risk |
|---|---|---|---|---|---|---|
| **`shared/i18n/{en,fa}.json`** | **Authoritative for all 4 React frontends** | Generated into 16 files (by design) | 4 frontends | **Low** — extend in place | Already generated | None |
| `shared/i18n/react/*` | Authoritative runtime | 4 copies | 4 frontends | Low | Already copied | None |
| `shared/i18n/react/rtl.css` | Canonical but app-inappropriate | 4 copies | 4 frontends | **Medium** — must become per-app or selector-generic | Yes, from a per-app class manifest | If consolidated naively, web-ui-specific rules could leak into Tailwind apps |
| `mobile-android/res/values*/strings.xml` | **Non-canonical second catalog** | Diverges from the 149-key JSON by construction | **0 (none)** | **Medium** — needs a generator + key-name mapping | Yes, with a name-mapping layer | None (0 consumers) |
| Rust (none exists) | N/A | N/A | N/A | **High** — from scratch | Yes, via `include_str!` + a lookup fn | N/A |
| `utils/errorHandler.ts` ×4 | Ad-hoc, bypasses i18n | 4 divergent copies | 4 apps | **Low** — static maps | Yes, once keyed | Wording divergence is currently a bug, not a feature |

### 8.2 Safest canonical-source strategy

**FACT:** `shared/i18n/{en,fa}.json` is already a single, deterministic, CI-verified authority consumed by 4 platforms, with zero third-party i18n libraries anywhere in the stack.

**RECOMMENDATION (fact-based, not a redesign):** **adopt `shared/i18n/` as the canonical source, unchanged in location and format.** Extending it is strictly lower-risk than the plan's target structure `shared/i18n/{schema,locales,metadata}`, because:

1. It is already CI-verified for parity and drift — the plan's §5 target would need that built from zero.
2. The flat key format is directly consumable from Rust via `include_str!` with no dependency, and is trivially mappable to Android `strings.xml` names.
3. Replacing it would invalidate 16 generated files, 4 provider mount points, and the one existing CI gate — with no functional gain.
4. The plan §2 explicitly directs extending rather than competing.

**RECOMMENDATION:** the plan's §5 sub-structure is still worth adopting *additively*, without relocating the catalogs:
- `shared/i18n/schema/translation.schema.json` — **new**, validated by CI.
- `shared/i18n/metadata/locales.json` — **new**, the locale registry (direction, default, display name, formatting locale).
- `shared/i18n/metadata/glossary.json` — **deferred** to the post-infrastructure Localization Quality Program, per plan §39.

**DECISION REQUIRED — canonical source is otherwise settled. Two sub-decisions are not:**

- **D-CAN-1:** Is the flat key format retained, or is the catalog migrated to a nested structure? Flat is simpler and already verified; nested is more ergonomic for authoring and matches the plan §7 example. **Migration cost is real (all 16 generated files, all 38 call sites).** *Recommendation: retain flat.*
- **D-CAN-2:** Where does `rtl.css` live? One canonical stylesheet that must be hand-adapted per app, or generated per app from a per-app class manifest? *Recommendation: per-app RTL layers, generated, because a single shared file demonstrably cannot serve both a BEM-CSS app and a Tailwind app.*

### 8.3 Information-loss assessment

No information loss is required by the recommended strategy. The two genuine risks are:

1. **Android key-name mapping.** The canonical keys are dotted (`missions.noMissions`); Android requires `[a-z0-9_.]`. A mechanical transform (`/` → `_`) is deterministic and lossless. **No loss.**
2. **Domain enum → display mapping.** `StatusBadge` maps domain wire values (`Draft`, `InProgress`, `AwaitingApproval`, …) to English labels. This mapping must be **explicit and preserved**, not derived — otherwise client-specific terminology diverges. **No loss if migrated deliberately; silent divergence if not.**

---

## 9. Migration Boundary Manifest

### 9.1 CANONICAL

```
shared/i18n/en.json                        149 keys  — authoritative EN catalog
shared/i18n/fa.json                        149 keys  — authoritative FA catalog
shared/i18n/react/I18nContext.tsx          LocaleProvider / useI18n / translateStatic
shared/i18n/react/LanguageSwitcher.tsx     the only sanctioned language control
scripts/sync-i18n.mjs                      sole generator; sole parity/drift auditor
```

Phase 1 additions (recommended, additive):

```
shared/i18n/schema/translation.schema.json     NEW — key syntax, value type, namespace
shared/i18n/metadata/locales.json              NEW — id, direction, default, display name, formatting locale
shared/i18n/README.md                          NEW — key conventions, commands
i18n.boundaries.json                           NEW — surface migration states (plan §36)
```

### 9.2 CONSUMERS

| Consumer | Path | Must consume via |
|---|---|---|
| Web Remote Operator | `web-ui/src/**` | `useI18n()` / `t()` |
| Mobile PWA | `mobile-pwa/src/**` | `useI18n()` / `t()` |
| Desktop UI | `crates/bins/desktop-shell/ui/src/**` | `useI18n()` / `t()` |
| Admin UI | `crates/bins/admin-shell/ui/src/**` | `useI18n()` / `t()` |
| Android | `mobile-android/app/src/main/**` | `stringResource(R.string.x)` |
| Rust (new) | new crate | new lookup function reading `include_str!("shared/i18n/en.json")` |

**Rule:** components must not import locale files directly (plan §14).

### 9.3 ADAPTERS

| Adapter | Purpose | Current state |
|---|---|---|
| `I18nContext.tsx` | React binding; sets `lang`/`dir`; persists locale | Working |
| `LanguageSwitcher.tsx` | Language control | Working; mounted in web-ui only |
| `scripts/sync-i18n.mjs` | Codegen for 4 React apps | Working |
| **Android generator** | `en.json` → `values/strings.xml` + `values-fa/strings.xml` | **ABSENT — must be built** |
| **Rust catalog loader** | `en.json` → lookup | **ABSENT — must be built** |
| **Locale registry** | direction/format metadata | **ABSENT — must be built** |
| **Formatting abstraction** | `formatDate/Number/RelativeTime/Duration/Percent/Currency` | **ABSENT — must be built** |
| **Pluralization** | locale plural categories | **ABSENT — must be built** |
| **Android locale application** | persist + apply across lifecycle | **BROKEN — `LocaleHelper` is dead and uses a deprecated API** |
| **`rtl.css`** | per-app RTL layer | **Shared-but-mostly-inert (§6.2)** |
| **Tauri title/productName** | native window + bundle name | **Not localizable without per-locale bundle config** |

### 9.4 NEVER LOCALIZE

| Category | Concrete ONYX instances |
|---|---|
| API paths, HTTP methods | `/api/admin/*`, `/api/mission/*` |
| JSON property names | `safe_details`, `correlation_id`, `lifecycle_epoch`, `authority_epoch` |
| Error/transport codes | `INVALID_CREDENTIALS`, `MOBILE_ACCESS_RESTRICTED`, `TENANT_MISMATCH`, `INVALID_TRANSITION`, `EPOCH_CONFLICT`, `CONFLICT_PENDING` (all 57) |
| `UserClass` wire values | `top_level_manager`, `senior_manager`, `team_leader`, `supervisor`, `staff` |
| Domain enum wire values | `Draft`, `AwaitingApproval`, `InProgress`, `Submitted`… |
| Conflict-resolution vocabulary | `"local"`, `"remote"`, `"escalate"` |
| Event / message identifiers | `"onyx:event"`, `SyncMessageType` integers |
| JNI symbols | `"com/onyx/bridge/EventCallback"`, `"onEvent"` |
| Package / bundle identifiers | `com.onyx.platform`, `com.onyx.admin`, `com.onyx` |
| Env vars / headers | `ONYX_BOOTSTRAP_TOKEN`, `ONYX_DATABASE_KIND`, `x-onyx-bootstrap-token` |
| CSS classes | `.sidebar`, `.onyx-workspace-header`, Tailwind utilities |
| Test selectors | all `getByText`/`getByRole` name strings |
| SQL identifiers | all table/column names in `migrations/` |
| Hex / UUID formatting | `UuidCodec.kt:32` (already `Locale.ROOT`) |
| Technical diagnostics / logs | `tracing::*`, `.expect(...)` panic messages, Web Push transport internals |
| Hex/byte constants | `android:application/octet-stream` |

### 9.5 DEFER

| Item | Rationale |
|---|---|
| `migration-tool` CLI localization | Operator tool, not product UI. **DECISION REQUIRED.** |
| Startup/deployment logs | Plan §13: internal logs stay technical. **DECISION REQUIRED.** |
| Web Push OS-chrome rendering | Outside ONYX's control; depends on F3 resolution. |
| Font asset embedding + Persian typography polish | Plan §25 wires infrastructure; plan §39 defers visual refinement. |
| Glossary / terminology | Plan §39 — Localization Quality Program. |
| Synthetic locale `xx`, pseudo-localization | Plan §26–27 — Phase 10. |
| Rust domain-layer localization | Plan §18 — domain must stay language-neutral; there is nothing to migrate there. |

---

## 10. Risk Register

### CRITICAL

**C1 — `web-ui` has three unresolved `useI18n` references and the i18n CI gate is likely unreachable.**
`Approvals/index.tsx:12`, `Notifications/index.tsx:9`, `Tasks/index.tsx:10` call `useI18n()` with no import; `strict: true`; no global declaration; confirmed in committed `HEAD`. `.github/workflows/ci.yml` runs `type-check` (`:213`) **before** the i18n check (`:221`), so a type-check failure blocks the gate. **Mitigation: confirm in CI, then fix with a 3-line import.** This is pre-existing, not introduced by this audit.

**C2 — `NotificationAggregate` embeds English prose in replicated state.**
`notification-domain/src/lib.rs:13-33`. Client-side localization is impossible without a schema change; golden fixtures under `scripts/verify/verify_serialization.sh` gate that change; Web Push output depends on it. **Mitigation: DECISION REQUIRED on the shape (code+params vs. dual-write) before Phase 7.**

**C3 — English substring matching controls API behavior.**
`api-server/src/routes/command.rs:1348,1357`. Any rewording of domain `#[error("…")]` silently changes HTTP status/code. The guard script that would catch this (`verify_error_exhaustiveness.sh`) does not run in CI. **Mitigation: replace with typed error matching before any error-prose work.**

**C4 — Effective localization coverage is ~4% of user-facing strings, concentrated in one app.**
~1,042 identified; 52 `t()` calls, of which 49 are in web-ui and 3 are in never-mounted `LanguageSwitcher` components. Android: 0 of 41 keys referenced. **Mitigation: this is the migration itself; the risk is scoping it as if infrastructure were absent.**

### HIGH

**H1 — Two dangling translation keys render raw key text to users in Farsi.**
`approvals.reviewApproval`, `common.total_items`; masked by the `return key` fallback at `I18nContext.tsx:63`. **Mitigation: add/repair the two keys; add a CI check that fails on unresolved keys instead of rendering them.**

**H2 — Three of four frontends have no reachable language switcher.**
`mobile-pwa`, `desktop-shell/ui`, `admin-shell/ui` never mount `LanguageSwitcher`. **Mitigation: mount it in each during Phase 1 so `fa` is verifiable at all.**

**H3 — RTL is functionally absent in three of four frontends.**
`rtl.css` is 2/17-effective in mobile-pwa and both Tauri shells. 17 unguarded physical Tailwind utilities in the shells; `MainLayout.tsx:123` drawer will slide in from the wrong side. **Mitigation: Phase 9 RTL hardening; but do not claim RTL works before then.**

**H4 — Android `values-fa/strings.xml` is unreachable and `LocaleHelper` is dead code using a deprecated API.**
`grep 'LocaleHelper'` → 2 self-references only. Uses `Resources.updateConfiguration` (deprecated API 25) instead of `AppCompatDelegate.setApplicationLocales`. No `locale_config.xml`, no `android:localeConfig`, no `configChanges`. **Mitigation: rebuild the Android locale mechanism on the supported API during Phase 7.**

**H5 — Android ships ~140 untranslated strings with 41 translated keys, and one instrumented test hardcodes 3 English literals.**
`ApprovalsScreenTest.kt:49,88,89`. **Mitigation: migrate tests to `stringResource`-keyed assertions in the same change that externalizes the strings.**

### MEDIUM

**M1 — No formatting abstraction; 21 locale-blind `toLocaleString()` calls.**
Persian users see non-Persian dates/numbers. **Mitigation: build the formatting API in Phase 1, before surface migration.**

**M2 — No pluralization; English plural logic is hardcoded in 3 components.**
Violates plan §10 directly. `ListCard.tsx:50`, `desktop MainLayout.tsx:218`, `AppShell.kt:131`. **Mitigation: build in Phase 1.**

**M3 — Two divergent error-copy systems (+2 more) bypass the provider entirely.**
4 files, 70 literals, all locale-blind. `translateStatic()` exists for this and has zero callers. **Mitigation: low-risk, high-value first win.**

**M4 — 25 web-ui + ~44 mobile-pwa English-string test selectors will fail as surfaces are localized.**
`web-ui/tests/integration/ui_gap_workflows.test.tsx:112,158-161`; `web-ui/tests/browser/ui-remediation.spec.ts:63,76,85,87,88`; `mobile-pwa/tests/browser/observer.spec.ts` (16). Plus a visual snapshot at `ui-remediation.spec.ts:48`. **Mitigation: migrate tests to `t()`-derived names *before* migrating the surfaces they assert on.**

**M5 — `mobile-pwa` is not built, linted, or tested in CI.**
~44 string selectors are dormant and will fail en masse when wired in. **Mitigation: wire mobile-pwa into CI before migrating it.**

**M6 — CI i18n enforcement is one regex over one directory.**
Cannot see attributes, TS-literal error/toast copy, Kotlin, or Rust. **Mitigation: expand scope per surface as it migrates (plan §21 boundary states).**

**M7 — Android strings.xml is a second, hand-authored, unenforced catalog.**
41↔41 parity holds today but nothing guards it. **Mitigation: generate it from the canonical source (Phase 2).**

**M8 — No locale registry; direction is hardcoded in two independent places.**
`I18nContext.tsx:47-48,63` and `LocaleHelper.kt:30` both hardcode `locale === "fa"`. Adding a third locale requires editing both. **Mitigation: build `locales.json` in Phase 1.**

**M9 — `Scripts/ci-pipeline.sh` and `.github/workflows/ci.yml` are divergent gate sets.**
Local runs do not include the i18n check. **Mitigation: align them.**

### LOW

**L1 — Tauri `title`/`productName` are hardcoded English; Tauri 2 has no per-locale bundle metadata.**
`desktop-shell/tauri.conf.json:2,14`; `admin-shell/tauri.conf.json:2,14`. **Mitigation: DEFER; document as a known limit.**

**L2 — All four `index.html` hardcode `<html lang="en">` with no `dir`.**
Corrected at runtime by `LocaleProvider`; causes first-paint flash. **Mitigation: cosmetic.**

**L3 — Dead dependencies and exports.**
`date-fns` in `mobile-pwa/package.json` (never imported); `translateStatic()` (zero callers); `LocaleHelper.kt` (zero callers).

**L4 — Two serialization casing conventions coexist on the wire.**
`MissionStatus` → `"Draft"` vs `UserClass` → `"top_level_manager"` vs API DTO → `"active"`. Language-neutral but inconsistent. **Mitigation: DECISION REQUIRED before clients key on these.**

**L5 — Duplicated `DomainError` contract in `platform-contracts` and `platform-contracts-ext`.**
Byte-identical. Not a localization issue; relevant if error codes are centralized.

---

## 11. Unknowns and Decisions Required

### DECISION REQUIRED (blocking or shaping)

| ID | Question | Why it cannot be answered from the repository | Blocks |
|---|---|---|---|
| **DR-1** | Is `NotificationAggregate` migrated to `code`+`params`, or dual-written with a legacy prose fallback? | Requires a replicated-state version strategy and a Web Push rendering decision. Both are architectural. | Phase 7, and the plan §19 model |
| **DR-2** | Is `migration-tool` CLI output in the localization scope? | Plan §13 doesn't classify operator-facing CLI output. | Scope sizing |
| **DR-3** | Web Push: server pre-renders per user locale, or the client pre-renders from code+params? | Depends on DR-1 and on whether the OS notification is treated as ONYX-owned copy. | Phase 7 |
| **DR-4** | Retain the flat key format or migrate to nested JSON? | Trade-off between authoring ergonomics and 16 generated files + 38 call sites + the existing verified pipeline. *Recommendation: retain flat.* | Phase 1 |
| **DR-5** | Per-app generated RTL layers, or one shared canonical stylesheet? | Depends on whether a single stylesheet can legitimately serve both BEM-CSS and Tailwind apps. Evidence in §6.2 says it cannot. | Phase 1 / Phase 9 |
| **DR-6** | Normalize domain-enum wire casing (`Draft` → `draft`) before clients key on them? | Changes a wire contract; no issue or decision record in the repo requests it. | Phase 4+ (or DEFER) |
| **DR-7** | Should `ShellError` become code+params, or should the UI keep mapping by `kind` and discard prose? | The desktop UI already partially discards prose (`userFacingError.ts:36`). Two valid designs. | Phase 8 |

### Unknowns requiring explicit investigation

| ID | Unknown | How to resolve |
|---|---|---|
| **U-1** | **Does `web-ui` type-check today?** | Run the CI `web` job on `d69604a` and read the `type-check` step. `node_modules` is not installed locally and the repo mandates CI-only builds. **This determines whether C1 is blocking.** |
| **U-2** | Are there uncommitted/undocumented i18n decisions in `docs/DECISIONS.md`? | `docs/DECISIONS.md` is 396 KB and was not read in full by this audit. **Should be checked before Phase 1 — a DECISIONS entry could contradict the recommendation in §8.2.** |
| **U-3** | Do `docs/governance/` mobile documents specify a locale mechanism? | `ONYX-MOB-01_Android_Kotlin_iOS_PWA_Technical_Blueprint_v1.1.md` was not inspected. |
| **U-4** | Is a Farsi webfont licensed and available for embedding? | No font asset exists in the repo; license status cannot be determined from source. |
| **U-5** | What is the intended Rust-side localization surface? | Nothing in the repo defines it. Plan §18 and §8 imply "presentation/UI bridge only," but no crate is designated. |
| **U-6** | Do visual snapshots survive RTL/font changes? | Cannot be determined without running Playwright. |
| **U-7** | Is `crates/kernel/platform-contracts-ext` a live duplicate or dead code? | Affects where a centralized error-code registry would live. |

---

## 12. Recommended Execution Order

Sequenced for safety. **This is a recommendation only; nothing here has been implemented.**

### Phase 0.5 — Verify and unblock (do this first)

1. **Run the CI `web` job on `d69604a`.** Read the `type-check` result. (U-1)
2. **If C1 is confirmed:** add the 3 missing `useI18n` imports. 3 lines, no behavior change.
3. **If H1 is confirmed as a real UI defect:** add `approvals.reviewApproval` (or repoint it to `missions.reviewApproval`) and `common.total_items` to both catalogs.
4. **Read `docs/DECISIONS.md` for existing i18n decisions** (U-2) and `docs/governance/ONYX-MOB-01` (U-3).

*Rationale: these are pre-existing defects in the very surface Phase 1 will build on. Fixing them first means every later phase inherits a green baseline.*

### Phase 1 — Consolidate the canonical core (extend; do not replace)

5. Add `shared/i18n/metadata/locales.json` (registry: id, direction, default, display name, formatting locale).
6. Refactor `I18nContext.tsx` to read direction/default from the registry instead of hardcoding `locale === "fa"`. This resolves M8 and is the precondition for a third locale.
7. Add `shared/i18n/schema/translation.schema.json` + a `schema` check to `sync-i18n.mjs`.
8. Add an **interpolation parity check** to `sync-i18n.mjs` (plan §9). Today this passes with 0 mismatches — adding the check costs nothing and locks it in.
9. Add a **formatting abstraction** (`formatDate`, `formatRelativeTime`, `formatNumber`, `formatPercent`, `formatDuration`) resolving M1, and route it through the registry's formatting locale.
10. Add **pluralization** support resolving M2, and remove the three hardcoded English plural sites as proof.
11. Mount `LanguageSwitcher` in mobile-pwa, desktop-shell, and admin-shell (H2). Without this, `fa` cannot be verified in three of four apps.
12. Document key conventions in `shared/i18n/README.md`; add `i18n:check` / `i18n:generate` / `i18n:audit` scripts per repo convention (plan §28).
13. Create `i18n.boundaries.json` with every surface at `unmanaged` (plan §36).

### Phase 2 — Generation

14. Extend `sync-i18n.mjs` to generate `values/strings.xml` and `values-fa/strings.xml` from the canonical JSON (M7). **Android files become generated, not authored.**
15. Decide DR-5; produce per-app RTL layers (H3, Phase 9 groundwork).
16. Build the Rust catalog loader if DR/U-5 is resolved.

### Phase 3 — CI enforcement

17. Change the audit from *fail-on-scope* to *baseline* (`i18n-baseline.json`, plan §22): existing violations recorded, baseline may only shrink.
18. Widen `--audit-scope` surface by surface; add attribute and TS-literal patterns alongside the JSX regex (M6).
19. Add Android `values`/`values-fa` parity to CI.
20. Add `i18n:report` with JSON output (plan §29).
21. Wire the i18n check into `scripts/ci-pipeline.sh` and `release.yml`, and reconcile the two gate sets (M9).
22. **Wire `mobile-pwa` into CI** (M5) — before migrating it, so its ~44 dormant string selectors are exposed while it can still be fixed cheaply.

### Phases 4–8 — Surface migration

23. Order by risk-adjusted value, **not** by app:
    - **Error/toast copy first** (M3): 4 files, 70 literals, static maps, no logic change. Cheapest real win, and it proves the key pipeline end-to-end.
    - **web-ui surfaces** (M4 first: migrate the 25 test selectors to `t()`-derived names *before* touching the components they assert on).
    - **mobile-pwa**, then **desktop-shell**, then **admin-shell**.
    - **Android** last among UI platforms (needs Phase 2 generation + H4's locale-mechanism rebuild).
24. **Rust (§19/§8):** do C3 first — replace the `contains("not permitted")` substring dispatch with typed error matching — before any error-prose work. Only then consider the `safe_details.message` prose removal. C3 is a **correctness** prerequisite, not a localization task.

### Phase 9–10

25. RTL hardening, synthetic locale `xx`, pseudo-localization — per plan §26, §27, §9.

---

## 13. Exact Files and Directories Inspected

### Read in full

```
shared/i18n/en.json
shared/i18n/fa.json
shared/i18n/react/I18nContext.tsx
shared/i18n/react/LanguageSwitcher.tsx
shared/i18n/react/rtl.css
scripts/sync-i18n.mjs
scripts/ci-pipeline.sh
i18n-dual-language-report.md
Cargo.toml
docs/i18n/  (created by this audit)
```

### Inspected via targeted read + scripted scan

```
mobile-android/app/src/main/res/values/strings.xml
mobile-android/app/src/main/res/values-fa/strings.xml
mobile-android/app/src/main/kotlin/com/onyx/util/LocaleHelper.kt
mobile-android/app/src/main/AndroidManifest.xml
mobile-android/app/build.gradle.kts
mobile-android/build.gradle.kts
mobile-android/settings.gradle.kts
mobile-android/gradle/wrapper/gradle-wrapper.properties
crates/bins/desktop-shell/tauri.conf.json
crates/bins/admin-shell/tauri.conf.json
crates/bins/desktop-shell/src/lib.rs        (error-construction sites)
crates/bins/desktop-shell/src/session.rs
crates/bins/desktop-shell/src/relay_socket.rs
crates/bins/desktop-shell/src/secure_storage/{mod,keyring_adapter}.rs
crates/bins/admin-shell/src/{lib,main}.rs
crates/bins/api-server/src/routes/{mod,auth,admin,command,relay,files}.rs
crates/bins/api-server/src/middleware/rate_limit.rs
crates/bins/worker/src/{job_runner,push_delivery,staff_loan_scheduler,webpush}.rs
crates/bins/migration-tool/src/{main,migrations}.rs
crates/domains/notification-domain/src/lib.rs
crates/domains/{mission,work}-domain/src/state_machine.rs
crates/domains/*/src/error.rs
crates/kernel/platform-contracts/src/error.rs
crates/kernel/platform-contracts-ext/src/error.rs
crates/applications/security-application/src/ports/user_store.rs
crates/infrastructure/security-adapter/src/user_store.rs
crates/mobile-core/src/{ffi_commands,ffi_files,ffi_events,ffi_queries,ffi_mobile}.rs
crates/mobile-android-jni/src/lib.rs
crates/transports/sync-transport/src/message.rs
crates/bins/api-server/tests/team_leader_precheck_authorization.rs
crates/bins/api-server/tests/mobile_observer_capability.rs
crates/bins/api-server/tests/mobile_access_gate.rs
crates/applications/client-composition/tests/task_owner_authority_gate.rs
crates/mobile-core/tests/ffi_integration.rs
migrations/sqlite/20260101000000_initial_schema.up.sql (referenced only)
```

### Scanned (directory-wide `rg` + per-file census)

```
web-ui/src/**                              web-ui/tests/**
web-ui/index.html                          web-ui/tsconfig.app.json
mobile-pwa/src/**                          mobile-pwa/tests/**
mobile-pwa/index.html                      mobile-pwa/package.json
mobile-pwa/playwright*.config.ts
crates/bins/desktop-shell/ui/src/**        crates/bins/desktop-shell/ui/index.html
crates/bins/admin-shell/ui/src/**          crates/bins/admin-shell/ui/index.html
mobile-android/app/src/main/kotlin/**      mobile-android/app/src/main/res/**
mobile-android/app/src/test/**             mobile-android/app/src/androidTest/**
.github/workflows/{ci,Debug,devcontainer-check,fmt-fix,release}.yml
scripts/verify/*                           scripts/release.sh
scripts/canary-rollout.sh                  scripts/setup-onyx-windows.ps1
scripts/collect-ui-evidence.mjs
```

### Explicitly NOT inspected

- `docs/DECISIONS.md` (396 KB) — flagged as **U-2**; may contain i18n decisions that would change §8.2.
- `docs/governance/*` mobile documents — flagged as **U-3**.
- `docs/audit/`, `docs/release/`, `docs/runbooks/`, `docs/mobile-migration/`.
- `node_modules/` (absent), `dist/`, `target/`, `playwright-report/`, `test-reports/`, `test-results/`.
- `mobile/` (does not exist).
- Full content of the 396 KB `DECISIONS.md` and the 55 KB manifest.

### Commands executed (read-only)

```
git log / git show / git status / git ls-files
ls, find, sed, head, md5sum, diff
rg (extensive, all claims re-verified)
node -e  (catalog key/parity/interpolation analysis)
node scripts/sync-i18n.mjs --check        → "i18n in sync."  exit 0
```

---

## 14. Verification Performed

| Claim class | How verified |
|---|---|
| 149 keys in each catalog | `node -e` object-key count on both files |
| Generated artifacts in sync | `node scripts/sync-i18n.mjs --check` → exit 0; plus md5 comparison of all 4 generated dictionaries (all `480f61e0…`) |
| `common.total_items` absent from catalog | `grep -c` → 0, in both `en.json` and `fa.json` |
| `approvals.reviewApproval` absent | Extracted all 38 distinct literal `t()` keys from 4 apps, diffed against catalog |
| Interpolation parity holds today | `node -e` placeholder-set comparison across all 149 keys → 0 mismatches |
| `useI18n` imported nowhere in 3 files | `grep -rn "useI18n" web-ui/src` — only 2 importers exist (`MainLayout.tsx:10`, `Sidebar.tsx`); the 3 pages are absent |
| Defect is in committed HEAD | `git show HEAD:web-ui/src/pages/Approvals/index.tsx` |
| `LanguageSwitcher` unmounted in 3 apps | `grep -rl LanguageSwitcher <app>/src --include=*.tsx` filtered to exclude `i18n/` |
| Android 0 `stringResource` / 0 `R.string.*` | `grep -rc` across all 37 `.kt` files |
| `LocaleHelper` has 0 call sites | `grep -rn "LocaleHelper" mobile-android` → 2 hits, both inside the class itself |
| i18n CI gate scope | `sed -n '195,270p' .github/workflows/ci.yml` |
| No competing i18n library | `rg` for `i18next\|react-intl\|lingui\|FormattedMessage\|IntlProvider` across all 4 frontends → 0 |
| No Rust i18n dependency | `rg -i 'i18n\|locale\|fluent\|icu\|langid\|rust-embed' Cargo.toml crates/**/Cargo.toml` → 0 |
| Notification prose in replicated state | Direct read of `notification-domain/src/lib.rs:13-33` + producer + push consumer |
| English substring control flow | Direct read of `command.rs:1348,1357` and `user_store.rs:37,108` |
| `mobile/` does not exist | `ls mobile` → absent |
| Manifest D1/D2 discrepancies | `ls mobile-pwa` (present, 40 src files) vs manifest §10/§38.3 |

**Not verified (explicitly):**

- **No build was run.** `tsc`, `eslint`, `vitest`, `playwright`, `cargo`, and `gradle` were not executed. All TypeScript build-status statements are static source inspection. This follows the repository's own CI-only policy recorded in `i18n-dual-language-report.md`.
- **All string counts are approximate** and derived from scripted pattern matching, not an AST parse of full JSX/Kotlin expressions. They are directionally reliable and internally consistent (same method across all apps) but should not be treated as exact.
- **No production behavior was changed.** Verified: `git status --short` returned clean before this audit; the only file created is `docs/i18n/PHASE-0-DISCOVERY-REPORT.md`.

---

## 15. Facts vs. Recommendations — Index

**Facts established (repository-verified):**

1. `shared/i18n/{en,fa}.json` exist with 149 identical keys; parity passes; generated artifacts are byte-identical across 4 apps and match source.
2. All four frontends mount `LocaleProvider` at the root.
3. Adoption: web-ui 52 `t()` calls; mobile-pwa, desktop-shell, admin-shell 2 each — all inside an unmounted component; Android 0; Rust 0.
4. `LanguageSwitcher` is mounted only in web-ui.
5. Android `values-fa/strings.xml` (41 keys) has 0 references; `LocaleHelper.kt` has 0 call sites.
6. `NotificationAggregate` carries `title: String` / `message: String` English prose into replicated state and out to Web Push.
7. `ApiErrorBody` already has code/category/retryability/correlation_id, but 27 English prose literals ride in `safe_details.message`.
8. `map_command_error` dispatches on English substring matching (`command.rs:1348,1357`).
9. Domain enum wire values are machine-readable; no domain enum serializes to prose. `UserClass` has no `Serialize` derive and uses hand-mapped snake_case.
10. No localization formatting abstraction exists; 21 bare `toLocaleString()` calls; 1 locale-blind `date-fns` call; Android has zero `DateFormat`/`NumberFormat`.
11. No pluralization support; 3 hardcoded English plural sites.
12. No locale registry, no schema, no glossary; direction is hardcoded in 2 places.
13. `rtl.css` is 2/17-effective in mobile-pwa and both Tauri shells; `desktop MainLayout.tsx:123` drawer will slide in from the wrong side in RTL.
14. No font file exists anywhere in the repository.
15. The only CI i18n gate is `ci.yml:221-223`, auditing one directory with one narrow regex; absent from `ci-pipeline.sh` and `release.yml`.
16. `mobile-pwa` is not built, linted, or tested in CI.
17. Three `web-ui` files call `useI18n()` without importing it (committed at HEAD).
18. Two referenced keys are absent from the catalog.
19. Manifest D1/D2 are stale: `mobile-pwa/` exists; `mobile/` does not.
20. 25 web-ui + ~44 mobile-pwa English-string test selectors exist; `ApprovalsScreenTest.kt:49,88,89` hardcode English literals.

**Recommendations (NOT implemented):** everything in §12, plus the retain-flat and per-app-RTL-layer proposals in §8.2.

**Decisions required:** DR-1 … DR-7 and U-1 … U-7 in §11.

---

**End of Phase 0 Discovery Report.**
