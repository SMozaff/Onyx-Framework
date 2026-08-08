# ONYX — Production Readiness Audit Register

**Status:** Phase 0 (Reconnaissance) complete — findings below are *evidence-backed*, not inferred.
**Auditor:** Claude
**Date opened:** 2026-08-06
**Toolchain used:** rustc/cargo 1.97.1 (matches `rust-toolchain.toml`), Node 22.22.2
**Document hierarchy honoured:** `team_prompts/PROMPT_X.md` + `onyx_handover_v1.0.yaml` = authoritative; Blueprint = reference; `ONYX_Increment_X_*.md` = historical only.

---

## 1. Scope Inventory (measured, not estimated)

| Area | Measure |
|---|---|
| Rust crates in workspace | 33 |
| Rust source files / LOC | 243 files / ~30,509 lines |
| TypeScript (web-ui) files / LOC | 65 files / ~3,016 lines |
| Flutter mobile | `mobile/` (Dart + Kotlin/Swift FFI shims) |
| SQL migration files | 13 (Postgres + SQLite) |
| Rust tests (`#[test]`/`#[tokio::test]`) | 344 |
| Property tests (`proptest!`) | 3 |
| `unsafe` occurrences | 91 (concentrated in mobile FFI) |
| `.unwrap()`/`.expect()` in non-test code | ~120 |
| Deployment assets | Docker (5), Helm (3 charts), Terraform |
| Runbooks | 7 |

---

## 2. Verified Findings

Severity: **C**=Critical, **H**=High, **M**=Medium, **L**=Low.
Status: OPEN unless stated.

### C-01 — Repository does not compile as delivered
**Evidence:** `cargo check --workspace --all-targets`
```
error[E0761]: file for module `handlers` found at both
"crates/applications/client-composition/src/handlers.rs" and
"crates/applications/client-composition/src/handlers/mod.rs"
```
Root `_DELETE_FILES.txt` contains exactly one line instructing deletion of the stale
`handlers.rs` (mtime Aug 4) in favour of the `handlers/` directory module (mtime Aug 5).
This cleanup step was never applied to the delivered archive.
**Impact:** Clean-clone CI and any fresh build fail immediately.
**Fix:** Delete `crates/applications/client-composition/src/handlers.rs`; remove `_DELETE_FILES.txt`.
**Status:** REPRODUCED AND FIX VERIFIED IN SANDBOX (see §4).

### C-02 — E2E test suite fails to compile
**Evidence:** `cargo check --workspace --all-targets --exclude desktop-shell`
```
error[E0505]: cannot move out of `state` because it is borrowed
  --> tests/end-to-end/test_harness.rs:128:15
```
`state` is borrowed at line 103 (`let public_id = state...`) then moved at line 128 (`.bind(state)`).
**Impact:** The end-to-end suite has not been executed under toolchain 1.97.1. Release
sign-off evidence that depends on E2E results is therefore unproven.
**Fix:** Clone/extract `public_id` before the move, or reorder so the borrow ends first.

### H-01 — Authentication is demo-grade, not production-grade
**Evidence:** `crates/bins/api-server/src/routes/mod.rs:49-53`
```rust
pub const ORGANIZATION_ID: &str = "11111111-1111-1111-1111-111111111111";
pub const USER_ID:         &str = "22222222-2222-2222-2222-222222222222";
pub const DEFAULT_USERNAME:&str = "operator";
pub const DEFAULT_PASSWORD:&str = "onyx";
```
`routes/auth.rs::login` compares credentials with `!=` against these constants.
**Sub-findings:**
- H-01a: Hardcoded credentials compiled into the binary.
- H-01b: No password hashing (no Argon2id/bcrypt); plaintext comparison.
- H-01c: Non-constant-time comparison → timing oracle.
- H-01d: Single hardcoded user and single hardcoded organization — no user store, no
  real multi-tenancy despite tenant checks existing downstream.
