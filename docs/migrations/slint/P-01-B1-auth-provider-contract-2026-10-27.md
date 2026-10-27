# P-01 B1 Authentication and Relay Provider Contract — 2026-10-27

**Status:** Evidence-backed implementation proposal; not final architecture approval.  
**Baseline:** `913a36a5606c0241a15104cebae2c915997ea02b` (`main`).  
**Gate 0b:** OPEN. No production authentication behavior is changed by this document.

## Findings confirmed against source

1. `client-composition::AppStateConfig` requires an `AuthorityProvider`, a `RelaySocketFactory`, local discovery, relay endpoint, replica/organization identity, and blob root. The composition root does not fabricate missing providers.
2. `sync-transport::placeholder_types::StaticAuthorityProvider` is explicitly a test fake. It must not be wired into a shipping binary.
3. A real `TungsteniteRelaySocketFactory` already exists in `crates/bins/desktop-shell/src/relay_socket.rs`. It is production-shaped and implements the relay-ticket protocol, but it is private to the Tauri-bound shell crate; the Slint host cannot reuse it without extracting the implementation and its dependencies into a shell-independent adapter crate/module. Earlier broad wording that no production socket implementation exists anywhere is inaccurate; the narrower fact is that the reusable composition boundary is not yet established.
4. The socket adapter mints a ticket with authenticated HTTPS `POST /api/relay-ticket`, sending `target_id` and `self_replica`, then connects to `/api/relay/:target_id` using the short-lived ticket. It does not put the full bearer access token in the WebSocket URL.
5. The API auth implementation exposes `POST /api/auth/refresh`; refresh rotates both access and refresh tokens. A provider cannot safely treat an access token as indefinitely valid or rotate refresh tokens without updating the owning session atomically.
6. The existing Tauri `session.rs` persists a `StoredSession` containing access and refresh tokens through `SecureStorage`. This is existing Desktop behavior, not authorization to apply keyring persistence to the Admin client. Q-01's Admin token rule remains memory-only.
7. The current `RelaySocketFactory` takes a bearer token on each connection. The provider therefore needs to return a current access token and coordinate refresh/rotation with the one authoritative session owner.

## Proposed B1 boundary to validate

Extract a shell-independent runtime/session layer that owns:
- authenticated principal and organization/replica binding;
- access-token retrieval and refresh-token rotation;
- logout/revocation and invalid-session transitions;
- construction of the shared `AppState`;
- relay auth-provider and socket-factory injection;
- event subscription ownership and cancellation.

Keep Tauri commands and Slint view models as thin shell adapters. Do not make the Slint host depend on Tauri types. Do not share an Admin in-memory token store with the Desktop keyring policy unless a separately approved contract explicitly permits it.

The production provider should implement the existing `AuthorityProvider` trait by asking the session service for a valid access token. It should not own a second independent copy of tokens. If refresh is needed, the session service must perform the Worker/API's documented rotation and commit the new pair atomically before returning the new access token. A refresh failure transitions to unauthenticated/re-authentication-required; it must not fall back to a static token or stale credentials.

## Required acceptance tests before provider is production-ready

- Missing session fails closed; no placeholder/default credential is emitted.
- A valid access token is returned without appearing in logs, tracing fields, error strings, UI properties, or diagnostics.
- Expired access token causes at most one coordinated refresh for concurrent callers.
- Successful refresh atomically replaces both access and refresh tokens.
- Invalid/revoked refresh token clears or invalidates the session and requires interactive login.
- Network failure preserves no claim of successful refresh and does not blindly replay a state-changing command.
- Relay-ticket minting sends the bearer token only in the HTTPS Authorization header; full access token never appears in a WebSocket URL.
- Relay tickets are used only for the target/self-replica pair for which they were minted; no ticket or token is logged.
- Logout cancels relay/event subscriptions and prevents subsequent token retrieval.
- Restart behavior follows the client-specific approved persistence rule (Desktop and Admin must not be conflated).
- Integration tests exercise the real Worker/API contract and negative cases, not just an in-memory fake.

## Remaining blockers / decisions

- Confirm which deployed API/Worker origin and relay origin are canonical for the Slint Desktop host; do not infer this from test URLs.
- Confirm how local-first Desktop sync obtains/refreshes the Worker/API access token, and whether the current `/api/auth/refresh` rotation contract is supported by the exact deployment.
- Extract `relay_socket.rs` into a reusable crate/module with explicit dependencies and cross-platform TLS/WebSocket tests.
- Implement the shell-independent session service and test token rotation/cancellation before wiring it into `AppStateConfig`.
- Verify local discovery is production-capable for supported target platforms rather than accepting the no-op fallback.
- Review replica ownership and identity persistence behavior, including shared-install account changes.
- Obtain final P-01 boundary approval and the remaining Gate 0b sign-off. This document does not authorize a release cutover or close Gate 0b.

## Execution order

1. Re-pin the contract to the actual deployed API origin and confirm endpoint availability.
2. Define the typed shell-independent session interface and error/state transitions.
3. Extract the existing relay socket adapter without changing its wire protocol.
4. Implement the provider through the session interface, with atomic refresh rotation and fail-closed tests.
5. Wire the new host behind a feature/build target while leaving Tauri as the default.
6. Run Windows/Linux builds and relay/auth integration tests; attach evidence to the BOUND decision record.
7. Seek explicit architecture and Gate 0b sign-off before treating the design as frozen.
