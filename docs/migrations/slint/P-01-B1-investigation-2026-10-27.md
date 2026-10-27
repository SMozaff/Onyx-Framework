# P-01 B1 Feasibility Investigation — Separate Native Slint Desktop Host

**Status:** Investigation in progress; B1 is not final architecture approval.  
**Repository baseline:** `913a36a5606c0241a15104cebae2c915997ea02b` (main, 2026-10-27).  
**Decision authority:** User authorized investigation of B1 under BOUND.  
**Gate 0b:** OPEN.

## Executive finding

B1 is **plausible but not yet implementation-ready**. The workspace has reusable Rust application/domain crates and a composition root in `client-composition`; a separate Slint executable can depend on those crates without embedding Tauri. However, the existing desktop authentication/session orchestration and composition construction are implemented inside `desktop-shell` and depend on Tauri-managed state, app paths, and shell-owned secure-storage adapters. B1 therefore requires an explicit shell-independent runtime/session boundary and a tested composition root. It is not safe to treat the existing Tauri shell crate as a reusable backend library.

## Evidence inspected

- Root `Cargo.toml` includes `crates/applications/client-composition`, domain/application crates, persistence adapters, `crates/bins/desktop-shell`, `crates/bins/admin-shell`, and `crates/bins/admin-slint` as workspace members.
- `crates/applications/client-composition/Cargo.toml` depends on the core domain/application crates, SQLite persistence, local blob storage, API server, Tokio, Reqwest, and transport crates.
- `client-composition::app_state::AppState` exposes command/query registries, event bus, sync agent, file upload coordinator, and SQLite pool. Its constructor requires an `AppStateConfig` containing organization/replica identity, relay endpoint, relay auth provider, relay socket factory, discovery, blob root, and optional owner authority.
- `crates/bins/desktop-shell/Cargo.toml` is a Tauri application crate with a library target, but its library uses Tauri types and manages `AppState` through Tauri state.
- `crates/bins/desktop-shell/src/lib.rs` owns login/logout commands, calls `session::authenticate/save/clear`, rebuilds `AppState`, and uses Tauri's app-data path.
- `crates/bins/desktop-shell/src/session.rs` owns session authentication and storage functions. `desktop-shell` uses a keyring-backed `SecureStorage` adapter.
- `crates/bins/admin-slint/Cargo.toml` currently depends only on Slint and slint-build; its source is a Phase 0 shell with language/theme UI controls. It is not a desktop host or backend integration yet.
- The current workspace tree contains `admin-slint` but no `desktop-slint` package.
- The repository has a Slint spike workflow and an Admin Slint workflow, but the spike does not establish full desktop authentication, session security, runtime lifecycle, accessibility, Persian IME, or production packaging readiness.

## B1 benefits

1. Keeps the UI host as a separate native executable while sharing typed Rust domain/application code in-process.
2. Avoids creating a new cross-process protocol, local IPC authentication, version negotiation, and reconnection semantics required by B2.
3. Allows the Slint layer to call typed Rust service interfaces instead of serializing every operation through a process boundary.
4. Can preserve the existing Tauri application as the default until explicit later release gates approve replacement.

## B1 constraints and risks

1. **Shell coupling:** login/session and composition setup currently live in `desktop-shell` and reference Tauri state/path APIs. The new host must not import Tauri UI/runtime behavior merely to reuse backend code.
2. **Session authority:** define a single owner for authenticated identity, token lifecycle, logout, expiry, and refresh. For the separate Admin client, the approved Q-01 in-memory-only token rule must be enforced end-to-end; do not reuse desktop keyring persistence for Admin tokens.
3. **Composition completeness:** `AppStateConfig` needs concrete relay authentication/socket/discovery providers and local paths. Existing comments identify missing production relay implementations in the workspace; the B1 spike must inventory which are real, stubbed, or test-only before asserting production readiness.
4. **Storage and identity:** specify per-OS data directories, SQLite migration behavior, blob-store paths, replica identity, startup lock handling, and multi-process contention behavior.
5. **Event lifecycle:** define subscription ownership, unsubscribe/cancellation, event filtering, login/logout transitions, and behavior when the host restarts. The existing Tauri event forwarding has a documented subscription lifecycle gap.
6. **Packaging:** define executable identity, installer/shortcut registration, application data migration, crash reporting, upgrade/rollback, and coexistence with the current Tauri package.
7. **Scope separation:** the Admin Slint scaffold is not proof of the Desktop migration. Do not silently repurpose `admin-slint` as the desktop host.

## Recommended B1 design boundary to validate

- Create a distinct `desktop-slint` binary crate only after the spike's Cargo/package constraints are demonstrated.
- Keep Slint view models and UI callbacks in the host crate.
- Extract a shell-independent runtime/session service from the Tauri-bound `desktop-shell/src/lib.rs` and `session.rs`, or provide an equivalent typed adapter with explicit ownership. Avoid making the Slint host depend on `desktop-shell` solely to access private or Tauri-bound functions.
- Reuse `client-composition` and approved domain/application crates directly.
- Inject filesystem paths, secure-storage policy, relay/auth providers, and event lifecycle through explicit interfaces.
- Keep the current Tauri/React executable as the default until later release gates approve a cutover.
- Keep Admin's Q-01 token storage policy separate from Desktop's current keyring-backed session behavior unless an explicit contract amendment unifies them.

## Required proof before final P-01 approval

1. A minimal `desktop-slint` Cargo target builds on Windows and Linux while depending on the intended backend crates and without pulling in Tauri runtime/UI dependencies.
2. The dependency tree and Cargo feature graph are archived and reviewed; no accidental duplicate or conflicting runtimes.
3. A typed runtime interface test covers login, current session, logout, expiry, process restart, errors, and cancellation without exposing tokens to UI/logs.
4. Composition startup tests prove the configured SQLite/blob paths and relay provider behavior; all test-only/stub providers are explicitly classified.
5. Event tests prove subscribe/unsubscribe, login/logout rebind, cancellation, and process shutdown.
6. CI builds and smoke-runs the new executable on Windows and Linux; packaging/upgrade tests demonstrate coexistence and rollback.
7. Threat model, accessibility, Persian IME/RTL, font, reduced-motion, and contrast gates remain separately evidenced.

## Policy transition reconciliation found during the same approved U-04 work

The current Rust policy aggregate rejects `EvaluatePolicy` and `RegisterViolation` when the policy is `Retired`; the Worker command-semantics table restricts both to `Active`, and the Worker tests assert rejection. Therefore the Worker test comments saying the Rust API permits these commands are stale. Do not change lifecycle behavior on the basis of those comments. The parity suite still needs to cover published-version requirements, unknown rule keys, draft uniqueness, concurrency/version checks, and typed error mapping.

## Decision

**B1 remains the authorized investigation path. Final architecture approval is pending.** The current evidence does not support claiming the separate host is ready for production implementation, and it does not satisfy Gate 0b sign-off.