**Fix:** Introduce a real identity store (users table + Argon2id via `argon2` crate),
constant-time verification (`subtle`), and remove all credential constants.

### H-02 — Token revocation is in-memory and non-durable
**Evidence:** `revoked_tokens: Arc<RwLock<HashSet<String>>>` (`routes/mod.rs:63,192`).
**Impact:** Revocation (logout) is lost on restart and is not shared across replicas.
Given the Helm chart runs `replicaCount` > 1 with canary rollouts, a logged-out token
remains valid on every other pod. Effectively, logout does not work in production.
**Fix:** Back revocation with Redis/Postgres keyed by `jti` with TTL = token exp.

### H-03 — Fully permissive CORS
**Evidence:** `routes/mod.rs:240-243`
```rust
CorsLayer::new().allow_origin(Any).allow_headers(Any)
```
**Impact:** Any origin may invoke the API. With `Authorization: Bearer` tokens held in
browser-accessible storage, this materially widens XSS/token-exfiltration blast radius.
**Fix:** Explicit origin allow-list driven by environment config.

### H-04 — Known-vulnerable / EOL dependency versions
**Evidence:** `Cargo.toml` workspace deps + cargo future-incompat report.
- `sqlx = "0.7"` (resolved 0.7.4). Cargo reports: *"packages contain code that will be
  rejected by a future version of Rust: sqlx-postgres v0.7.4"*. sqlx < 0.8.1 is also
  subject to a published binary-protocol advisory (RUSTSEC-2024-0363).
- `rustls = "0.21"` — superseded (0.23 line current); 0.21 is out of support.
- `quinn = "0.10"`, `reqwest = "0.11"`, `tokio-tungstenite = "0.20"`, `base64 = "0.21"`,
  `axum = "0.7"` — all one or more majors behind.
**Fix:** Staged upgrade with `cargo audit`/`cargo deny` gating in CI.
**Note:** Exact advisory applicability to be confirmed by running `cargo audit` in Phase 1.

### M-01 — Bearer token transmitted in WebSocket query string
**Evidence:** `routes/events.rs:19-24` — `Query(params): Query<WebSocketAuth>` with `token`.
**Impact:** Credentials land in access logs, proxy logs, and browser history.
Tenant isolation itself *is* correctly enforced (`TENANT_MISMATCH` check) — this is
about credential leakage, not authorization.
**Fix:** Use `Sec-WebSocket-Protocol` or a short-lived single-use ticket.

### M-02 — Auth enforced per-handler rather than by route layer
**Evidence:** `authenticate_headers` is called individually inside `command.rs`,
`query.rs`, `auth.rs::logout`, and the rate-limit middleware.
**Impact:** Auth is opt-in per handler; a future route that forgets the call is silently
public. This is a latent-defect pattern, not a current hole (all current routes are covered).
**Fix (Context7-confirmed idiomatic axum):** `Router::route_layer(middleware::from_fn_with_state(auth))`
with the authenticated principal injected via request extensions and extracted by handlers
as `Extension<AuthenticatedUser>`. `route_layer` is specifically correct here so that
unmatched paths return 404 rather than 401.

### M-03 — `ONYX_TEST_MODE` allows client-driven error injection
**Evidence:** `routes/mod.rs:487-511` — when `ONYX_TEST_MODE=1`, the client-supplied
header `x-onyx-test-status` forces 429/500 responses.
**Impact:** If the variable is ever set in a production-like environment, any client can
induce server errors on demand. Fail-open on env misconfiguration.
**Fix:** Compile out under `#[cfg(feature = "test-endpoints")]` rather than runtime env.

### M-04 — `web-ui` has no lockfile
**Evidence:** `web-ui/package-lock.json` absent; all deps use floating `^` ranges.
**Impact:** Builds are not reproducible; `npm ci` is impossible; supply-chain drift is
unbounded between builds.
**Fix:** Commit a lockfile; switch CI to `npm ci`; add `npm audit` gate.

