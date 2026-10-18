# ONYX Cloudflare Worker

The Cloudflare Worker is the supported remote API for Admin, Staff, web, and mobile clients. It uses Cloudflare D1 as the authoritative store for ONYX principals, password hashes, organizations, roles, sessions, and authorization decisions. The Worker does not proxy requests to the legacy Axum deployment.

## Authentication contract

- **All-Father:** canonical principal `id='__allfather__'`, username `allfather`, organization `__allfather__`, explicit `role='ALL_FATHER'`, `is_admin=1`, and active account + active system organization.
- **Organization Admin and Staff:** ONYX username/email plus password.
- **Password hashing:** PHC Argon2id using the Worker’s established parameters (`parallelism=1`, `iterations=3`, `memorySize=65536`, `hashLength=32`). Never store plaintext credentials.
- **Login:** `POST /api/auth/login` accepts ONYX username/password and returns the existing ONYX access/refresh token pair.
- **Refresh/logout:** `POST /api/auth/refresh` and `POST /api/auth/logout`.
- **Authorization:** protected requests re-read the active user and active organization, explicit role, admin state, tenant binding, and revocation state from D1. Client-supplied role, admin, principal, or organization fields do not grant authority.
- **All-Father capabilities:** the `/api/allfather/*` routes enforce `ALL_FATHER` server-side. Ordinary accounts cannot claim the canonical ID or elevate themselves by changing request metadata.

The Admin Shell’s `getServerAddress()` points to the deployed Worker and its login form submits `{ username, password, client_type: "admin" }` to `/api/auth/login`. It consumes the returned ONYX token pair and sends the access token on protected API calls. There is no Google/Supabase fallback in the supported login flow.

## All-Father password provisioning

GitHub Actions secrets `ONYX_ALLFATHER_USERNAME` and `ONYX_ALLFATHER_PASSWORD` are supplied to the smoke workflow; they do not automatically provision the Worker or D1 account. The canonical principal is seeded by migrations with a deliberately unusable placeholder hash until an owner-controlled provisioning operation sets the password.

The repeatable command `npm run provision:allfather-password`:

1. Requires the existing unique canonical principal and active system organization.
2. Verifies explicit `ALL_FATHER` role, `is_admin=1`, active account, reserved username, and canonical organization.
3. Hashes the password using the same Argon2id parameters as the Worker.
4. Updates only the existing principal’s password hash and timestamp, writes a non-sensitive audit event, then verifies the stored hash.
5. Never creates, activates, or promotes an account and never prints credentials or password hashes.

For CI/local testing, use only:

```sh
npm ci
npm run d1:local
ONYX_ALLFATHER_USERNAME=allfather ONYX_ALLFATHER_PASSWORD="$ONYX_ALLFATHER_PASSWORD" npm run provision:allfather-password -- --environment test
```

Test mode is hard-restricted to local D1 using `wrangler.dev.toml`; it refuses remote mode or any other config. The smoke workflow starts a loopback-only Worker, tests successful and failed login, validates token claims and a protected All-Father read operation, verifies that forged client role/admin fields do not elevate a Staff account, tests the retired exchange route, and removes the disposable Staff fixture.

Production password provisioning is not run by CI or the deployment workflow. Only after explicit owner authorization, an operator with Wrangler credentials can run the following from this directory while supplying the password through the environment (never on the command line):

```sh
npm run provision:allfather-password -- --environment production --approve-production __allfather__ --expect-database onyx-free-db
```

Production mode resolves the named remote D1 database and requires both explicit approval flags. It updates only the already-canonical account; it will not create or reactivate a principal. This command was not run as part of the code change. Production login remains unverified until that authorized operation and a secret-safe live smoke succeed.

## Supabase migration history

The old All-Father Supabase exchange implementation, Rust/Axum route, Admin Shell OAuth helper and Tauri callback command, and Supabase-specific runtime/build configuration have been removed. The historical D1 migration `0003_supabase_identity.sql` and the `users.supabase_user_id` column remain only for migration-ledger compatibility; current authentication and provisioning do not read or write that column. Historical migrations are not rewritten or deleted. This change does not authorize removal of any separately operated Supabase service outside this repository.

## Local development

```sh
npm ci
cp .dev.vars.example .dev.vars
# Set a strong random ONYX_JWT_SECRET in the untracked .dev.vars file.
npm run d1:local
npm run dev
```

`wrangler.dev.toml` has a fixed local-only D1 identifier and must never be used for remote commands. Do not commit `.dev.vars`.

## Checks and deployment

- `npm test` — Worker auth, authorization, relay, schema, and migration tests.
- `npm run check` — Worker bundle dry-run.
- `npm run smoke:allfather` — smoke against the local Worker; requires the local D1 migrations, local password provisioning, and a loopback Worker to be running.
- `.github/workflows/allfather-auth-smoke.yml` runs the smoke in isolated local D1 and never sends credential tests to production.
- `.github/workflows/deploy-cloudflare-edge.yml` configures the Cloudflare Worker and D1 using Cloudflare/JWT/CORS and optional Hugging Face credentials. Supabase authentication secrets are no longer required by this workflow. The workflow is deployment-capable; do not dispatch it or merge to `main` without the required deployment authorization.

Keep `ONYX_JWT_SECRET`, Cloudflare credentials, and optional Hugging Face S3 credentials out of source control and logs. Public CORS origins must be exact trusted origins; wildcard credential CORS is forbidden.
