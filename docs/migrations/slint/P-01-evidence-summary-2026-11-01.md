# P-01 evidence summary for owner review (2026-11-01)

**Purpose:** Gate 0b blocker 1 asks for complete B1 architecture evidence and an approved boundary. This summary gathers what the repo currently shows. It does not approve the boundary. The owner decides.
**Sources:** `P-01-B1-investigation-2026-10-27.md` and `P-01-B1-auth-provider-contract-2026-10-27.md` (baseline `913a36a`).

## 1. What B1 is

A separate native Slint desktop executable, sharing Rust application and domain crates in-process. Tauri stays the default until a later release gate approves the switch.

## 2. What the repo verifies (read-only evidence)

| Claim | Evidence | Status |
|---|---|---|
| Reusable Rust crates exist | `client-composition`, domain and application crates in the workspace | Verified (source read) |
| Slint host can avoid Tauri runtime types | Shared crates do not import Tauri; `desktop-shell` is the Tauri-specific part | Partly verified: the dependency tree of a Slint host has not been built and checked |
| Login path | `desktop-shell/src/session.rs` calls `POST /api/auth/login` | Verified |
| Relay ticket path | `relay_socket.rs` exchanges the access token for a short-lived ticket via `POST /api/relay-ticket` | Verified in source; not run against a live Worker |

## 3. Risks found (must be resolved before implementation)

1. **Production relay auth uses a test-only provider.** `desktop-shell` builds the logged-in `AppState` with `StaticAuthorityProvider`, which its own source calls "a trivial provider for tests: always returns a fixed token." Logged-out state passes an empty string. An empty token must never count as a credential.
2. **No refresh, expiry or revocation in that provider.** `StoredSession` holds an access and a refresh token, but the provider does not refresh or invalidate them.
3. **Event subscription lifecycle gap.** Documented in the investigation; not yet fixed.
4. **Discovery fallback.** Desktop falls back to relay-only mode if LAN discovery fails. This is degraded-mode behavior and has no explicit test.
5. **Dependency check not done.** No `desktop-slint` package has been built with a Tauri-free dependency tree.

## 4. What is not done

- Track A (fix the relay auth provider, add refresh and logout tests, integration test of the ticket path): **not started.**
- Track B (shell-independent runtime boundary, separate `desktop-slint` crate, dependency proof): **not started.**
- The investigation's own status is "in progress."

## 5. Decisions the owner must make

1. Approve B1 as the boundary, or choose B2 (separate process).
2. Approve Track A as the first implementation step (fix the test-only provider before anything else).
3. Confirm the desktop token storage policy. Desktop currently keeps a keyring-backed session. Admin uses memory-only tokens (Q-01). Confirm these stay different.
4. Confirm relay-only fallback when LAN discovery fails is acceptable.

## 6. Recommendation

Approve B1 conditionally: Track A first, with tests, then Track B. Do not start Track B until Track A passes. This keeps the boundary decision separate from the security fix.

## 7. Gate 0b blocker 1 status

**Open.** This summary is evidence, not sign-off. The owner's approval, the boundary text, and Track A evidence are still required.
