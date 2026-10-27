# P-01 B1 Feasibility Investigation — Separate Native Slint Desktop Host

**Status:** Investigation in progress; B1 is not final architecture approval.  
**Repository baseline:** `913a36a5606c0241a15104cebae2c915997ea02b` (main, 2026-10-27).  
**Decision authority:** User authorized investigation of B1 under BOUND.  
**Gate 0b:** OPEN.

## Executive finding

B1 is **plausible but not yet implementation-ready**. The workspace has reusable Rust application/domain crates and a composition root in `client-composition`; a separate Slint executable can depend on those crates without embedding Tauri. However, authentication/session orchestration and composition construction currently live inside `desktop-shell` and depend on Tauri-managed state, app paths, and shell-owned secure-storage adapters. B1 requires a shell-independent runtime/session boundary and tested composition root; the Tauri shell itself is not a safe reusable backend boundary.

A source-level correction matters: the workspace does have a concrete WebSocket implementation (`TungsteniteRelaySocketFactory`) and the desktop shell wires the authenticated session's access token into the relay provider. But it does so through `StaticAuthorityProvider`, which its own source explicitly describes as test-only. The problem is therefore **not that the desktop currently has no token value**; it is that the production composition relies on a test-labelled fixed-token provider, while token refresh/expiry and provider lifecycle are not represented in that provider contract. This must be corrected and tested before treating relay authentication as production-ready. Do not replace it with a fixed test token or silently broaden the Admin Q-01 persistence policy.

## Evidence inspected

- Root `Cargo.toml` includes `crates/applications/client-composition`, domain/application crates, persistence adapters, `crates/bins/desktop-shell`, `crates/bins/admin-shell`, and `crates/bins/admin-slint` as workspace members.
- `crates/applications/client-composition/Cargo.toml` depends on core domain/application crates, SQLite persistence, local blob storage, API server, Tokio, Reqwest, and transport crates.
- `client-composition::app_state::AppState` exposes command/query registries, event bus, sync agent, file upload coordinator, and SQLite pool. Its constructor requires organization/replica identity, relay endpoint, relay auth provider, relay socket factory, discovery, blob root, and optional owner authority.
- `crates/bins/desktop-shell/src/session.rs` authenticates through `POST /api/auth/login`, creates a `StoredSession` containing access/refresh tokens and identity, and persists that session via the `SecureStorage` port.
- `crates/bins/desktop-shell/src/lib.rs` rebuilds `AppState` on login/logout. In the logged-in path it passes `s.access_token.clone()` to `StaticAuthorityProvider`; the logged-out placeholder passes an empty string to the same provider.
- `crates/transports/sync-transport/src/placeholder_types.rs` defines `StaticAuthorityProvider` as “A trivial provider for tests: always returns a fixed token.” That type is nevertheless used by the desktop shell's logged-in composition path. This mismatch is a concrete implementation risk.
- `crates/bins/desktop-shell/src/relay_socket.rs` provides `TungsteniteRelaySocketFactory`. It calls `POST /api/relay-ticket` with the access token, then uses the returned short-lived ticket for the WebSocket handshake. This is preferable to putting the full access token in the WebSocket URL; the ticket route and deployment must still be verified in integration tests.
- `client-composition/src/app_state.rs` says `local_discovery: None` falls back to a no-op stub, while desktop-shell currently attempts to start `lan_discovery::LanDiscovery` and falls back to relay-only mode if it fails. Classify this as degraded-mode behavior and test it explicitly.
- `crates/bins/desktop-shell/Cargo.toml` is a Tauri application crate with a library target; its library uses Tauri types and manages `AppState` through Tauri state.
- `crates/bins/admin-slint/Cargo.toml` currently depends only on Slint and slint-build; its source is a Phase 0 shell with language/theme UI controls. It is not a desktop host or backend integration yet.
- At the inspected main baseline, the workspace contained `admin-slint` but no `desktop-slint` package. This branch now adds a minimal, separate `desktop-slint` feasibility target with only Slint dependencies; it has no backend/session wiring and is not a production implementation. CI for the workspace-registration commit is pending, so the compile and cross-platform dependency claims are not yet verified.
- The Slint spike workflow does not establish full desktop authentication, session security, runtime lifecycle, accessibility, Persian IME, or production packaging readiness.

## Immediate solution path

### Track A — production relay authentication and session lifecycle