### M-05 — Panic surface in long-running services
**Evidence:** ~120 `.unwrap()`/`.expect()` outside test code, concentrated in:
`client-composition/src/sync_agent.rs` (19), `event_bus.rs` (11),
`platform-kernel/src/causality.rs` (9), `query_registry.rs` (7),
`transports/sync-transport/src/cloud_relay.rs` (5).
**Impact:** In sync/transport hot paths, a panic on malformed peer input is a
remote availability risk.
**Fix:** Triage each; convert recoverable cases to typed errors. Not all are defects —
some are provably-infallible invariants and should be documented as such.

### M-06 — `desktop-shell` cannot build in a headless/CI environment
**Evidence:** `gdk-sys v0.18.2` build fails — `gdk-3.0` not found.
**Impact:** Desktop client is excluded from any Linux CI job lacking GTK/WebKit dev
packages, so it is effectively unverified by automation.
**Fix:** Install `libgtk-3-dev`/`libwebkit2gtk-4.1-dev` in the CI image, or isolate
desktop-shell behind a dedicated job.

### L-01 — Thin property-test coverage for CRDT correctness
**Evidence:** Only 3 `proptest!` invocations against a CRDT core of RGA, OR-Set,
MV-Register, PN-Counter, LWW-Register, append-only log, and tombstone GC.
**Impact:** Convergence/commutativity/idempotence laws are under-exercised relative to
the risk they carry in a local-first sync system.
**Fix:** Law-based property suite per CRDT + randomized concurrent-merge fuzzing.

---

## 3. Explicitly Verified as SOUND (no action)

These were tested and found clean — recorded so effort is not re-spent:
- **No hardcoded secrets** matching credential patterns outside the auth constants in H-01.
- **No SQL injection surface** — no `format!("SELECT ...")` string-built queries; sqlx
  parameter binding used throughout; `.sqlx/` offline metadata present.
- **No insecure TLS in production code** — `SkipVerify`/`ServerCertVerifier` bypass exists
  *only* in `tests/quic_nat_tests.rs`, correctly scoped to tests.
- **Container hardening is good** — non-root `USER 10001`, `tini` as PID 1, multi-stage
  build, `--locked` release build, healthcheck defined.
- **Kubernetes posture is mature** — Argo Rollouts canary with automated Prometheus
  error-rate analysis and gated promotion, readiness/liveness probes, resource limits,
  `podSecurityContext`, secrets via `secretRef`.
- **Production Rust code compiles cleanly** — 0 errors, 0 lint warnings across the
  workspace (libs + bins, excluding desktop-shell) once C-01 is fixed.
- **WebSocket tenant isolation** is correctly enforced.
- **Operational documentation is genuinely present** — 7 runbooks, go-live checklist,
  sign-off template, OpenAPI spec.

---

## 4. Change Log (sandbox modifications)

Per the working agreement, every codebase edit is registered here.

| # | Change | Rationale | Authority | Status |
|---|---|---|---|---|
| 1 | Deleted `crates/applications/client-composition/src/handlers.rs` | Resolves C-01 build break | Mandated by repo's own `_DELETE_FILES.txt` | **DONE & VERIFIED** |
| 2 | `tests/end-to-end/test_harness.rs` — `public_id` changed to `.to_owned()` | Resolves C-02 (E0505) | Audit fix | **DONE & VERIFIED** |
| 3 | `Cargo.toml` — added `argon2`, `password-hash`, `subtle` workspace deps | Required for H-01 | Audit fix | DONE |
| 4 | NEW `security-application/src/ports/user_store.rs` — `UserStore` port | H-01 identity contract | Audit fix | DONE |
| 5 | NEW `security-adapter/src/password.rs` — Argon2id + constant-time compare | H-01 core fix | Audit fix | DONE (8 tests pass) |
| 6 | NEW `security-adapter/src/user_store.rs` — Postgres + SQLite stores | H-01, per decision "Both Postgres and SQLite" | Audit fix | DONE (6 tests pass) |
| 7 | NEW migrations `20260105000000_add_users.{up,down}.sql` (postgres + sqlite) | H-01 schema | Audit fix | DONE |
| 8 | `security-adapter/src/lib.rs`, `ports/mod.rs` — module registration | Wiring | Audit fix | DONE |

