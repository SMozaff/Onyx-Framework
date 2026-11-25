# Desktop Slint session architecture (S7, 2026-11-25)

- `desktop-slint` now uses `SessionAuthority::with_api_factory(c cloud, http_api_factory(), storage)` and `sign_in_at(server, Some(org), user, password)` (see `crates/bins/desktop-slint/src/composition.rs`).
- Server resolution: `GET {cloud}/api/organizations/lookup?identifier=`; 404 -> `LoginFailure::OrganizationNotFound`; null/empty `server_url` -> cloud base; only http(s) accepted (`server_from_lookup`).
- The issued server is stored in `StoredSession.server_address`; startup restore, outbox and relay endpoints use that stored server (never the build-time `ONYX_API_ORIGIN`). `relay_ws_endpoint` maps https->wss and http->ws.
- Forced password change: UI gated on `SessionSummary.must_change_password`; `change_password` posts `{server}/api/auth/password` with the bearer token, maps Worker codes to `PasswordFailure`, and signs out on success (Worker revokes all sessions).
- `StoredSession.must_change_password` keeps `#[serde(default)]` for older keyring records.
- Login fields order: Organization, Staff identifier, Password (Tauri parity). No token/password is ever a Slint property.
- Verification: GitHub Actions only, no local builds/tests (owner instruction).