1. Replace production use of `StaticAuthorityProvider` with a named application-layer provider backed by the current session authority, not a frozen token string. The provider must fail closed when logged out and return a typed `AuthorityError::NoCredentials`; it must never return an empty token as a valid credential.
2. Define whether the provider reads an in-memory current access token on every call and how refresh is triggered. The `StoredSession` currently contains an access token and refresh token, but the inspected provider path does not itself refresh or invalidate credentials. Do not invent refresh endpoint semantics: verify the current server route, request/response contract, expiry behavior, and revocation behavior first.
3. Keep access tokens out of Slint properties, event payloads, logs, crash reports, and diagnostics. For Admin, preserve approved Q-01 memory-only token handling. Desktop's existing keyring-backed persisted session is a separate current contract and must not be copied into Admin without an explicit decision.
4. Add tests for logged-out denial, valid token retrieval, token replacement after refresh, expiry/refresh failure, logout invalidation, concurrent reads during session replacement, and redaction/no-token-leak behavior.
5. Integration-test the full path: authenticated access token -> `/api/relay-ticket` -> short-lived ticket -> `/api/relay/{target_id}` WebSocket handshake, including 401/403/501, expired/replayed/wrong-target tickets, network timeout, and logged-out startup. Never treat a compile-only check as proof of online relay operation.

### Track B — shell-independent B1 host boundary

1. Create a distinct `desktop-slint` binary crate only after Cargo/package constraints are demonstrated; do not repurpose `admin-slint` silently.
2. Extract or create a shell-independent runtime/session service with typed operations for login, current session, logout, sync status, command/query dispatch, event subscription/unsubscription, and shutdown. The host should inject paths, credential policy, relay providers, and event lifecycle rather than importing Tauri runtime types.
3. Share `client-composition` and approved domain/application crates directly. Preserve one owner for session identity, token lifecycle, and `AppState` replacement; define what happens to event subscriptions and sync tasks during login/logout/restart.
4. Prove the dependency tree excludes Tauri runtime/UI dependencies from `desktop-slint`; test startup and shutdown, SQLite/blob paths, replica identity, migrations, event cancellation, and simultaneous Tauri/Slint installation/data-directory coexistence.
5. Keep the current Tauri/React executable as the default until later release gates explicitly approve cutover.

## B1 benefits

1. Keeps the UI host as a separate native executable while sharing typed Rust domain/application code in-process.
2. Avoids a new cross-process protocol, local IPC authentication, version negotiation, and reconnection semantics required by B2.
3. Lets Slint view models call typed Rust service interfaces instead of serializing every operation through a process boundary.
4. Allows Tauri to remain the default until later release gates approve replacement.

## Remaining constraints and risks

- **Session authority:** access-token refresh, expiry, revocation, logout, and in-flight relay behavior need a defined owner and tests.
- **Storage and identity:** specify per-OS data directories, SQLite migration behavior, blob-store paths, replica identity, startup lock handling, and multi-process contention behavior.
- **Event lifecycle:** define subscription ownership, unsubscribe/cancellation, event filtering, login/logout transitions, and behavior when the host restarts. Existing Tauri event forwarding has a documented subscription lifecycle gap.
- **Packaging:** define executable identity, installer/shortcut registration, application data migration, crash reporting, upgrade/rollback, and coexistence with the current Tauri package.
- **Scope separation:** the Admin Slint scaffold is not proof of the Desktop migration.

## Required proof before final P-01 approval

1. A minimal `desktop-slint` Cargo target builds on Windows and Linux without Tauri runtime/UI dependencies.
2. Dependency tree and Cargo feature graph are archived and reviewed.
3. Typed runtime tests cover login, current session, logout, expiry/refresh, process restart, errors, cancellation, and token non-disclosure.
4. Composition tests prove SQLite/blob paths, real relay auth/socket behavior, relay ticket exchange, and explicit classification of every stub/test-only provider.
5. Event tests prove subscribe/unsubscribe, login/logout rebind, cancellation, and process shutdown.
6. CI builds and smoke-runs the executable on Windows and Linux; packaging/upgrade tests demonstrate coexistence and rollback.
7. Threat model, accessibility, Persian IME/RTL, font, reduced-motion, and contrast gates remain separately evidenced.

## Policy transition reconciliation found during the same approved U-04 work

The current Rust policy aggregate rejects `EvaluatePolicy` and `RegisterViolation` when the policy is `Retired`; Worker command semantics restrict both to `Active`, and Worker tests assert rejection. Worker test comments describing a Rust mismatch are stale. Do not change lifecycle behavior based on those comments. The parity suite still needs published-version requirements, unknown rule keys, draft uniqueness, concurrency/version checks, and typed error mapping.

## Decision

**B1 remains the authorized investigation path. Final architecture approval is pending.** The current evidence does not support claiming the separate host is ready for production implementation, and it does not satisfy Gate 0b sign-off.

## Evidence update (2026-10-27)

The source review distinguishes two separate facts that must not be conflated:

- A desktop login does supply a real access-token string and a real `TungsteniteRelaySocketFactory`; the current shell passes that token through `StaticAuthorityProvider`.
- `StaticAuthorityProvider` is explicitly documented as test-only and holds a fixed token, so the production composition lacks a proper session-aware provider contract. The code also needs verified refresh/expiry/revocation semantics and end-to-end relay-ticket integration evidence.

The next implementation slice should therefore add a session-aware, fail-closed provider at the shell-independent boundary and test it before wiring it into the new Slint host. Do not use a fixed test token as a production fallback, and do not claim online sync from a build-only smoke test.
