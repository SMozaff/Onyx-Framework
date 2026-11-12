# ONYX Slint Migration — BOUND Decision Log

**Status:** Gate 0b OPEN — decisions below do not constitute Gate 0b sign-off.  
**Repository baseline inspected:** `913a36a5606c0241a15104cebae2c915997ea02b` (main, 2026-10-27).  
**Decision update:** 2026-10-27.  
**Authority:** User-confirmed decisions in the ONYX project conversation.  
**Rule:** Preserve unresolved items as blockers; do not infer approval from recommendations or investigations.

## Confirmed decisions

### P-01 — Desktop hosting architecture

- **Decision state:** Investigation authorized; final architecture not yet approved.
- **User decision:** Option B — separate native Slint host.
- **Authorized investigation:** B1 — separate executable reusing approved Rust backend crates in-process.
- **Boundary:** This authorizes feasibility investigation only. It does not authorize replacing the Tauri default, deleting Tauri/React, freezing the architecture, or claiming Gate 0b passed.
- **Next evidence required:** dependency graph and feature/build validation; shared composition/session boundary; authority and token ownership; event/subscription lifecycle; startup/shutdown/restart/error behavior; data-directory and packaging behavior; cross-platform build and smoke-test evidence.
- **Current finding:** `client-composition` is a reusable workspace library and `AppState` exposes command/query/event registries, but `desktop-shell` currently owns Tauri-bound session orchestration, secure storage wiring, and the composition setup. A Slint host should not depend on the Tauri shell as its runtime. Extracting a shell-independent runtime/session boundary is likely required. See [P-01 B1 investigation](P-01-B1-investigation-2026-10-27.md).

### Q-01 — Admin token persistence

- **Decision state:** Approved.
- **Decision:** Keep tokens in memory only; do not persist them to disk or an OS keyring.
- **Contract obligations:** tokens must not be exposed to Slint properties/UI state, logs, diagnostics, crash reports, or disk; define expiry, refresh, logout, revocation, and process-restart reauthentication behavior; add tests for all of these.

### Q-05 — Profile team-membership representation

- **Decision state:** Approved.
- **Decision:** Preserve the existing list/API contract (`Vec<String>` in the Rust DTO) and the existing semicolon-delimited editor behavior.
- **Constraints:** Do not invent a team picker or team catalog without a verified catalog and authorization contract. Test whitespace, empty and duplicate entries, semicolon-containing values, serialization round trips, and server validation/authorization.

### Q-14 — Theme and language preferences

- **Decision state:** Approved.
- **Decision:** Persist theme and language preferences across launches.
- **Remaining contract work:** define storage location, schema/versioning, defaults, corruption fallback, and tests.

### Q-16 / U-05 — ID digit display

- **Decision state:** Approved.
- **Decision:** Display Latin digits for IDs. This is display-only; canonical values, serialization, identifiers, and API data remain unchanged.
- **Remaining contract work:** UI formatting tests, including mixed RTL/LTR contexts.

### U-04 — Policy lifecycle and transitions

- **Decision state:** Approved, subject to implementation evidence and Worker/Rust parity tests.
- **Accepted model:** Policy lifecycle `Active -> Retired`, with Retired terminal. Policy versions move `Draft -> Published` and published versions are immutable; at most one pending draft. Evaluation and violation registration require an Active policy with a published version. An unknown rule key returns an explicit missing/null result, not a silently coerced false value. Mutations must use expected version/epoch checks; on conflict, reread and require fresh user intent. Do not blindly replay state-changing operations. Retry an ambiguous create only with the same idempotency key and payload when the endpoint explicitly supports idempotency.
- **Current evidence:** Rust policy aggregate rejects `EvaluatePolicy` and `RegisterViolation` on Retired policies. Worker command semantics require Active for both; existing Worker tests assert rejection. The Worker test's comments describing a Rust mismatch are stale and should be corrected without changing the behavior.
- **Remaining work:** parity tests for lifecycle, published-version requirement, missing rule keys, draft uniqueness, optimistic concurrency, and error mapping. Review exact endpoint behavior before changing production semantics.

## Gate 0b blockers — still open

These items are not closed by the decisions above:

1. **P-01:** complete B1 architecture evidence and approve the final boundary, state/session authority, typed interfaces, event behavior, failure/restart semantics, packaging, and exact implementation scope.
2. **Q-01:** implement and test the in-memory token contract and lifecycle behaviors.
3. **Q-14:** freeze persisted preference schema, defaults, fallback, and tests.
4. **Q-05:** implement contract-level validation and round-trip tests; confirm actual server validation behavior.
5. **Q-16/U-05:** verify display-only Latin-digit formatting without changing canonical data.
6. **U-01:** reproducible Windows/Linux table sorting/paging, multiline editor, Persian IME composition, and mixed RTL/LTR cursor/selection evidence.
7. **U-02:** Windows UI Automation and Linux AT-SPI/screen-reader evidence.
8. **U-03:** bundled font source/licensing, Latin/Persian glyph coverage, shaping samples, and per-OS render evidence.
9. **U-04:** complete Worker/Rust parity tests and resolve version/epoch/error behavior against actual command handlers.
10. **U-10:** reduced-motion source/fallback and per-OS evidence.
11. **U-12:** complete dark-theme tokens, WCAG AA contrast pairs, and automated report.
12. **C-01:** re-pin API contracts to current Worker routes/handlers and review; existing embedded baseline is stale.
13. **C-03:** review/version typed error mapping.
14. **C-09:** approve fixture schema and provenance.
15. **Governance:** identify integration coordinator; record exact baseline, accepted evidence, frozen contract versions, approved fallbacks, approver/date, and scoped authorization in the formal Gate 0b sign-off.

