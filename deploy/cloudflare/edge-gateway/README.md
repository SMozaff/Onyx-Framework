# ONYX Cloudflare Worker migration

This directory is now the start of the ONYX backend migration to Cloudflare's free Workers platform. It no longer proxies requests to Render or any other Axum/container origin.

The target architecture is:

```text
Mobile / Admin / Staff
        |
        v
Cloudflare Worker (free workers.dev)
        |
        +---- D1 (free SQLite-compatible database)
        |
        +---- Supabase Auth (Google for All-Father; email/password for Admin/Staff)
        |
        +---- Hugging Face object storage for existing blob data
```

Cloudflare Workers Free is the runtime boundary. No custom domain is required: the `workers.dev` hostname is sufficient. The migration deliberately does not use Cloudflare Containers, Workers Paid, Render, or another paid/container-only service.

## Locked authentication and authorization model

This model is binding for the migration:

- **All-Father only:** Google sign-in through Supabase Auth. The Worker verifies the signed Supabase JWT, resolves the bearer token through Supabase Auth, requires the returned user ID to match the JWT subject, requires a confirmed email, and matches the configured All-Father email. Supabase identity is not used to authenticate Admin or Staff accounts.
- **Organization Admin:** ONYX username or email plus password. Credentials are verified against the Argon2id password hash stored in D1.
- **Staff:** ONYX username or email plus password. Accounts are created, deactivated, and managed by an authorized organization Admin; staff cannot self-register or elevate roles.
- **D1 is authoritative** for ONYX users, password hashes, account status, roles, organization membership, and application authorization.
- **All-Father provisions organizations and Admins. Admins manage Staff only within their own organization.** The Worker must enforce these rules server-side.

Authentication endpoints:

- `POST /api/auth/supabase`: exchange a valid Supabase All-Father bearer token for an ONYX session.
- `POST /api/auth/login`: verify an ONYX username/email and password against D1, then issue an ONYX session.
- `POST /api/auth/refresh`: rotate a valid ONYX refresh token.
- `POST /api/auth/logout`: revoke supplied ONYX tokens.

D1 migration `0002_onyx_identity_model.sql` adds role/email/identity fields, organization records, and the seeded All-Father authority record. Admin and Staff passwords are Argon2id hashes in D1; their accounts are not provisioned in Supabase.

## Trust boundary: external claims vs ONYX-assigned privileges

Which facts are proven by an external identity provider, and which are assigned by ONYX. Only the
left column is ever *authenticated* externally; every privilege lives in the right column and is
decided server-side by the Worker against D1.

| Fact / claim | How it becomes trusted |
|---|---|
| Supabase JWT `sub`, `iss`, `exp`, signature | Verified by the Worker (`src/supabase.ts`): JWKS signature, RS256/ES256 allowlist, exact issuer, expiry |
| Supabase Auth user id, primary email, `email_confirmed_at` | Resolved server-side through `GET /auth/v1/user` with the presented bearer token; must equal the verified JWT `sub` and the configured All-Father email |
| `users.supabase_user_id` (external subject mapping) | **Owner-controlled provisioning only.** Login never creates, updates, or replaces it; an unmapped or mismatched subject is denied with `ALLFATHER_NOT_PROVISIONED` and no database write |
| `users.role` (`ALL_FATHER` / `ORGANIZATION_ADMIN` / `STAFF`) | Assigned by ONYX provisioning in D1 (All-Father provisions organizations and Admins; Admins manage Staff in their own organization) |
| `users.is_active`, `users.organization_id` | ONYX account lifecycle in D1; revalidated on login, refresh, and every protected request |
| ONYX access/refresh JWT validity | Issued by the Worker with `ONYX_JWT_SECRET` only after D1 checks; expiry and `token_revocations` rechecked on use |
| `role`, `is_admin`, `organization_id`, `class`, `parent_user_id` in any request body, query, or client store | **Never trusted as authority** — client-supplied values are ignored for authorization decisions |

Public authentication failures return stable error codes only (`INVALID_CREDENTIALS`,
`ALLFATHER_REQUIRED`, `ALLFATHER_NOT_PROVISIONED`, `INVALID_SUPABASE_TOKEN`,
`INVALID_REFRESH_TOKEN`, `UNAUTHORIZED`). The verifier's detailed reason is logged server-side as
a structured event (`allfather_subject_rejected`, `allfather_token_rejected`) that never contains
tokens, passwords, OAuth codes, or secrets.

## Current state

Phase 1 has converted the Worker from an origin proxy into an origin-free application boundary:

- `/health` is implemented locally in the Worker.
- `/ready` checks the D1 binding when it is configured.
- There is no `ONYX_ORIGIN_URL` and no upstream `fetch()` to Render.
- D1 migrations create the Worker’s authoritative users, organization, event, and audit tables; Supabase Auth stores credentials and authenticates identities.
- Unported ONYX API routes return `501 API_MIGRATION_IN_PROGRESS` rather than silently routing traffic to the unreachable legacy backend.

This is intentional. The Worker is not claimed to be a complete replacement until each API contract has been ported and verified.

## Deployment

The GitHub Actions workflow `.github/workflows/deploy-cloudflare-edge.yml` creates or reuses the free D1 database, applies checked-in migrations, configures Worker secrets, deploys the Worker, and checks `/health`, `/ready`, and anonymous access to a protected route. It requires Cloudflare and Supabase credentials configured as GitHub Actions secrets. The workflow is manually dispatchable; deployment credentials are never committed to the repository.

For local development after authenticating Wrangler:

```bash
npm install
npm run d1:local
npm run dev
```

Do not copy production secrets into `.dev.vars` or commit them.

## Migration rule

The Rust/Axum API remains the reference implementation while individual contracts are ported. We do not rewrite the whole service blindly. For every route, the migration must preserve:

1. request and response JSON contracts;
2. authentication and capability checks;
3. tenant/organization isolation;
4. idempotency and audit semantics where applicable;
5. existing D1-compatible persistence behavior;
6. client compatibility for Mobile, Admin, and Staff shells;
7. automated tests before the route is switched from `501` to live behavior.

Only after all required routes are live and verified will the legacy Render deployment/configuration be removed.
