# ONYX Authentication and Authorization Model

**Status: authoritative.** All-Father uses ONYX-managed username/password credentials. Google/Supabase identity exchange is not an accepted All-Father authentication path.

## Canonical All-Father authority

- Canonical D1 principal: `id='__allfather__'`, username `allfather`, organization `__allfather__`.
- The principal must be active, its organization must be active, `role='ALL_FATHER'`, and `is_admin=1`.
- Only `POST /api/auth/login` on the Cloudflare Worker accepts All-Father username/password. The Worker runtime verifies the stored PHC Argon2id hash using `argon2-wasm-edge` with `parallelism=1`, `iterations=3`, `memorySize=65536`, and `hashLength=32`. The owner-controlled provisioning script and local smoke/tests use `hash-wasm` to produce/verify compatible PHC Argon2id hashes with the same parameters; do not describe the runtime dependency as `hash-wasm`.
- The Worker issues the existing ONYX access/refresh token pair. Access tokens are short-lived and protected requests re-read the current user, organization, role, active state, tenant, and revocation watermark from D1.
- User-provided `role`, `is_admin`, principal IDs, organization IDs, and client type do not grant authority. The canonical ID, explicit role, admin flag, and active state come from D1.
- All-Father-only organization and organization-admin routes enforce `ALL_FATHER` on the server. Ordinary users cannot claim or inherit this authority.

## Client and API contract

- Admin Shell uses `getServerAddress()` to target the Cloudflare Worker and posts `{ username, password, client_type: "admin" }` to `/api/auth/login`. It stores the returned ONYX token pair and sends the access token on protected requests.
- Web and mobile clients use ONYX credential authentication; there is no Google/Supabase fallback.
- Axum's legacy `POST /api/auth/supabase` route and its Supabase-specific principal bootstrap have been removed. Axum's ordinary username/password login rejects the reserved `allfather` username so it cannot become an alternate master-account path.
- The Worker has no `POST /api/auth/supabase` route. The retired path cannot issue an ONYX session.
- The historical D1 migration/column `users.supabase_user_id` is retained only for migration-history compatibility; current Worker authentication and provisioning do not read or write it. It is not an authorization input.

## Password provisioning and secret handling

GitHub Actions secrets `ONYX_ALLFATHER_USERNAME` and `ONYX_ALLFATHER_PASSWORD` are workflow inputs, not Worker or D1 configuration. They do not automatically provision the deployed account.

The repeatable command `npm run provision:allfather-password` hashes the supplied password, verifies the existing canonical principal and active organization, writes only the hash, writes a non-sensitive audit event, and verifies the stored hash. It never creates the principal or grants its role.

For CI, use only `--environment test`, which is hard-restricted to local D1 and `wrangler.dev.toml`. Production provisioning is intentionally not automated. After an authorized operator approves the account update, run the command from `deploy/cloudflare/edge-gateway` with Wrangler credentials and the two credentials supplied through environment variables, plus explicit flags:

```sh
npm run provision:allfather-password -- --environment production --approve-production __allfather__ --expect-database onyx-free-db
```

The production command selects the named remote D1 database, verifies the existing canonical row and active organization, and does not create or reactivate accounts. Do not run it without explicit production-data authorization. Do not pass the password on the command line or print it. No production database, secret, or deployed service was modified as part of this code change.

## Other user classes and unrelated services

- Organization Admins and Staff continue to use ONYX username/email plus password with salted Argon2id hashes.
- Supabase-specific All-Father code, client commands, and deployment configuration have been removed from this repository's runtime paths. Historical migrations are not deleted or rewritten. This does not authorize removal of any separately operated Supabase service outside the repository.
- D1 is authoritative for Worker users, active state, roles, organizations, session revocations, and authorization decisions.

## Required verification

The local smoke workflow must run against a loopback-only Worker and local D1, provision the test principal from Actions secrets, verify the canonical token and a protected All-Father operation, reject invalid/missing credentials, reject the retired exchange route, and prove that an ordinary user with forged client role/admin fields remains an ordinary user. Live production authentication remains unverified until the owner-approved provisioning step is performed and a secret-safe live smoke run succeeds.
