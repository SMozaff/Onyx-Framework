# C-01 Worker API Contract — Draft Reconciliation

**Source of truth:** `deploy/cloudflare/edge-gateway/src/index.ts`, especially `dispatch()`.
**Repository baseline:** `main` at `b056c6ef204080446e84b31f3d35a226e6a40867` (verified 2026-10-27).  
**Worker source file blob at that baseline:** `f587f240c51ce84ec6d19db3b4de5f1a944e2e3f`.  
**Existing contract file blob:** `4bf90e73dc4387f3610c845631bb985eb20c30c5`; its embedded source-baseline field was `38c8975fd6616376d51e016b3e718b7736888514` and is stale relative to the current source blob. This draft carries forward the 47 route rows from that contract file as a working inventory; it is not yet a completed handler-by-handler C-01 reconciliation and is not proof of deployed behavior.
**Unknown paths:** the Worker currently returns `501 ROUTE_NOT_MIGRATED` with `FEATURE_UNAVAILABLE`. Its response text generically says an equivalent Axum route exists; that text is not evidence that a native route actually exists. Check the native router separately, and never treat a `501` as success.

Status terms: `implemented-in-source` means a dispatch branch and handler exist in this revision. It does not imply production deployment or successful end-to-end verification.

## Routes

| Method | Path | Authentication / authorization | Request / response contract |
|---|---|---|---|
| GET | `/health` | None | Reachability only; `{status:"ok",service,runtime}`. |
| GET | `/ready` | None | `200` when required D1 schema is present; otherwise `503` with missing schema items. |
| POST | `/api/auth/login` | None | `{username|email,password,client_type?}` → access/refresh token pair; All-Father is allowed only for canonical `__allfather__` with explicit `ALL_FATHER` role and admin flag. |
| GET | `/api/allfather/organizations` | ALL_FATHER | Lists organizations from the Worker/D1 store. |
| PATCH | `/api/allfather/organizations/:id` | ALL_FATHER | Renames the selected organization. |
| POST | `/api/allfather/organizations/:id/activate` | ALL_FATHER | Reactivates an eligible organization. |
| POST | `/api/allfather/organizations/:id/deactivate` | ALL_FATHER | Deactivates an organization. |
| GET | `/api/allfather/admins` | ALL_FATHER | Lists organization administrator accounts. |
| POST | `/api/allfather/admins/:id/reactivate` | ALL_FATHER | Reactivates an eligible organization administrator. |
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

## Known Worker/native API differences

The native Axum router is registered separately in `crates/bins/api-server/src/routes/mod.rs`; it is not an upstream proxy for this Worker. In particular, native Axum registers `GET /api/admin/users/:id`, while the Worker dispatch table does not. A GET to that path on the Worker falls through to its `501 ROUTE_NOT_MIGRATED` response. Do not assume route parity from matching route names or from the OpenAPI subset. The Worker additionally implements the All-Father organization/admin management routes listed above; native Axum does not provide those as the canonical All-Father login/administration path.

## Security and cross-client decisions

- **Identity and authority:** ONYX access sessions are verified server-side. Current account, organization, role and session revocation state are re-read from D1 for protected operations.
- **Mobile capability:** `users.class` intersected with `mobile_access_settings.allowed_classes` governs staff capability endpoints. Missing, malformed, inactive or unlisted staff records deny. The caller-supplied `client_type` claim is metadata only. This is a server-side class policy, **not device attestation** and not a login restriction.
- **Tenant isolation:** authorization and persistence must use the verified session's organization scope; caller-supplied tenant metadata is not authoritative.
- **Hierarchy wire shape:** JSON array of `{id,parent_user_id,is_admin}` records, matching the Rust/Android contract.
- **Errors:** the Worker fallback is `501 ROUTE_NOT_MIGRATED` for unimplemented paths. Many handlers return `{error,category,retryability}`, but response envelopes remain route-specific; callers and docs must follow the actual handler/tests rather than assume every route has a fully uniform shape.
- **CORS:** exact `ONYX_CORS_ORIGINS` allowlist. Wildcard origin must never be paired with credentialed responses.

## Verification boundary

This contract and the accompanying route-completeness test validate source-level expectations. They do not prove production D1 migration state, deployed Worker behavior, relay peer synchronization, or packaged client end-to-end success. Those remain release gates.


## BOUND status and unresolved reconciliation

**Status: DRAFT — NOT FROZEN. Gate 0b is NOT APPROVED. Do not implement production client modules until the authorized gate signature exists.**

This route inventory is transcribed from the current checked-in contract document at the blob SHA above. It must be verified against the exact Worker source revision route-by-route, including request/response fields, error variants, auth rules, timeout, retry/idempotency and concurrency semantics. A static route-string comparison alone is insufficient for dynamic routes or handler semantics.

### Required decisions before freeze

- **Organization-admin create DTO:** Worker `POST /api/allfather/admins` reportedly returns `is_admin` and `role`, while the current UI `OrganizationAdminRow` expects `created_at` and `updated_at`. B-01/B-10/API owner must approve the canonical shape. Do not fabricate timestamps or use unchecked casts.
- **C-03:** error envelopes vary by route; agree safe internal error variants with B-04/B-02 and preserve machine codes without exposing raw bodies.
- **C-09:** B-13 must approve fixture schema/version and provenance before candidates are treated as authoritative contract tests.
- **Source/deployment distinction:** no deployed Worker revision or live behavior was verified in this task. This draft documents source-level expectations only.
- **Open Gate 0b dependencies:** P-01, Q-01, Q-05, Q-14, Q-16/U-05, U-04, U-01/U-02/U-03/U-10/U-12 and signed Contract Freeze v1 remain unresolved in the accessible decision evidence.

### Freeze checklist

- [ ] Handler-by-handler request/response/auth/error reconciliation against Worker source at the pinned revision.
- [ ] Exact operation-to-caller mapping for every B-01-consumed endpoint.
- [ ] DTO mismatch disposition approved by B-10/API owner.
- [ ] C-03 taxonomy/localization mapping approved by B-04.
- [ ] C-09 schema/version and fixtures approved by B-13.
- [ ] Gate 0b decision log signed with approver/date and scoped implementation authorization.
