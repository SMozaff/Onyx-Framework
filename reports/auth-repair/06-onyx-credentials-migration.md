# All-Father ONYX Credentials Migration — Verification Report

**Scope:** Replace the All-Father Google/Supabase exchange with ONYX username/password authentication. No production account, secret, database, or service was changed during this work.

## Canonical contract

- Canonical Worker principal: `id='__allfather__'`, username `allfather`, organization `__allfather__`, explicit role `ALL_FATHER`, `is_admin=1`, active account and active system organization.
- Supported client login endpoint: `POST /api/auth/login` on the Cloudflare Worker.
- Password hashing: the Worker’s existing Argon2id implementation (`parallelism=1`, `iterations=3`, `memorySize=65536`, `hashLength=32`).
- Session: the existing ONYX access/refresh token pair; protected requests re-read authoritative account, organization, role, tenant, and revocation state from D1.
- The Admin Shell's `getServerAddress()` resolves to the Cloudflare Worker. The login UI submits username/password through `loginWithOnyxCredentials`, stores the ONYX session, and sends the access token on protected requests. It does not invoke Axum.

## Implemented changes on this branch

- Removed the Admin Shell All-Father OAuth helper, callback-listener code/tests, Tauri browser OAuth command, and callback event wiring.
- Removed the Axum `POST /api/auth/supabase` route, its Supabase identity module/state/startup provisioning, and dedicated Render Supabase/All-Father provider configuration. The Axum ordinary ONYX login rejects the reserved `allfather` username rather than creating an alternate master-account path.
- Removed the Worker’s unused Supabase identity implementation and obsolete Worker deployment/dev variables. The Worker has no `POST /api/auth/supabase` route; the smoke test explicitly verifies the retired route does not issue a session.
- Updated the Admin Shell login UI and contract tests to use ONYX credentials.
- Added `provision-allfather-password.mjs`: validates the already-existing canonical account and active system organization, hashes the supplied password with the same Argon2id parameters, writes only the hash, records a non-sensitive audit event, and verifies the stored hash. Test mode is local-D1 only. Production mode is never used by CI and requires explicit command-line confirmations.
- Added an isolated local Worker/D1 smoke workflow that validates the canonical session claims and expiry, exercises a protected All-Father read operation, rejects invalid/missing credentials and malformed sessions, checks the retired endpoint, verifies ordinary-user metadata cannot elevate role or tenant, and cleans up the disposable ordinary user and organization.
- Preserved the historical D1 provider-mapping migration/column for upgrade compatibility; it is no longer required by the Worker auth schema contract or used as login authority. Unrelated Supabase use elsewhere in the repository was not globally removed.

## CI evidence observed before the latest fixes

- Base/main: `54221e10f1dff46730aef33826db0c631c3d293d`.
- PR #267 merged at that SHA; its changes alone did not establish end-to-end success.
- On the first branch candidate, Worker CI passed: [run #39152267112](https://github.com/SMozaff/Onyx-Framework/actions/runs/39152267112).
- The full CI failure identified two concrete source defects: stale references to the removed `super::supabase::ALLFATHER_USERNAME` constant in `routes/admin.rs`, and a rustfmt-only extra blank line in `routes/auth.rs`. Those are fixed in the current change.
- The smoke workflow correctly failed before running tests because the configured Actions username secret does not match the repository’s reserved canonical username `allfather`. The secret value is intentionally not logged or reproduced. The cleanup step also ran before checkout when that validation failed; checkout is moved ahead of validation.
- The earlier main-branch live smoke [run #39135600185](https://github.com/SMozaff/Onyx-Framework/actions/runs/39135600185) attempted the production Worker and received HTTP 401. That does not distinguish a missing/unusable production password hash from a secret/account mismatch. This branch does not repeat that production request.
- Root cause of the Rust container build failure: the All-Father OAuth removal deleted `url.workspace = true` from `crates/bins/admin-shell/Cargo.toml`, but the `admin-shell` dependency array in `Cargo.lock` still listed `url`. That stale lock entry made `cargo build --locked` fail. The current fix removes only that obsolete `url` entry; the `url` package itself remains in the lockfile for other crates that still use it. Re-run full CI and Security on the exact final SHA to verify.

## Provisioning and production boundary

GitHub Actions secrets are not automatically written to D1, Render, or a deployed Worker. The smoke workflow provisions only its isolated local D1 database. No production provisioning or deployment was performed. To complete production verification, the owner must first confirm/update the `ONYX_ALLFATHER_USERNAME` Actions secret to the reserved value `allfather` (without sharing the password), then explicitly authorize the production password-provisioning command for the canonical D1 principal, and run a separately approved production verification. Do not claim production login is verified until a protected request succeeds using the resulting session.

## Status

**Implementation is in progress; do not merge or deploy until current-candidate CI passes and the canonical username secret is corrected.** The exact final SHA and final workflow results are to be recorded after the current candidate is committed and its workflows complete.