### Verification evidence

* **C-01 + C-02:** `cargo check --workspace --all-targets --exclude desktop-shell`
  → `Finished dev profile ... ` **0 errors**.
* **Deletion safety (C-01):** public surface of the deleted `handlers.rs` was diffed against
  `handlers/`: both expose exactly `MissionCreationHandler`, `TaskCreationHandler`,
  `MissionDecisionHandler`, `TaskDecisionHandler`, plus private `RandomIdGenerator` and
  `creation_decision_context`. 1:1 match — the directory module is a documented structural
  refactor with no behaviour change. Nothing was lost.
* **Regression baseline:** `cargo test --workspace --exclude desktop-shell --lib`
  → **100 passed, 0 failed**.
* **New security modules:** `cargo test -p security-adapter` → **14 passed, 0 failed**.

### Decision log (owner-confirmed, this session)

| Q | Decision |
|---|---|
| Authorization model | **Uniform scope retained** — this change is authentication only. `is_admin` gates user-management endpoints only; it is not a general role. |
| Bootstrap mechanism | **Admin-only user-management API endpoints** |
| User store backends | **Both Postgres and SQLite** |
| Contract latitude | Amendment **authorised** by owner for auth. |

**Verification of change 1:** after deletion, `cargo check --workspace --exclude desktop-shell`
→ `Finished dev profile ... 0 errors, 0 warnings`. Confirms the deletion is the correct and
sufficient fix, and that no code depended on the stale file.

---

## 5. Open Questions for the Project Owner

These are undecided/ambiguous and I will **not** guess:

1. **Identity model (blocks H-01).** Is ONYX intended to have real multi-user auth, or is
   single-operator by design with identity delegated to an upstream IdP/gateway? The
   handover contract must govern this — I need the authoritative answer before redesigning
   the auth surface.
2. **Scope of dependency upgrades (H-04).** `axum 0.7→0.8`, `rustls 0.21→0.23`, and
   `sqlx 0.7→0.8` are breaking. Do you want a security-minimal upgrade (sqlx + rustls
   only) or a full modernization pass?
3. **Frozen-contract latitude.** `team_prompts/PROMPT_X.md` are described as *frozen
   contracts*. Several fixes (auth redesign, route-layer refactor) alter API-adjacent
   behaviour. Am I permitted to propose contract amendments, or must remediation stay
   strictly within the frozen surface?
4. **`team_prompts/` and `onyx_handover_v1.0.yaml` were not in the archive.** I have the
   `Team_Prompt_*.md` and handover markdown in project knowledge, but not the machine-
   readable YAML. Please confirm which artifact is authoritative for contract verification.
5. **Target environment.** Is production Kubernetes (Helm/Argo present) the sole target,
   or must desktop/mobile ship on the same cadence? This sets M-06's priority.

## 6. Session 3 — Pitch-Build Verification & Additional Fixes

