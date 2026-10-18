# ONYX Authentication and Authorization Model

**Status: authoritative security requirement for all ONYX clients and deployments.**

This document governs the ONYX authentication migration. Do not replace this design with a single third-party identity provider for every user class.

## Identity and authority boundaries

### All-Father: ONYX-managed credentials

- All-Father signs in with the canonical ONYX username and password for principal `__allfather__`.
- The Worker verifies the supplied password against the Argon2id hash stored in D1.
- The account must be active, belong to an active organization, have explicit `ALL_FATHER` role, and have the admin flag enabled.
- GitHub Actions secrets `ONYX_ALLFATHER_USERNAME` and `ONYX_ALLFATHER_PASSWORD` are used only by CI to exercise `POST /api/auth/login`; they do not provision the D1 account or become Worker secrets.
- Google and Supabase are not part of All-Father authentication.
- All-Father provisions organizations and organization administrators.

### Organization administrators: ONYX-managed credentials

- Admins authenticate using either username + password or email + password.
- Credentials and password hashes are managed by ONYX; do not require Admins to sign in through a third-party identity provider.
- Passwords must be stored as strong salted Argon2id hashes. Never store plaintext passwords or placeholder hashes.
- D1 is authoritative for user identity, active status, role, and organization membership in the Cloudflare deployment.

### Staff: admin-provisioned ONYX accounts

- Staff accounts are created, deactivated, and managed by an authorized Admin.
- Staff cannot self-register, create organizations, create Admins, or elevate their own role.
- An Admin may manage Staff only within that Admin's own organization.
- Staff use ONYX username/email + password authentication. Supabase Auth is reserved for the designated All-Father identity.

### Render/Axum API configuration

The Cloudflare Worker All-Father login path is ONYX-managed credentials. Any separate Axum deployment must be evaluated against this same contract before release; do not reintroduce Google/Supabase login for All-Father.

## Roles and authorization

At minimum, distinguish:

- `ALL_FATHER`: cross-organization authority limited to explicitly defined All-Father operations, including organization and Admin provisioning.
- `ORGANIZATION_ADMIN`: authority within the Admin's own active organization, including Staff provisioning/deactivation.
- `STAFF`: only the capabilities granted to Staff within their own organization.

Every protected operation must enforce role and organization boundaries server-side. Never trust role, organization ID, or account status supplied by a client. Re-fetch or validate authoritative account status when exchanging/refreshing sessions and reject deactivated accounts. The UI is not an authorization boundary.

## Cloudflare Worker authentication endpoints

- `POST /api/auth/login`: accepts ONYX username/email and password, verifies the Argon2id hash, and issues an ONYX session for an active account. All-Father is eligible only when the account ID is `__allfather__`, the explicit role is `ALL_FATHER`, and the admin flag is true.
- `POST /api/auth/refresh`: rotates an ONYX refresh token after re-checking account, role, organization, and revocation state.
- `POST /api/auth/logout`: revokes ONYX tokens/session.
- There is no Worker `POST /api/auth/supabase` route.

## Authoritative data and safe migration

D1 owns Cloudflare-deployment records for ONYX users, roles, active state, organization membership, and authorization decisions. Password hashes are stored only in the ONYX user record. Keep session signing keys and Supabase secrets in deployment secrets, never in source control.

Do not switch production traffic until the Worker implementation and migrations satisfy this contract and GitHub Actions verifies the relevant build and authorization tests. During migration, do not silently fall back to another identity provider or grant broad access when configuration is missing.


## Implementation reconciliation (2026-10-10)

The normative contract above remains authoritative. The current Worker implementation has known gaps; see [reports/auth-repair/02-auth-contract.md](../reports/auth-repair/02-auth-contract.md) for the route matrix, evidence, and unresolved decisions.

In particular:

