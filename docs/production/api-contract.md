# ONYX Production API Contract

**Source of truth:** `deploy/cloudflare/edge-gateway/src/index.ts`, especially `dispatch()`.
**Baseline:** `f0fca48cabd74f109096258f84925282bb349dba`; this is a source-level contract, not proof of deployed behavior.
**Unknown paths:** return `501 ROUTE_NOT_MIGRATED` with `FEATURE_UNAVAILABLE`; never treat that response as success.

Status terms: `implemented-in-source` means a dispatch branch and handler exist in this revision. It does not imply production deployment or successful end-to-end verification.

## Routes

| Method | Path | Authentication / authorization | Request / response contract |
|---|---|---|---|
| GET | `/health` | None | Reachability only; `{status:"ok",service,runtime}`. |
| GET | `/ready` | None | `200` when required D1 schema is present; otherwise `503` with missing schema items. |
| POST | `/api/auth/login` | None | `{username|email,password,client_type?}` → access/refresh token pair; All-Father is excluded from password login. |
| POST | `/api/auth/supabase` | Supabase bearer token | All-Father exchange; verified Supabase subject must match the provisioned canonical `__allfather__` principal. |
| POST | `/api/auth/refresh` | Refresh token in body | Single-use refresh token → rotated pair; replay is rejected. |
| POST | `/api/auth/logout` | Optional bearer and/or refresh token | Revokes supplied valid token/session family; returns `{success:true}`. |
| GET | `/api/query` | ONYX access session; role and ownership policy | Base64url query envelope → query result page. |
| POST | `/api/command` | ONYX access session; explicit command policy | Validated command envelope; unknown command denied. |
| GET | `/api/events` | ONYX session; origin allowlist | Realtime WebSocket upgrade; tenant and staff ownership filtering. |
| POST | `/api/relay-ticket` | ONYX access session; replica ownership | `{target_id,self_replica}` → short-lived scoped relay ticket. Staff capability uses server-managed class policy, not `client_type`. |
| GET | `/api/relay/:target_id` | Short-lived, scoped relay ticket | WebSocket upgrade; ticket target, expiry and single-use redemption checked. |
| GET | `/api/users` | ONYX access session | Active tenant-scoped identity picker. |
| GET | `/api/users/hierarchy` | ONYX access session | Canonical JSON array of hierarchy records; All-Father returns an empty array. |
| GET | `/api/admin/users` | ONYX access session | Tenant-scoped user directory. |
| POST | `/api/admin/users` | ORGANIZATION_ADMIN; own tenant | Creates a STAFF user; this alias matches the Admin UI contract. |
| POST | `/api/admin/staff` | ORGANIZATION_ADMIN; own tenant | Legacy STAFF creation alias. |
| POST | `/api/admin/users/:id/activate` | ORGANIZATION_ADMIN; tenant-scoped | Activates eligible STAFF account. |
| POST | `/api/admin/users/:id/deactivate` | ORGANIZATION_ADMIN; tenant-scoped | Deactivates eligible STAFF account. |
| POST | `/api/admin/users/:id/password` | ORGANIZATION_ADMIN; tenant-scoped | Resets STAFF password and revokes sessions. |
| POST | `/api/admin/users/:id/manager` | ORGANIZATION_ADMIN; tenant-scoped | Updates legacy manager flag. |
| POST | `/api/admin/users/:id/parent` | ORGANIZATION_ADMIN; tenant-scoped | Sets reporting parent; cycle checked. |
| POST | `/api/admin/users/:id/class` | ORGANIZATION_ADMIN; tenant-scoped | Sets operational class from the supported enum. |
| DELETE | `/api/admin/users/:id` | ALL_FATHER or ORGANIZATION_ADMIN | Deactivates eligible account; tenant and role boundaries enforced. |
| GET | `/api/admin/mobile-access` | ALL_FATHER or ORGANIZATION_ADMIN | Returns organization policy, version, timestamp and `enabled_server_policy` status. |
| PUT | `/api/admin/mobile-access` | ORGANIZATION_ADMIN; own tenant | `{allowed_classes,expected_version?}` → persisted, audited canonical policy with optimistic concurrency. |
| POST | `/api/admin/policies` | ORGANIZATION_ADMIN | `{name}` → durable policy creation. |
| POST | `/api/admin/legal-holds` | ORGANIZATION_ADMIN | `{target_id,target_type,reason}` → tenant-scoped legal hold creation. |
| GET | `/api/profiles` | ONYX access session | Profile list under role and ownership rules. |
| GET | `/api/profiles/:ownerId` | ONYX access session; STAFF own profile only | Owner-scoped profile list. |
| PUT | `/api/admin/profiles` | ORGANIZATION_ADMIN | Upserts a profile. |
| POST | `/api/admin/profiles/import` | ORGANIZATION_ADMIN | Imports profile records. |
| GET | `/api/admin/profiles/export` | ORGANIZATION_ADMIN | Exports profile records. |
| POST | `/api/todo/lists` | ONYX access session + mobile capability policy | Creates todo-list aggregate; staff class must be allowed by current organization policy. |
| POST | `/api/todo/targets` | ONYX access session + mobile capability policy | Creates target-list aggregate; staff class must be allowed by current organization policy. |
| POST | `/api/todo/staff-loans` | ONYX access session + mobile capability policy | Creates staff-loan aggregate; staff class must be allowed by current organization policy. |
| POST | `/api/push/subscriptions` | ONYX access session | Creates push subscription. |
| DELETE | `/api/push/subscriptions/:id` | ONYX access session; owner-scoped | Deletes own push subscription. |
| GET | `/api/files/:hash` | ONYX access session; role and ACL checks | Downloads a file only when authorization and ACL checks pass. |
| POST | `/api/admin/files/:hash/acl` | ORGANIZATION_ADMIN | Grants tenant-scoped file ACL access. |
| DELETE | `/api/admin/files/:hash/acl/:userId` | ORGANIZATION_ADMIN | Revokes tenant-scoped file ACL access. |
| POST | `/api/allfather/organizations` | ALL_FATHER | Creates an organization. |
| POST | `/api/allfather/admins` | ALL_FATHER | Creates an organization administrator. |

## Security and cross-client decisions

- **Identity and authority:** ONYX access sessions are verified server-side. Current account, organization, role and session revocation state are re-read from D1 for protected operations.
- **Mobile capability:** `users.class` intersected with `mobile_access_settings.allowed_classes` governs staff capability endpoints. Missing, malformed, inactive or unlisted staff records deny. The caller-supplied `client_type` claim is metadata only. This is a server-side class policy, **not device attestation** and not a login restriction.
- **Tenant isolation:** authorization and persistence must use the verified session's organization scope; caller-supplied tenant metadata is not authoritative.
- **Hierarchy wire shape:** JSON array of `{id,parent_user_id,is_admin}` records, matching the Rust/Android contract.
- **Errors:** stable `{error,category,retryability}` response shape; invalid input uses 400, unauthenticated 401, unauthorized 403, conflict 409, and infrastructure failures 5xx where defined.
- **CORS:** exact `ONYX_CORS_ORIGINS` allowlist. Wildcard origin must never be paired with credentialed responses.

## Verification boundary

This contract and the accompanying route-completeness test validate source-level expectations. They do not prove production D1 migration state, deployed Worker behavior, native OAuth, relay peer synchronization, or packaged client end-to-end success. Those remain release gates.