| # | Change | Rationale | Status |
|---|---|---|---|
| 9 | `tests/end-to-end/approval_workflow.rs` — replaced hardcoded `operator`/`onyx` login with `bootstrap_and_login()` | H-01 follow-through: caller broke after credentials were removed | DONE |
| 10 | `tests/end-to-end/test_harness.rs` — added `bootstrap_and_login()` helper + `TEST_ADMIN_*` constants | Reusable across E2E tests | DONE |
| 11 | `web-ui/src/pages/Login/index.tsx` — removed prefilled `operator`/`onyx` from login form state | The shipped UI was publishing a working login to anyone who opened the page | DONE |
| 12 | `web-ui/tests/mocks/server.ts`, `tests/test-utils.tsx` — mock credentials aligned to `test-admin` fixture | Consistency with real server post-H-01 | DONE |
| 13 | `web-ui/tests/e2e/real-server.test.ts` — credentials now env-driven (`ONYX_E2E_USERNAME`/`PASSWORD`), with bootstrap fallback | Hardcoded creds would always 401 now | DONE |
| 14 | `web-ui/vite.config.ts` — added explicit `provider: 'v8'` to coverage config | **Live reproduction of finding M-04**: `vitest@^1.0.0` resolved to `1.6.1` on fresh install (no lockfile existed), and 1.6's coverage types now require `provider`. Confirms M-04 is a real, currently-active risk, not theoretical. | DONE |
| 15 | `web-ui/package-lock.json` — generated | Closes M-04: `npm ci` is now possible; future installs are reproducible | DONE |

### `npm audit` findings (informational, not blocking)
Generating the lockfile surfaced 12 vulnerabilities (1 critical, 7 high, 4 moderate)
in `vite`, `vitest`, `minimatch`, and `@typescript-eslint/*`. **Confirmed all are
devDependencies** (build/lint/test tooling) — the shipped runtime bundle
(react, axios, zustand, react-query) is unaffected. Tracked under **H-04**
(dependency modernization) for a future session; not a pitch-build blocker.

### Full-tree verification (this session, after all fixes)
| Target | Command | Result |
|---|---|---|
| Core workspace (lib+bin) | `cargo check --workspace --exclude desktop-shell --exclude e2e` | **0 errors** |
| E2E test crate | `cargo check -p e2e --all-targets` | **0 errors** |
| Desktop shell (Rust) | `cargo check -p desktop-shell` | **0 errors** |
| web-ui (TypeScript) | `npm run type-check` | **0 errors** |
| web-ui (production build) | `npm run build` | **succeeds**, 101.7 KB gzipped JS, passes bundle-size gate |
| Desktop UI (production build) | `npm run build` (Vite) | **succeeds**, 37 modules |

## 7. Session 4 — CI Pipeline Audit (`.github/workflows/ci.yml`)

User asked directly whether the CI file builds anything. It did not — four
distinct, individually reproduced problems.

| # | Finding | Reproduction | Fix |
|---|---|---|---|
| 16 | `check` job builds `--workspace --release` with no GTK/WebKit install step; `desktop-shell` is a workspace member | Directly hit `gdk-3.0 not found` on a bare image earlier this session — same class of runner | Added `apt-get install libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev` (+ webkit/appindicator/rsvg deps) before the toolchain build step |
| 17 | `web` job's "Patch Vite config for Vitest" step greps for `'"test":'` but `vite.config.ts` uses unquoted `test: {` — **the step has never fired, ever** | `grep -q '"test":' vite.config.ts` → no match, confirmed | Removed the dead step; root cause (vitest 1.6 coverage schema) already fixed at source in session 3 |
| 18 | `mobile-android` job: `android/build.gradle` pins AGP `8.5.2`; Flutter 3.44.8 stable requires ≥`8.6.0` | Ran `./gradlew tasks` for real after generating the scaffold → `FAILURE: ... Android Gradle Plugin version (8.5.2) is lower than Flutter's minimum supported version ... 8.6.0` | Bumped to `8.6.0` in `settings.gradle`. Re-ran: **that specific error is gone**, build now proceeds to (environment-only) SDK-location resolution, which is expected — this sandbox has no Android SDK installed; GitHub Actions runners do. |
| 19 | `ensure_platform_scaffold.sh` leaves both `build.gradle`/`build.gradle.kts`, `settings.gradle`/`settings.gradle.kts`, and two `MainActivity.kt` in different packages (`com.onyx` vs `com.onyx.onyx_mobile`) after regeneration | Confirmed by directly running the script and listing the resulting files. Gradle happened to prefer the Groovy variants this run (undocumented precedence), so this was **latent, not the active failure** | Deleted the redundant `.kts` duplicates and the orphaned template `MainActivity.kt`, leaving one unambiguous build definition |

