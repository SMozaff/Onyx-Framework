# Remediation 01 — Worker Authorization and Session Security

Status: implementation on a feature branch; not a production deployment. Treat this document as an implementation inventory, not a release approval.

## Source of truth

- `src/authorization.ts` is the explicit query and command role/action matrix. Any action absent from the matrix is denied.
- `src/command-semantics.ts` is the strict payload schema and lifecycle transition contract. A command does not run merely because its role is allowed.
- `src/resource-scope.ts` defines D1 resource-reference normalization and the SQL-level Staff ownership predicate.
- `src/index.ts` resolves the principal, organization status, and role from D1 on each protected request, then applies route-specific authorization.

## Identity and authority

- All-Father is allowed only through the Supabase identity exchange, after provider-token verification, a confirmed designated identity, the canonical D1 principal `id='__allfather__'`, explicit `role='ALL_FATHER'`, active status, and an exact `supabase_user_id === JWT sub` mapping.
- The password login path rejects the canonical All-Father principal and all rows without a recognized explicit D1 role. It does not create or alter Supabase mappings.
- Admin and Staff continue to use ONYX username/password sessions.
- `client_type` is retained only as an informational token field; it is not a command/query capability.
- Protected requests require an unexpired signed ONYX access token with the configured Worker issuer and audience, and verify token/session-family revocation. D1 user activity, organization activity, role, tenant, and legacy admin flag are rechecked on every protected request.
- Refresh tokens are one-use: the D1 revocation insert must report one change before a replacement pair is issued. Access and refresh tokens share a random session-family ID. Logout revokes the family even if the client submits only its access token.
- **Deployment compatibility gate:** the stricter Worker JWT checks reject access/refresh tokens minted before issuer, audience and session-family claims were added. Treat rollout as a forced reauthentication event; validate that desktop/web/mobile clients return to login cleanly and do not retry stale refresh tokens indefinitely. This is an app-session cutover, not a reason to reset D1.

## Route inventory

| Route | Access model | Enforcement |
| --- | --- | --- |
| `GET /health`, `GET /ready` | Public health/readiness | No user data or mutations |
| `POST /api/auth/login` | Public entry to ONYX Admin/Staff credentials | Active user + active organization + recognized explicit D1 role + password verification; All-Father excluded |
| `POST /api/auth/supabase` | All-Father identity exchange | Verified Supabase identity, confirmed designated account, canonical D1 role and exact subject binding |
| `POST /api/auth/refresh` | Signed refresh token | Active D1 user/organization, unchanged role/tenant/admin flag, exact All-Father subject binding, one-use refresh token |
| `POST /api/auth/logout` | Idempotent revocation endpoint | Verifies supplied token(s), revokes token hash and session-family hash |
| `GET /api/query` | Role/query matrix | Unknown query denied; requested organization must match the session; Staff aggregate reads include a D1 SQL ownership predicate |
| `POST /api/command` | Role/action/resource matrix + semantic policy | Unknown command denied; exact action/target type, tenant, payload schema, state transition, owner/reference, version and epoch checks; mutation/event/idempotency/audit are batched |
| `GET /api/users/hierarchy` | Tenant-scoped | Admin gets same-organization hierarchy; Staff sees self and direct parent only |
| `GET /api/profiles[/<id>]` | Tenant-scoped; Staff self-only | Reads constrained by organization and profile ID |
| `POST /api/push/subscriptions`, `DELETE /api/push/subscriptions/<id>` | Session owner | Subscription rows bind user and organization; conflicting reassignment is rejected |
| `GET /api/files/<hash>` | All-Father or organization Admin | Organization-scoped file lookup; Staff is denied until per-resource ACL/owner data exists |
| `GET /api/events` | All-Father or organization Admin | Organization-scoped event records |
| `POST /api/allfather/organizations` | All-Father only | Role gate; create + audit batched |
| `POST /api/allfather/admins` | All-Father only | Active target organization; create + audit batched |
| `POST /api/admin/staff` | Organization Admin only | Creates Staff in the actor's organization; create + audit batched |
| `DELETE /api/admin/users/<id>` | All-Father deactivates Admin; organization Admin deactivates Staff in the same tenant | Rechecks target role and tenant, scopes the write itself, and batches audit |
| Unmatched routes | Not implicitly public | Return `501 ROUTE_NOT_MIGRATED`; they are not claimed as implemented |

## Explicit command policy

The source-code matrix is authoritative and table-tested over every registered role × command pair. The current role boundaries include:

- Staff: owned task work, acknowledgement and permitted todo/target submissions only. Existing resource ownership is mandatory where the matrix calls for it.
- Organization Admin: organization-scoped management, task/mission lifecycle, policy and legal-hold actions allowed by the explicit matrix.
- All-Father: only actions expressly present in the matrix and the dedicated identity-provisioning endpoints; it does not bypass tenant predicates for generic queries or commands.
- Unknown action names, target-type mismatches, privileged actions outside the caller's role, unexpected payload keys, client-forged state/status/role/tenant properties, and stale resource versions are denied.

## Intentionally fail-closed commands

Some domain commands remain unavailable through the Worker because its present schema cannot prove native-domain preconditions safely. These return a denial rather than fabricating success:

- Staff-loan approve/decline/extend/end: the Worker cannot establish real-owner, borrowing-manager, and loaned-staff relationships.
- Target-list verification: no proven check of the authoritative time-window end plus verifier relationship.
- Todo/target escalation: the Worker cannot prove the next authorized manager in the hierarchy.
- Policy evaluation/violation registration: no implemented native-equivalent result/projection.
- File downloads for Staff: the table does not yet contain resource owner/ACL fields.

These are functional release blockers for any client flow that requires them. Do not un-disable them until D1 data and tests prove the required authority.

## Tests and release evidence

- `test/authorization.test.ts`: role × command matrix, query roles, unknown action and target spoofing denial, ownership checks, strict payload/state-transition behavior, and fail-closed domain preconditions.
- `test/security.test.ts`: client-type invariance, unknown/deletion-shaped command denial, cross-tenant target denial without writes, inactive account/organization denial, All-Father subject mismatch, malformed/expired/wrong-issuer/wrong-audience token denial, refresh replay, logout revocation, push-subscription ownership, and SQL-level Staff query scope.
- `test/auth.test.ts`: Supabase All-Father mapping and ordinary password-login behavior, including blank/invalid role denial.
- The Worker CI must pass on the final commit; preview tests must use synthetic principals/organizations. Record the exact commit, CI URL, preview report, Worker version and rollback plan before any release.
- No production database writes, secret changes, or deployments are authorized by this remediation branch.