## Baseline and governance notes

- Main baseline inspected: `913a36a5606c0241a15104cebae2c915997ea02b`.
- The plan-referenced canonical decision log did not exist on this baseline; this file establishes the proposed canonical record in the expected `docs/migrations/slint/` area.
- The plan path `common/ONYX_Slint_Migration_BOUND_Plan_Execution_Ready.md` returned 404 on the current main branch. The supplied planning artifact remains the governing source for this work, but its in-repository canonical location must be reconciled.
- Existing Slint feasibility CI is evidence for a spike only; it is not proof of full production UI, Persian IME, accessibility, font shaping, reduced motion, or theme-contrast readiness.
- Do not start B-01 production Worker-client implementation until Gate 0b's required evidence and formal approval are complete.

## Gate 0b formal sign-off record

**Status:** Owner approval recorded; Gate 0b NOT passed. Blockers 1–14 above remain open, and the required fields below are incomplete.

| Field | Value |
|---|---|
| Owner approval (verbatim) | "Approved for full migration" |
| Approver | Project owner (fanoos.media@gmail.com) |
| Approval date | 2026-10-29 |
| Scope | Full migration (Admin and Desktop, Slint), per the BOUND plan |
| Exact baseline | origin/main `9ac3964` (PR #342 and #343 merged) |
| Integration coordinator | **OWNER TO PROVIDE** |
| Accepted evidence (blockers 1–14) | **OWNER TO PROVIDE** — none accepted yet |
| Frozen contract versions | **OWNER TO PROVIDE** |
| Approved fallbacks | **OWNER TO PROVIDE** |

**Effect:** Tauri/React remains the production default. No production change, B-01 implementation, or Worker deploy follows from this record alone.

## Owner decisions recorded 2026-10-29

- **P-01 (desktop boundary):** Owner approved the boundary. Reference: [P-01 B1 investigation](P-01-B1-investigation-2026-10-27.md). The boundary text is not yet committed as its own file; it must be added before Gate 0b evidence review.
- **Q-14 (preference schema):** Owner approved the schema. The schema itself must be committed (fields, defaults, fallback) before it can be frozen.
- **C-09 (fixture schema):** Owner approved the fixture schema. Provenance must be committed alongside it.
- **C-03 (typed error mapping):** Owner approved the mapping. Reference the committed mapping file when it exists.

**Gate 0b status:** still OPEN. Approvals here do not close blockers 1–14 and do not pass the gate.

**Open:**

## Gate 0b coordination and font (2026-10-29)

- **Bundled font (U-03):** Vazirmatn, SIL Open Font License 1.1. Owner confirmed the font and its license.
- **Integration coordinator (blocker 15):** Claude (AI assistant), named by the owner. Coordinates evidence collection and keeps this record current. The owner remains the approver of record for Gate 0b sign-off.

## U-04 error codes (2026-10-31)

- **Decision:** Policy precondition failures use the specific codes `DRAFT_ALREADY_PENDING`, `NO_DRAFT_VERSION`, and `NO_PUBLISHED_VERSION` on the API. The Worker's generic `COMMAND_PRECONDITION_UNSUPPORTED` is not used for these cases.
- **Status:** Decided. Worker implementation pending.
- Source matrix: `docs/migrations/slint/u04-policy-parity.md`.
## C-09 fixture provenance (2026-10-31)

- **Recorded:** Inventory of fixtures with git provenance in `docs/production/fixture-provenance.md`.
- **OPEN:** The approved fixture schema is not identified in the repo. Owner to confirm which schema (candidate: `shared/diagnostics/diagnostic-event.schema.json`, version 1.0).
## C-01 API contract re-pin (2026-10-31)

- **Decision:** `docs/production/api-contract.md` re-pinned to main `98a2258`. Verified by the route-completeness test (42 passing).
- **Reason:** The contract's routes already matched the Worker. Only the baseline commit was stale.
- **Not changed:** Historical B-01 reports keep their original baseline. They record what was reviewed at that time, and rewriting them would misstate the history.

## Q-05 team names and semicolons (2026-10-31)

- **Decision:** Team names containing `;` are rejected at input. Team member lists keep the `;`-separated format.
- **Reason:** `;` is the separator in `team_ids` and the editor list. A name containing it would be split into two entries. Team IDs (`tm…`) never contain `;`, and no legitimate name needs it.
- **Decided by:** Coordinator, per the owner's standing instruction to decide minor matters and document them. Owner informed.

## Future priority: diagnostics collection (after migration, 2026-10-31)

- **Status:** Not started. Deliberately deferred until the Slint migration is complete.
- **Goal:** Store diagnostic events (`shared/diagnostics/diagnostic-event.schema.json`, v1.0) on Cloudflare so failures can be counted by `error_code` and component.
- **Current state:** Admin Shell login events go only to the browser console and a window event. Worker events are not collected.
- **Candidate approach (not decided):** Cloudflare-native logging. Volume is small and needs no computation. Options to evaluate: Workers Logs, Logpush, or a D1 table.
- **Owner:** To be assigned when the migration closes.

## U-10 motion preference fallback (2026-10-31)

- **Decision:** If the OS motion setting is unknown or unreadable, the app uses reduced motion.
- **Reason:** Reduced motion is the safer default for users who may need it. It is also the value `MotionPreference::resolve(None)` already returns in `crates/applications/client-composition/src/motion.rs`.
- **Decided by:** Coordinator, per the owner's standing instruction. Owner confirmed on 2026-10-31.
- **Still to do:** Per-OS readers and Slint wiring.

## Staff accounts on import (owner decision, 2026-11-01)

- Import creates an account for each staff row and assigns its staff ID. Single admin creation follows the same rule.
- Sign-in uses the Onyx ID. Each new account gets a system-generated one-time password, which must be changed at first login.
- This replaces the 2026-08-13 rule that import does not create accounts.
- **Open:** How the one-time password reaches the admin. Passwords must not be stored, logged, or shown in chat.

## Creation report with one-time passwords (owner decision, 2026-11-01)

- Each batch import and each single creation produces an export report for the admin.
- The report includes all staff fields and each new account's one-time password.
- The system stores only password hashes. Plain passwords are never stored or logged.
- **Open:** report format (CSV or Excel) and the report's exact column list.

## Creation report format (owner decision, 2026-11-01)

- The creation report is an Excel (.xlsx) file.
- **Open:** exact column list.

## Creation report columns (owner decision, 2026-11-01)

- Columns: the staff sheet fields, plus the assigned `onyx_id`, the one-time password, and the account status.
- Format: Excel (.xlsx).

## Postponed: staff account import and creation report (owner decision, 2026-11-01)

- **Decision:** Postponed until after the Slint migration. **Superseded (2026-11-01):** the owner later allowed this lane to run in parallel. See "Backend accounts: decisions".
- **Scope when resumed:** Batch import and single creation create accounts with Onyx ID sign-in, a one-time password, forced change at first login, and the Excel creation report. Rules are recorded in `docs/product/staff-and-team-rules.md`.
- **Reason:** The migration plan keeps the Worker and D1 as the identity authority and puts backend changes out of scope.
- **Open items to resolve when resumed:** Worker-side password hashing (PBKDF2 or a library for Argon2id) and the one-time password delivery in the report.

## Backend accounts lane: status (2026-11-01)

- **Status:** Current-state review, draft migration, draft report spec and pending tests are on branch `feat/backend-accounts`. No decision is recorded here.
- **Open for owner:** O-1 to O-6 in `docs/migrations/slint/backend-accounts-current-state.md`.
- **Conflict to confirm:** The entry "Postponed" above says the lane waits until after the migration. The handover says the owner allowed it to run in parallel on the same date.

## Backend accounts: decisions (owner approved "decide and document", 2026-11-01)

Decided by the coordinator under the owner's standing authority. Owner may override any item by a new entry.

- **O-1 Hashing:** Argon2id via the existing Worker library (`argon2-wasm-edge`). Reason: already in use, memory-hard, matches the Rust side.
- **O-2 Account store:** D1 is the account authority. Reason: the migration plan makes the Worker and D1 the identity authority. The Rust `UserStore` is not written.
- **O-3 Sign-in identifier:** The Onyx ID becomes the login identifier. The username and email stay accepted as aliases during transition. Reason: the owner requires sign-in with the Onyx ID.
- **O-4 Forced password change:** A `must_change_password` flag on the account. When set, `requireSession` allows only the password-change and logout routes; all others return `PASSWORD_CHANGE_REQUIRED` (409). The flag clears on a successful change. Reason: simplest enforcement that the Worker already applies centrally.
- **O-5 Blank `onyx_id` on import:** Every import row creates an account and gets a generated staff ID. A supplied `onyx_id` is ignored and reported in the row result. Reason: owner's instruction, 2026-11-01.
- **O-6 Existing import:** Extended. `POST /api/admin/profiles/import` accepts `.xlsx` as well as CSV and JSON, and creates accounts per row. Reason: one endpoint for the owner's import flow.
- **Email:** Account email is required. For a row without `contact_email`, the system uses a generated placeholder address in the form `<onyx_id>@accounts.invalid`, flagged in the report. Reason: account creation requires an email; placeholder keeps rows importable and is marked for follow-up.
- **Password minimums:** Unchanged (8 on create, 12 on reset). One-time passwords are generated to satisfy the 12-character minimum.
- **Postponed entry:** Superseded by the parallel-lane decision above.
- **Excel report:** Columns as in `backend-accounts-creation-report-spec.md`. Spreadsheet formula-injection protection applies to every text cell.

## Rust toolchain in local sandboxes (2026-11-01)

- **Repo pin:** `rust-toolchain.toml` pins 1.97.1. CI and release workflows use 1.97.1. Unchanged.
- **Sandbox limit:** the coordinator sandbox cannot download 1.97.1 (egress policy blocks static.rust-lang.org). Local runs use the installed stable toolchain via `RUSTUP_TOOLCHAIN=stable`.
- **Consequence:** results from local sandbox runs are labelled "stable 1.97.0". Re-run on CI or on a machine with 1.97.1 before relying on them.

## C-01 API contract re-pin (2026-11-01)

- **Decision:** `docs/production/api-contract.md` re-pinned to main `d330451`. Route-completeness test: 42 passing on that commit.
- **Reason:** The contract matches the current Worker source. Only the baseline commit was stale. This closes the C-01 part of Gate 0b blocker 12.

## Q-05 implementation: team name validator (2026-11-01)

- **Implemented:** `validate_team_name` in `crates/domains/profile-domain/src/value.rs`. Rejects `;` (`TEAM_NAME_CONTAINS_SEMICOLON`) and empty or whitespace-only names (`TEAM_NAME_EMPTY`). Trims surrounding whitespace.
- **Tests:** 3 unit tests; profile-domain suite 12 passing.
- **Not yet wired:** no route accepts team names as input today. The CSV/JSON/XLSX import splits `team_ids` on `;`, so it cannot produce a `;` inside a name. Wire the validator when a team write path exists.

## P-01 owner decision: B1 approved conditionally (2026-11-04)

- **Decision:** Owner approved B1 (separate native Slint desktop host sharing Rust crates in-process), conditionally.
- **Condition:** Track A first. Replace the test-only `StaticAuthorityProvider` in the production relay path, with fail-closed logged-out behaviour, refresh/expiry/revocation handling, and tests. Track B (shell-independent runtime boundary and Tauri-free dependency proof) starts only after Track A passes.
- **Not decided by this entry:** token-storage policy for desktop vs Admin, and relay-only fallback when LAN discovery fails. These remain open owner decisions (see `P-01-evidence-summary-2026-11-01.md`, section 5).
- **Gate 0b:** blocker 1 is partly closed by this approval. Gate 0b itself is still not passed.

## RustSec advisories: root-cause fix (2026-11-06)

- **Correction:** The earlier plan to upgrade Slint was wrong. Slint 1.18.1 is the latest published version. The failing advisories were not caused by Slint.
- **RUSTSEC-2026-0436 (high, 7.5):** `calamine` 0.37.0 panics or allocates without bound on malformed CFB/OLE2 input. ONYX parses staff spreadsheets uploaded by admins, so this is a real input path. **Fix:** upgrade `calamine` to 0.38.0 (workspace dependency). Parser tests pass unchanged; a regression test for malformed OLE2 input was added.
- **RUSTSEC-2026-0433 (high, 7.5):** `pulldown-cmark` 0.13.5 (pulled in by Slint's markdown path) has quadratic parsing of nested wikilinks. **Fix:** lock bump to 0.13.6, which is the patched version the advisory names. No code change.
- **bincode 2.0.1:** an "unmaintained" warning only, not a failure. Tracked for later; not changed here.

## U-10 Windows reduce-motion reader implemented (2026-11-07)

- **Implemented:** `read_windows_animations` in `spikes/slint/src/logic.rs` calls `SystemParametersInfoW` with `SPI_GETCLIENTAREAANIMATION` (the "Show animations in Windows" setting). Returns `None` on call failure, so the reduced-motion fallback applies (U-10 decision).
- **Compile and test evidence:** Linux tests pass (8 unit tests). The Win32 path is compiled and built only on the `windows-latest` runner in `slint-spike-artifacts.yml`. The Windows result is the evidence for this row; it is not manual proof.
- **Still manual:** the setting must be toggled on a real Windows machine to confirm the live value changes (spike-results Windows row).
- **CI fix:** `slint-spike-artifacts.yml` pointed at `spike/slint-feasibility`, which no longer exists, and called a `packaging/build-deb.sh` script that does not exist. It now targets `spikes/slint`, uses the real binary name, and runs on changes under `spikes/slint/`.

### U-10 Windows follow-up (2026-11-07)

- First Windows CI run failed (E0432): `windows-sys` 0.48 needs the `Win32_Foundation` feature for `SystemParametersInfoW`. Root cause: the feature was missing in `spikes/slint/Cargo.toml`. Fixed in b6a06e4. Linux does not compile the `cfg(windows)` block, so only Windows CI could catch it.
- Windows CI green on b6a06e4 (`Build and test (windows-latest)`).
- Still manual: confirm the Windows setting toggles the live value on a real machine.

### PR #409 merge order (2026-11-07)

- PR #409 was merged at 2026-11-06T22:31:50Z on head 3083e78, while its first commit (85adc7b) had a failing Windows job. I did not trigger this merge. The fix (b6a06e4) and docs commit (3083e78) were already on the branch, so `main` (8a44ab9) contains the fix.
- Verified on `main`: `Build and test (windows-latest)` and `Build and test (ubuntu-latest)` both completed successfully on 8a44ab9.
- Process note: merge only after every required check on the head commit is green.

## File-send route selection (owner decision, 2026-11-07)

- **Decision:** Before a file is sent, the app asks the user to choose the route:
  1. **Local:** BLE and LAN.
  2. **Internet:** cloud and internet.
- **If Internet is chosen:** send directly over the internet.
- **If Local is chosen:** go through the relay path, with P2P connections as described by the owner.
- **Effect on open items:** this makes the "relay-only fallback when LAN discovery fails" question moot for file sends. The fallback is no longer automatic; the user chooses the route.
- **Open detail (not decided):** what "P2P connections, etc." covers in the Local route, and whether the choice applies to every file send or only the first.

## Track A and Track B status (2026-11-07)

- **Track A (production relay auth):** implemented and merged (PR #407). `desktop-shell` uses `SessionBearerProvider` on both the logged-in and logged-out paths. Logged-out returns `NoCredentials` (fail closed). Unit tests in `session_authority.rs` (16) and `session.rs` (17) cover fail-closed, logout, and restore-rejected paths. Still pending: the two live relay ticket checks (need deployed Worker and granted account). The P-01 evidence summary dated 2026-11-01 is stale on this point.
- **Track B step 4, dependency proof (partial):** `cargo tree -p desktop-slint --locked -e normal` contains 0 Tauri-family crates (`tauri`, `wry`, `tao`, `webkit2gtk`) out of 1901 crates. This is evidence for the dependency proof only; it does not cover startup, shutdown, or data-directory coexistence.
- **Track B open work:** `desktop-slint` still has its own `session_service.rs` and `authority.rs`, duplicating `desktop-shell`'s `session_authority`. Consolidating to one session owner (step 3) is the next chunk. The subscription and sync-task lifecycle on login, logout, and restart (step 3) is not defined yet.

## Track B session owner: step 1 (2026-11-07)

- **Decision:** the session authority (login, refresh rotation, logout, bearer provider, storage record) moves out of `desktop-shell` into a new shell-independent crate, `crates/applications/desktop-session`. Owner decision given to proceed. The crate names no Tauri, UI, or keyring type.
- **Moved, unchanged logic:** `session.rs`, `session_authority.rs`, and the `SecureStorage` port (`secure_storage.rs`). The keyring adapter stays in `desktop-shell` (platform code).
- **Evidence:** `desktop-session` tests: 34 passing. `cargo clippy -D warnings` clean for both crates. `desktop-shell` compiles.
- **Known environment failure, not caused by this change:** two `keyring_adapter` tests fail in this sandbox ("No default store has been set"). They fail the same way on `main`.
- **Not done yet (next step):** `desktop-slint` still runs its own in-memory `session_service.rs` and `authority.rs`. Switching it to `desktop-session` is step 2 and is a behaviour change (it would adopt the keyring-backed desktop token policy). It needs its own decision entry.

## Track B session owner: step 2, desktop-slint switched (2026-11-07)

- **Decision (minor, documented):** `desktop-slint` uses the shared `desktop-session` crate and removes its own `session_service.rs` and `authority.rs`. This is the one session owner the B1 condition requires.
- **Behaviour change:** the refresh token is now stored in the OS keyring (desktop token policy, `desktop-token-storage-policy.md`), where `desktop-slint` previously kept it in memory only. The app also restores a stored session at startup and starts the outbox for that organization. The access token stays in memory.
- **Keyring adapter:** moved into `desktop-session` behind a `keyring` feature. `desktop-shell` and `desktop-slint` both enable it. `desktop-shell` behaviour is unchanged.
- **Tests:** the four `authority.rs` tests and the `session_service.rs` tests are removed with their code. The same fail-closed, redaction, and logout behaviour is covered by `desktop-session` tests (34 total).
- **Evidence:** `cargo check` passes for `desktop-slint` and `desktop-shell`. `clippy -D warnings` is clean for `desktop-session` (with keyring), `desktop-slint`, and `desktop-shell`. The two keyring-adapter tests fail in this sandbox (no default credential store), as on `main`.
- **Not verified:** a real keyring round trip on Windows, macOS, or Linux desktop. Needs a device test.
- **Stacked:** this branch is based on PR #413. Merge #413 first.

## Track B steps 3 and 5: lifecycle and coexistence (2026-11-07)

- **Step 3, lifecycle (decided, minor):** `desktop-slint` has no event subscriptions. Its only lifecycle is the sync task (outbox):
  - Sign-in builds one `AppState` and starts the sync agent. `start_outbox` stops any previous task first.
  - Sign-out stops the sync task, then cancels in-flight relay connections via the shared cancellation token.
  - Startup restores a stored session and starts the same sync task.
  - Event subscriptions exist only in `desktop-shell` (Tauri `subscribe_events`). That path is out of Slint scope, and its existing gap stays recorded for the cutover gate.
- **Step 5, coexistence (defect found and fixed):** both hosts stored secrets under keyring service `com.onyx.platform` with the same keys. Installing both would make one overwrite the other's session. `desktop-slint` now uses service `com.onyx.platform.slint`. `desktop-shell` keeps `com.onyx.platform`. Data folders already differ (`onyx-slint` vs `com.onyx.platform`).
- **Still not done:** a device test that installs both hosts and confirms separate sessions. Startup and shutdown tests for `desktop-slint` need a runnable keyring and database, which this sandbox does not have.
- **Evidence:** `clippy -D warnings` is clean for `desktop-session` (keyring feature), `desktop-slint`, and `desktop-shell`. `desktop-session` tests: 34 pass. The two keyring-adapter tests still fail here (no default store).

## Keyring adapter: hermetic tests (fix for the two failing keyring tests, 2026-11-07)

- **Root cause:** the keyring tests depended on the machine's OS credential store. This sandbox and CI have none unlocked, so two tests failed with "No default store". The `keyring` crate's own store initialisation cannot be replaced by a test store, so the tests could not be fixed by changing only the test.
- **Fix:** the adapter takes an injected `CredentialStore` (`keyring-core`). `for_service` / `new` use the platform default (returning `Result`). `with_store` takes any store.
- **Tests:** round-trip, unknown-key, not-found, and service-isolation tests run against the in-memory mock store, one store per test. 38 pass. The real OS-store round trip is an ignored device test (`cargo test -p desktop-session --features keyring -- --ignored`).
- **Caller change:** `KeyringSecureStorage::new()` and `for_service()` now return `Result`. Callers in `desktop-shell` and `desktop-slint` map the error.
- **Dependency:** `keyring-core` is an optional dependency behind the `keyring` feature.
- **Not verified:** the device test on Windows, macOS, and Linux desktops (needs a real unlocked store).

## desktop-slint lifecycle tests (Track B step 4, 2026-11-08)

- **Change:** `SlintComposition::open` now delegates to `open_with(OpenOptions { data_dir, storage, api_origin })`. Production behaviour is unchanged: the same data folder, the `com.onyx.platform.slint` keyring service, and the production origin. Tests inject a temporary folder, an in-memory store, and an unreachable origin (`http://127.0.0.1:9`).
- **Tests (3, passing):**
  - Fresh start is signed out. Restart over the same folder keeps the replica identity and migrations are idempotent.
  - An offline restart does not sign the user in, and keeps the stored session record for the next launch (a network failure is not a verdict; matches `RefreshError::Unavailable` handling).
  - Logout with no session is idempotent.
- **Evidence:** `cargo test -p desktop-slint` passes. `clippy -D warnings` and `fmt --check` are clean.
- **Not covered:** a real successful login and refresh (needs a deployed Worker and a test account), and the two-host coexistence on a device.

## desktop-slint lifecycle tests (Track B step 4, 2026-11-08)

- **Change:** `SlintComposition::open` now delegates to `open_with(OpenOptions { data_dir, storage, api_origin })`. Production behaviour is unchanged: the same data folder, the `com.onyx.platform.slint` keyring service, and the production origin. Tests inject a temporary folder, an in-memory store, and an unreachable origin (`http://127.0.0.1:9`).
- **Tests (3, passing):**
  - Fresh start is signed out. Restart over the same folder keeps the replica identity, and migrations are idempotent.
  - An offline restart does not sign the user in, and keeps the stored session record for the next launch. A network failure is not a verdict (matches `RefreshError::Unavailable`).
  - Logout with no session is idempotent.
- **Evidence:** `cargo test -p desktop-slint` passes. `clippy -D warnings` and `fmt --check` are clean.
- **Not covered:** a real successful login and refresh (needs a deployed Worker and a test account), and two-host coexistence on a device.

## Ready-to-build items: status (2026-11-08)

- **Q-16 / U-05 (ID digit display): done.** `crates/bins/admin-shell/ui/src/utils/displayDigits.ts` converts Persian and Arabic-Indic digits to Latin for display only. `IdText` wraps the result in an LTR isolate for mixed RTL/LTR text. The stored value is never changed. 8 tests pass (`tests/displayDigits.test.ts`). The helper is not yet used by any screen, and it is not shared with the desktop UI.
- **File-transfer route placeholders: done (desktop).** `TransferRoutes` on the Files page shows Internet as enabled with the 100 MB limit, and LAN and the emergency BLE/Wi-Fi Direct module as disabled.
- **Settings notification placeholders: done (desktop).** Email and SMS are disabled options under "Notification channels", marked as future modules. Push is not part of this placeholder.
- **i18n:** new keys added in `shared/i18n/en.json` and `fa.json` (flat keys, to pass the schema). Dictionaries regenerated. The Persian strings need review by a Persian speaker.
- **Not done, needs owner input:**
  - **Q-14 (theme and language preferences):** the owner approved the schema, but it is not in the repo. Fields, defaults, and fallback must be provided before they can be implemented.
  - **C-09 (fixture schema):** the candidate is `shared/diagnostics/diagnostic-event.schema.json` (v1.0). The owner has not confirmed it, so no provenance commit was made.
  - **Web UI file route:** web-ui has no Files page. Adding one (Internet-only, LAN disabled) is new screen scope and needs a decision.

## CI: web-ui and mobile-pwa checks removed (2026-11-09)

- **Decision (owner):** remove the `web` and `mobile-pwa` jobs from every CI check. Web UI is postponed (`docs/product/web-ui-postponed.md`), and these jobs cost significant CI time.
- **Changes:**
  - `ci.yml`: deleted the `web` and `mobile-pwa` jobs, and their entries in `merge-gate` (`needs`, env, and result checks).
  - `security.yml`: removed `web-ui` and `mobile-pwa` from the frontend-audit matrix and from the lockfile presence checks.
- **Kept on purpose:** the `i18n` job still runs `sync-i18n.mjs --audit-scope=web-ui/src/pages/Dashboard`. It checks shared catalogues, not the web app, and it is cheap. The owner can remove it too if wanted.
- **Not changed:** the code and `dependabot.yml` entries for web-ui and mobile-pwa. Dependabot PRs for those folders will open but will not run these checks.
- **Branch protection:** the `main` branch has no protection rules, so no required checks had to be removed.
- **To restore:** re-add the jobs from git history (commit that removes them) when web UI resumes.

## Open items reconciled against the log (2026-11-09)

Recorded from the log entries above. Each item is marked by what the log actually shows. Anything marked "not recorded" needs the owner to confirm.

| Item | Status in the log | What is missing |
|---|---|---|
| P-01 desktop boundary | Approved conditionally (2026-10-29, 2026-11-04) | Boundary text not committed as its own file |
| Q-14 preference schema | Approved (2026-10-29) | Schema content (fields, defaults, fallback) not committed |
| C-09 fixture schema | Approved (2026-10-29). The 2026-10-31 provenance section still says "OPEN" | Which schema file is approved (candidate `shared/diagnostics/diagnostic-event.schema.json` v1.0) and its provenance commit |
| C-03 typed error mapping | Approved (2026-10-29) | Committed mapping file |
| U-03 font | Vazirmatn (OFL 1.1) confirmed (2026-10-29) | Per-OS render evidence |
| U-10 reduce-motion fallback | Confirmed (2026-10-31) | Windows toggle check on a device |
| File-send route | Decided (2026-11-07): Local or Internet, chosen by the user before each send | Whether the choice applies to every send or only the first; what "P2P" covers in Local |
| Token storage policy (desktop vs Admin) | Open in P-01 evidence summary (section 5) | Owner decision; the Track A relay work does not record one |
| Relay-only fallback when LAN discovery fails | Moot for file sends (see route decision) | Whether it still applies to other flows |
| Backend accounts O-1 to O-6 | O-1 to O-6 decided by the coordinator (2026-11-01); owner may override | Owner override or confirmation; the "Postponed" entry conflicts with the parallel-lane decision and should be confirmed |
| Persian review | Not recorded | Persian-speaker review of new `fa.json` strings |
| Device and live checks | Not recorded as done | Keyring on Windows/macOS/Linux, two-app separation, U-01, U-02, U-03, live relay checks |
| Gate 0b | Not passed. Blockers 1 to 14 open | Evidence per blocker and formal sign-off |

**Correction:** the 2026-11-08 "ready-to-build" note said C-09 was not confirmed. That was wrong. The owner approved it on 2026-10-29. What is still missing is the committed provenance.


## Audit reconciliation addendum (2026-11-10)

This addendum corrects current-status interpretation without rewriting the dated history above.

### File route selection cadence — owner clarification

The 2026-11-07 owner decision requires the user to choose **Local or Internet before each file is sent**. The owner explicitly confirmed this per-file scope in the 2026-11-10 documentation-audit instruction. The earlier open question asking whether the choice applies to every file or only the first is therefore **resolved**. Do not ask the owner to decide this again.

The separate question of what the Local route actually transports remains unresolved. The decision-log wording describes Local as BLE + LAN using relay/P2P; `docs/product/file-transfer.md` describes routine LAN as direct transfer and BLE/Wi-Fi Direct as a deferred emergency module. Preserve this conflict until the owner/technical authority supplies a reconciled Local transport contract. The per-file selection decision does not settle that transport question.

### Desktop/Admin session policy status

The owner-approved high-level distinction is already recorded in Contract Freeze v1 (2026-10-28): Desktop session persists across restarts; Admin is memory-only (Q-01). The detailed Desktop refresh-token-only keyring rules were recorded separately by the coordinator under the owner's standing authority in `desktop-token-storage-policy.md` (2026-11-04). The 2026-11-01 P-01 evidence summary's request to confirm the distinction is stale. The delegated attribution for the detailed rules remains unchanged.

### Backend O-1–O-6 status

O-1 through O-6 were decided by the coordinator under the owner's standing “decide and document” instruction on 2026-11-01. They are recorded delegated decisions, not six unanswered owner questions and not direct verbatim owner approvals. The older “not decided” labels in `backend-accounts-current-state.md` and `backend-accounts-handover.md` are historical snapshots and must not be used as current decision status. The owner retains the ability to override an item with a new decision entry.

### Current status versus historical evidence

- Track A is recorded as implemented and merged in PR #407; live relay-ticket checks remain outstanding.
- Track B has progressed through shared `desktop-session`, Tauri-free normal dependency proof and Slint lifecycle tests. Real OS keyring device tests and two-host coexistence remain unverified.
- The P-01 evidence summary dated 2026-11-01 is a historical snapshot. Its “Track A not started” and token-policy questions are superseded by later entries; retain its original findings as historical evidence.
- Gate 0b remains OPEN/BLOCKED. None of these corrections passes the gate or authorizes B-01 production implementation.
## Desktop Slint TEST BUILD workflow (issue #445, 2026-11-11)

- **Owner decision:** the test build targets the production Worker origin (`https://onyx-framework.soheil-mozaffari.workers.dev`), the same origin the current desktop-shell uses.
- **Consequence recorded for the owner:** once signed in, the test build can sync and read production data through the same endpoints as the Tauri app. Production writes still require explicit owner approval per the governance rules.
- **Changes:**
  - `crates/bins/desktop-slint/src/composition.rs`: `ONYX_API_ORIGIN` can be set at build time. Unset builds keep the production default.
  - `crates/bins/desktop-slint/ui/*.slint` and `main.rs`: the window title gets a `build-label` suffix from `ONYX_TEST_BUILD_LABEL`, so test builds show "TEST BUILD / NOT FOR PRODUCTION".
  - `scripts/build-desktop-slint-test-package.sh`: builds `onyx-desktop-slint-test` .deb with runtime dependencies from `dpkg-shlibdeps` plus the libraries winit, Slint and the GL stack load at runtime (`dlopen`), which `dpkg-shlibdeps` cannot detect.
  - `.github/workflows/desktop-slint-test-build.yml`: manual only, separate from CI and release. Builds in `debian:trixie`, uploads an Actions artifact with `SHA256SUMS`, `BUILD-INFO.txt` and `INSTALL.md`. A second job installs the package in a clean Debian 13 container, launches it under Xvfb, stops it with SIGTERM, and uninstalls it.
- **Not included:** Windows and macOS packages, production release, and any change to the Tauri default.
- **Evidence status:** the release build and package build succeed locally on Ubuntu 24.04 (not Debian 13). The Debian 13 install, launch, exit and uninstall checks run only in the workflow's `verify` job and are not yet recorded.

## Desktop Slint TEST BUILD: first passing run (issue #445, 2026-11-12)

- **Run:** `desktop-slint-test-build.yml`, run 43194818545, on `main`. Build and clean Debian 13 verify both passed.
- **Verified in CI (Debian 13 x86_64 container):** SHA-256 check, `apt install`, launch under Xvfb for 15 s, SIGTERM exit, `apt remove`, and confirmation that the package and binary are gone.
- **Artifact:** `onyx-desktop-slint-TEST-BUILD-debian13-amd64-<sha>` in that run. Contains the `.deb`, `SHA256SUMS`, `BUILD-INFO.txt` (commit, API origin, toolchain) and `INSTALL.md`.
- **API origin compiled in:** production Worker, as the owner chose. Signed-in use can read and sync production data.
- **Findings from the failing runs, fixed:**
  1. The Debian container's default shell is `dash`. Steps now run with bash.
  2. The container rejected the workspace's git ownership. The build now reads `GITHUB_SHA`.
  3. The app exits at startup without a Secret Service (D-Bus keyring). The app behavior is unchanged. The verify job now provides a D-Bus session and unlocked keyring, as CI does for tests. **Open question for the owner:** whether a machine without a keyring should refuse to start, as now, or run with a warning. This is a product decision. Current behavior is kept.
- **Not covered by this evidence:** interactive sign-in, OS-keyring save and restore on a real desktop, Persian IME, screen readers, Windows and macOS (not in scope for this milestone).
- **PRs:** #446 (workflow and code), #448, #449, #450 (CI fixes), all merged.