### Correction to an earlier statement (session 2)
I previously told the user Android/iOS "cannot build" due to missing
`gradlew`/`Runner.xcodeproj`/launcher icons, and offered to hand-scaffold them.
That was incomplete — the team had already solved exactly that problem via
`mobile/tool/ensure_platform_scaffold.sh`, which backs up the hand-written
`MainActivity.kt`/`WorkManagerService.kt`/`AppDelegate.swift`, runs
`flutter create`, and restores them. Verified: running it produces a working
`gradlew` and `Runner.xcodeproj`, and the custom files survive intact. The
scaffold problem was already solved; the AGP version pin (finding #18) was
the actual, previously-undiscovered blocker underneath it.

### Files delivered this session
* `.github/workflows/ci.yml` — findings #16, #17
* `mobile/android/settings.gradle` — finding #18
* `mobile/android/` scaffold cleanup — finding #19 (not independently
  re-delivered; folded into the next full archive)

### Still unverified (flagged, not claimed)
* `verify_team7.sh` runs `cargo test -p team7-integration-tests --test integration`
  — crate confirmed to exist; the named integration test target itself was not
  individually executed this session.
* The full `mobile-android` job cannot be end-to-end verified without a real
  Android SDK, which this sandbox does not have. The reproduced AGP fix is
  confirmed as far as this environment allows.

## 8. Session 5 — Real CI Log Analysis (post-outage jobs)

User uploaded two real GitHub Actions job logs from runs after the platform-wide
outage cleared. Two genuine, unrelated failures — both root-caused by execution,
not inspection.