- The Supabase subject mapping must be provisioned explicitly. Login must not create, update, or replace users.supabase_user_id. A subject mismatch must be denied without a database write.
- Supabase JWT verification must enforce signature, algorithm, issuer, expiry, subject, and the approved audience/project claim policy. The Supabase Auth user response must match the verified JWT subject and the designated confirmed identity.
- Public authentication errors must be generic. Return stable error codes only; do not return raw verifier or upstream provider error messages.
- ONYX access tokens currently last 1 hour and refresh tokens 7 days. Refresh rotates the presented refresh token using D1 token revocation records. These are current implementation values, not a substitute for tests or a complete session policy.
- Protected requests must revalidate current account and organization status, role, tenant membership, and applicable client capability. Authorization for each protected command/resource remains server-side.
- Historical note resolved by the handover below: the Admin Shell uses Authorization Code Flow with PKCE, not the older implicit-style fragment callback. Do not reintroduce implicit token fragments; verify browser/Tauri redirect behavior through the PKCE flow before changing provider settings.
- A canonical current-user/session introspection route and server-backed logout wiring are not confirmed in the inspected implementation.

This addendum records source evidence and must not be read as confirmation that production configuration or end-to-end authentication has been verified. No migrations, production configuration, secrets, or deployed services were changed in Phase 2.

### Implementation reconciliation (Phase 4, 2026-10-11)

- The single `users.supabase_user_id` binding is written **only** by the owner-run command
  `deploy/cloudflare/edge-gateway/scripts/provision-allfather.mjs` (see
  [reports/auth-repair/04-allfather-provisioning.md](../reports/auth-repair/04-allfather-provisioning.md)
  and the Worker README). It is idempotent, environment-aware, requires explicit approval for
  production, audits every change, and refuses to create a principal, double-bind a subject,
  rebind without approval, or activate an inactive principal silently.
- `ALL_FATHER` is never inferred from a mutable attribute: an account whose `users.role` is empty is
  no longer promoted by the literal username `allfather` (fixed in `src/index.ts` `roleForUser`).
- Google/Supabase authentication alone still cannot grant All-Father access: the verified subject
  must match the explicitly provisioned mapping, the resolved Supabase user must match the JWT
  subject with a confirmed email, and the ONYX principal must be active with an explicit role.

### Implementation reconciliation (handover, 2026-10-11)

- **Browser sign-in is now Authorization Code Flow with PKCE**, replacing the implicit flow
  (`flow_type=implicit` + `access_token` in the URL fragment) that the 2026-10-10 record flagged as
  unresolved. `crates/bins/admin-shell/ui/src/auth/allfatherOAuth.ts` generates a 32-byte random
  verifier, its RFC 7636 S256 challenge, and a separate random CSRF nonce; the nonce and verifier
  are stored per-tab in `sessionStorage` and the exchange uses only the publishable key
  (`POST /auth/v1/token?grant_type=pkce`). The authorization code is scrubbed from the address bar
  and history before any network call, a legacy implicit-flow fragment is refused rather than
  processed, and state is validated as missing/mismatched/expired/replayed fail-closed. OAuth `state`
  is deliberately not sent to `/authorize`, which owns that parameter internally and rejects a
  caller-supplied value (supabase/auth#1548); the nonce travels inside the allow-listed
  `redirect_to` instead. See
  [reports/auth-repair/05-pkce-and-identity-review.md](../reports/auth-repair/05-pkce-and-identity-review.md).
- **Supabase JWT verification now enforces the audience.** `src/supabase.ts` requires
  `aud === "authenticated"` (string or array form) in addition to the existing signature,
  RS256/ES256 allowlist, exact issuer, expiry and subject checks, and rejects a token whose
  optional `nbf` is in the future. This satisfies the "approved audience/project claim policy"
  requirement above; the value is a hardcoded constant rather than an env var so the Worker cannot
  be deployed with audience checking silently disabled.
- **The documented `npm run provision:allfather` command now exists.** `package.json` had lost the
  script in a merge, so the README's documented command could not run; the script was restored and
  is covered by a test that asserts the documented alias and the file it invokes both exist.
- **Nothing here changes the privilege model.** The Worker still denies every Supabase subject that
  is not explicitly provisioned (`ALLFATHER_NOT_PROVISIONED`, no database write), role still comes
  only from `users.role`, and no production migration, principal, binding, secret or deployment was
  touched. Production D1 state, the provisioned mapping, and end-to-end Google sign-in remain
  **unverified** pending owner-run read-only checks.
