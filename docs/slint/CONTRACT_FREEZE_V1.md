# ONYX Slint: Contract Freeze v1 (DRAFT)

Status: DRAFT. Not frozen. Gate 0b is NOT passed. This document records what the merged code does today, plus the gaps that block freezing. Freezing requires owner approval (see section 6).

Source of truth: the merged code at `8a482b0` and later, plus the owner decisions in the planning package. Every rule below cites the code it was read from.

## 1. C-07 Session lifecycle (Slint host)

Source: `crates/bins/desktop-slint/src/session_service.rs`.

Observable states:
- **Unauthenticated**: no token pair held. `current_access_token()` returns none.
- **Authenticated**: a token pair is held, with an access expiry.
- **LoggingOut**: `logging_out` is true. The local authority is disabled, and callers get no token while logout drains an in-flight refresh.

Rules as implemented:
- Login (`login(username, password)`) sends `client_type: "desktop"` to `POST /api/auth/login`. It clears the old pair and bumps `session_generation` first. A login response that arrives after logout was requested is never reported as success.
- Access token lifetime comes from the server's `expires_in` (3600 s in the Worker). A 30 s refresh skew is applied (`REFRESH_SKEW`).
- Refresh is single-flight (`refresh_gate`). The rotation runs in a detached task, so cancelling one caller does not discard a successful rotation.
- Logout disables local authority first, waits for any in-flight rotation, then calls `POST /api/auth/logout` with the latest accepted pair (bearer and refresh token).
- Tokens are never persisted by the Slint host and never formatted into diagnostics. `SessionIdentity` contains only non-secret fields.
- Identity returned to the UI: `user_id`, `organization_id`, `username`, `is_admin`, `role`. `role` is required by the parser.

Login response contract (Worker `authLogin`, `deploy/cloudflare/edge-gateway/src/index.ts`):
- Body: `access_token`, `refresh_token`, `expires_in`, `user`.
- `user`: `id`, `username`, `email` (nullable), `organization_id`, `is_admin`, `class` (nullable), `role`.
- Slint `LoginUser` matches this as of PR (branch `feat/session-login-type`). `email` and `class` are parsed but unused.

Refresh and logout contract (Worker `authRefresh`, `authLogout`):
- Refresh consumes the refresh token once. Replays and revoked sessions return `401 INVALID_REFRESH_TOKEN`.
- Logout accepts a bearer token, a refresh token, or both from the same session family.

### Gaps that block freeze of C-07
- **G-07-1 (decided by owner, 2026-10-28):** behaviour depends on the app.
  - **ONYX Admin (administration):** fail closed. Any refresh failure (401, network, or server error) clears the session and requires a fresh login. Admin tokens are memory-only (Q-01), so a restart also requires login.
  - **ONYX Desktop (staff operations):** keep the session until the user logs out intentionally. Refresh failures keep the current session and retry. Desktop must persist the session across restarts. The server still rejects revoked access tokens, so the app must show a clear message when a request is refused.
  - The Slint Desktop host does not yet implement this. Its current behaviour keeps the old pair on any failed refresh, and it has no persistent restore. Both are implementation work.
- **G-07-2:** Desktop's persistent restore (OS keyring) is not wired into the Slint host. Required by the decision above.
- **G-07-3:** Startup rule (what a fresh launch does with no restored session) is not defined in the Slint host.

## 2. C-08 Secret storage

Sources: `crates/bins/desktop-shell/src/secure_storage/keyring_adapter.rs`, `crates/bins/desktop-shell/src/session.rs`, `crates/bins/desktop-slint/src/main.rs`.

- **Desktop (existing shell):** OS keyring, service `com.onyx.platform`. One storage key `auth.session` holds one JSON blob for the whole session. Save and clear are atomic at the key level.
- **Slint host:** in-memory only. No keyring writes in this slice.
- **Admin (owner decision Q-01):** tokens in memory only, not in the keyring.
- **Never stored:** passwords, in any store or log. Enforced by the planned V-11 scan. Not yet implemented.
- **Keyring failure:** must never block sign-in; show a non-blocking notice. Not yet implemented in the Slint host.

### Gaps that block freeze of C-08
- **G-08-1:** the desktop keyring tests need a secret service. They fail in a sandbox without one (`No default store has been set`). CI on Ubuntu has one.
- **G-08-2:** the Admin keyring-failure notice path is not implemented.

## 3. C-02 Core-to-UI commands and view models (partial)

Desktop command surface today (`crates/bins/desktop-shell/src/lib.rs`):
- `execute_command(envelope: CommandEnvelope<serde_json::Value>) -> Result<serde_json::Value, ShellError>`
- `subscribe_events(organization_id) -> Result<String, ShellError>`
- `get_sync_status`, `upload_file`, `download_file`
- `get_current_session(storage, local_replica) -> Result<Option<SessionInfo>, ShellError>`

Slint host view models:
- `SessionIdentity { user_id, organization_id, username, is_admin, role }`
- `SessionSourceError` (opaque; no detail to the UI)

### Gaps that block freeze of C-02
- **G-02-1:** the Slint-side adapter for each Desktop command (envelope shape per screen) is not written.
- **G-02-2 (U-11):** `subscribe_events` has no matching unsubscribe command. The Slint host needs a teardown rule.
- **G-02-3:** the Admin view-model list (users, organizations, policy, profiles) is not defined. It depends on the Worker endpoints in `docs/production/api-contract.md`.

## 4. Other contracts (status only)

| Contract | Status |
|---|---|
| C-01 Worker API | Source in `docs/production/api-contract.md`; not pinned to a commit in this document. |
| C-03 Error taxonomy | Not frozen. `SessionSourceError` is opaque. The `LOGIN_*` mapping is not yet implemented in the Slint host. |
| C-04 Theme tokens | Not frozen. Dark token set is open (U-12). |
| C-05 Component API | Not frozen. U-01 and U-02 spikes are pending. |
| C-06 Localization | Not frozen. Q-14 persistence not implemented. Q-16 decided (Latin digits). |
| C-09 Fixtures | Not frozen. |
| C-10 Platform services | Not frozen. Notifications: Q-09 decided (approvals and messages). |
| C-11 Packaging | Not frozen. U-06 decided (unsigned for now). U-07 decided (MSI and AppImage). Q-15 decided (channel `slint`). |

## 5. Blockers still open for Gate 0b

- U-01: table sorting, paging, multiline input, Persian IME, bidi. Manual checks not run.
- U-02: Windows UI Automation and Linux AT-SPI. Not run.
- U-03: font shaping and licence. Coverage only.
- U-10: reduced-motion API per OS. Not verified.
- Q-14: persistence implementation.
- Unmaintained-crate decision for ttf-parser, zbus_names, zvariant, proc-macro-error (upgrade or documented acceptance).

## 6. Freeze procedure

1. G-07-1 is decided (see section 1). Owner confirms the Admin and Desktop rules are final.
2. Each gap in sections 1 to 3 is closed or explicitly deferred with an owner and date.
3. Blockers in section 5 have evidence and an approved disposition.
4. Owner approves freezing C-02, C-07, and C-08 at version 1. C-01 is frozen by reference to `docs/production/api-contract.md` at a named commit.

Until step 4, this document remains a draft and Gate 0b remains NOT PASSED.