| # | Job | Failure | Root cause | Fix |
|---|---|---|---|---|
| 20 | `check` | `cargo fmt --check` exit 1, 44 diffs across 14 files | **Mixed**: 7 files were pre-existing formatting drift never caught before (this repo has no fmt-check history); 7 were files added/edited across sessions 2–4 (`password.rs`, `user_store.rs` ×2, `admin.rs`) that I never ran `cargo fmt` against. **This half is on me** — I verified compilation and test-passing throughout but never checked formatting. | `cargo fmt --all` across the whole workspace. Re-verified: `cargo check` 0 errors, `cargo test --lib` **114 passed, 0 failed** (up from 100 — the 14 new is expected: security-adapter's tests were previously counted separately and are now included in the same full-workspace run). |
| 21 | `web` | `tests/integration/commands.test.tsx` — `expected false to be true` on `result.current.isSuccess` | **Confirmed by direct execution, not inference.** Instrumented the test: `mutateAsync` resolved with real success data (`{"success":true,...}`), but `isSuccess` read immediately after was still `false` — a React Query state-transition timing issue, not a logic bug. Confirmed by comparing against the sibling `queries.test.tsx`, which already uses `waitFor(() => expect(result.current.isSuccess).toBe(true))` for exactly this reason; `commands.test.tsx` read the flag synchronously instead. | Changed the assertion to `await waitFor(() => expect(result.current.isSuccess).toBe(true))`, matching the established pattern. Re-verified: **passes**. |

### Explicitly ruled out during investigation (stated for the record, not left implicit)
* The credential-name change from session 3 (`operator`→`test-admin`) was
  **not** the cause of #21 — `authenticate()` in `test-utils.tsx` sets auth
  state directly via the Zustand store and never calls the mock `/api/auth/login`
  endpoint at all.
* `sessionStorage['onyx_user']`, which `useCommand.ts`'s `organizationId()`
  reads, **is correctly populated** by `authStore.login() → storeSession()`
  in both the real app and the test helper — confirmed by direct instrumentation.
  This was suspected as a possible production bug and directly disproven.
* The mock server's fixture notification ID/version matched the test's
  expectations exactly — not the cause.

### Full verification after both fixes
| Check | Result |
|---|---|
| `cargo fmt --check` | **0 diffs** |
| `cargo check --workspace --exclude desktop-shell --exclude e2e` | **0 errors** |
| `cargo test --workspace --exclude desktop-shell --exclude e2e --lib` | **114 passed, 0 failed** |
| `npm run type-check` (web-ui) | **0 errors** |
| `npx vitest run` (web-ui, full suite) | **130 passed, 7 intentionally skipped, 0 failed** |

## 9. Session 6 — Three New Real CI Logs (post-outage, new job types)

Three new logs — including, for the first time, `mobile-android` and
`mobile-ios` jobs actually completing enough to fail meaningfully (previously
blocked by the platform outage). Each root-caused by direct reproduction.

| # | Job | Failure | Root cause | Fix |
|---|---|---|---|---|
| 22 | `mobile-android` | `Both build.gradle and build.gradle.kts exist ... likely a mistake`, ultimately fails on `workmanager` Kotlin compile errors | **My AGP fix from session 4 is confirmed working** (Gradle now runs at 8.6.0 with only a soft deprecation notice, not a failure) — but `mobile/tool/ensure_platform_scaffold.sh` regenerates the `.kts` duplicate files from `flutter create` on **every fresh CI checkout**, so my earlier one-time manual deletion never survived into CI, which always starts from a clean clone | Moved the cleanup into the scaffold script itself (its permanent, correct location) so it runs every time the script does. Re-ran the script from a clean state in the sandbox: **confirmed only Groovy files remain, `gradlew` regenerates correctly.** |
| 23 | `mobile-android` (actual failure) | `Unresolved reference 'shim'/'registerWith'/'ShimPluginRegistry'` in `workmanager-0.5.2`'s own Android source | Confirmed via web search against `fluttercommunity/flutter_workmanager` issues #586 and #588 — exact matching error text. `workmanager 0.5.2` references Android's v1-embedding shim classes, removed by newer Flutter/AGP toolchains. Checked this project's own usage (`lib/background/android/workmanager_service.dart`) against the stable public API (`initialize`/`registerPeriodicTask`/`executeTask`) — unaffected by the fix. | Bumped `workmanager: ^0.5.0` → `^0.6.0` (fixes the shim removal without pulling in 0.9.x's unrelated breaking factory-pattern API change, per the package's own changelog). |
| 24 | `mobile-ios` | `Invalid Podfile: cannot load such file -- /opt/flutter/packages/flutter_tools/bin/podhelper` | **Not a repository bug.** The Podfile correctly resolves `flutter_root` dynamically from `ios/Flutter/Generated.xcconfig` at build time — verified by reading it. The actual cause: `Generated.xcconfig` (containing a **stale path from my own sandbox's Linux Flutter install**, `/opt/flutter`) had been accidentally included in the delivered zip. `mobile/ios/.gitignore` correctly lists `Flutter/Generated.xcconfig` as ignored — the project's own ignore rules are correct; my packaging in session 4/5 didn't respect `.gitignore` when zipping. | Deleted `ios/Flutter/Generated.xcconfig`, `flutter_export_environment.sh`, `.flutter-plugins-dependencies`, and `ios/Flutter/ephemeral/` from the delivered tree. On a real checkout `flutter pub get` regenerates these correctly, scoped to that machine's actual Flutter install. |
| 25 | `check` (clippy) | `bind_instead_of_map` in `observability-adapter/src/logging.rs` | Pre-existing, predates all sessions — first time `clippy -D warnings` has ever been run against this codebase in CI. Every match arm returned `Some(...)`, making `.and_then()` strictly more complex than `.map()`. | Applied clippy's own suggested rewrite. Re-verified: `cargo clippy -p observability-adapter -- -D warnings` clean. |

### Widening the clippy check surfaced a larger pre-existing gap — not yet fully resolved
Running `cargo clippy --workspace -- -D warnings` (rather than per-crate) after
fixing #25 surfaced **12 further pre-existing lint violations**, all predating
this project's audit sessions, concentrated in `security-adapter` and
`api-server`:

* Fixed immediately (mechanical, no design decision required):
  - `type_complexity` in `rate_limiter.rs` — factored into a named `RateLimitEventLog` type alias.
  - `unnecessary_lazy_evaluations` in `secret_provider.rs` — `.ok_or_else(|| X)` → `.ok_or(X)` per clippy's own suggestion.
* **Deliberately NOT fixed yet, flagged for owner decision:**
  - `result_large_err` (8 occurrences in `routes/command.rs`) — clippy reports
    `ApiError` is ≥136 bytes, and recommends `Box<ApiError>` or boxing its large
    fields. This is a genuine, worthwhile perf finding, but changing `ApiError`'s
    shape touches every `Result<_, ApiError>` call site across the API surface —
    a wider, more deliberate change than the mechanical fixes in this session.
  - `collapsible_match` in `query_handler.rs:275` — a real but low-risk simplification.
  - `useless_conversion` — one occurrence, not yet triaged.

**This is now flagged as its own item, not silently deferred** — see open
questions below.

### Verification after fixes applied this session
| Check | Result |
|---|---|
| `mobile/tool/ensure_platform_scaffold.sh` re-run from clean state | Only Groovy Gradle files present, `gradlew` regenerated, custom Kotlin/Swift files preserved |
| `cargo clippy -p observability-adapter --lib -- -D warnings` | **Clean** |
| `cargo clippy --workspace --exclude desktop-shell --exclude e2e -- -D warnings` | **12 pre-existing violations found and reported to owner; 2 fixed, 10 deferred pending scope decision (see below)** |

## 10. Open Question — `result_large_err` Scope

`ApiError` is ≥136 bytes and clippy's `-D warnings` gate fails 8+ call sites on
`result_large_err`. Three ways to resolve, in increasing order of invasiveness:

1. **`#[allow(clippy::result_large_err)]`** at the affected functions — fastest,
   documents the decision, changes nothing structurally. Reasonable if `ApiError`
   is rarely on a hot path (it's constructed on error paths only, not success paths).
2. **`Box<ApiError>`** the return type — clippy's literal suggestion. Touches every
   call site that constructs or matches `Result<_, ApiError>` across `api-server`.
3. **Shrink `ApiError` itself** — e.g. box `ApiErrorBody`'s `Value` field. Smallest
   runtime cost, largest diff.

I have not picked one — this is exactly the kind of "undecided, ask" case per
your standing instruction. Which approach do you want?

### Resolution — Session 7

**Owner decision: option 1, `#[allow(clippy::result_large_err)]`.**

Implemented as a single crate-level `#![allow(clippy::result_large_err)]` in
`crates/bins/api-server/src/lib.rs`, with an inline doc comment explaining the
rationale (all 11 call sites are per-request, error-path-only helpers in
`routes/{admin,command,events,mod}.rs`; none are hot-path; `ApiError` is
always consumed immediately by `IntoResponse`, never threaded through many
stack frames). One allow, documented once, rather than 11 scattered
per-function annotations — chosen to match this file's existing convention
of a single provenance/rationale doc comment at the crate root.

`main.rs` was checked and confirmed to never construct or return `ApiError`
directly (it only imports `router`/`ApiState` from the `routes` module), so
the bin target needed no separate annotation.

**Verification status:** implemented but **not yet re-run through
`cargo clippy`** in this session — this sandbox has no Rust toolchain
installed (`rustc`/`cargo` not found). This should be verified with
`cargo clippy -p api-server -- -D warnings` on a machine with the pinned
1.97.1 toolchain before being counted as closed. Flagging this explicitly
rather than claiming verification I did not perform.
**Status:** CLOSED (implementation) / VERIFICATION PENDING (toolchain unavailable in this session).
