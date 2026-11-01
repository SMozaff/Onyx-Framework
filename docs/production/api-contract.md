# ONYX Cloudflare Worker API Contract

**Source of truth:** `deploy/cloudflare/edge-gateway/src/index.ts`, especially `dispatch()`.
**Source baseline:** `98a2258` (main, re-pinned 2026-10-31; previous baseline `38c8975`, 2026-10-23). Verified by `deploy/cloudflare/edge-gateway/test/route-completeness.test.ts` (42 passing). This is a source-level contract, not proof of deployed behavior.
**Unknown paths:** the Worker currently returns `501 ROUTE_NOT_MIGRATED` with `FEATURE_UNAVAILABLE`. Its response text generically says an equivalent Axum route exists; that text is not evidence that a native route actually exists. Check the native router separately, and never treat a `501` as success.

Status terms: `implemented-in-source` means a dispatch branch and handler exist in this revision. It does not imply production deployment or successful end-to-end verification.

## Routes

| Method | Path | Authentication / authorization | Request / response contract |
|---|---|---|---|
| GET | `/health` | None | Reachability only; `{status:"ok",service,runtime}`. |
| GET | `/ready` | None | `200` when required D1 schema is present; otherwise `503` with missing schema items. |
| POST | `/api/auth/login` | None | `{username|onyx_id|email,password,client_type?}` → access/refresh token pair. Staff sign in with their Onyx ID; username and email remain accepted aliases during transition. The `user` object includes `onyx_id` and `must_change_password`. All-Father is allowed only for canonical `__allfather__` with explicit `ALL_FATHER` role and admin flag. |
| POST | `/api/auth/password` | ONYX access session; allowed while `must_change_password` is set | `{current_password,new_password}` → self-service password change. New password 12+ characters, must differ from the current one. Clears `must_change_password` and revokes all sessions for the account (the user signs in again). `403 CURRENT_PASSWORD_INCORRECT` on a wrong current password. |
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
| POST | `/api/sync/outbox` | ONYX access session; replica ownership (`replica_ownership` must match the user and organization) | `{replica_id,items:[{outbox_id,event_id,event_type,aggregate_id,organization_id,occurred_at_millis,payload}]}` (max 100 items, 64 KiB payload each) → `{acks:[{outbox_id,status:"stored"}\|{outbox_id,status:"rejected",retryable,reason}]}`. Rows are keyed by `(replica_id,outbox_id)`, so re-sending is idempotent. A batch-level `503 OUTBOX_STORE_UNAVAILABLE` is retryable. |
| GET | `/api/relay/:target_id` | Short-lived, scoped relay ticket | WebSocket upgrade; ticket target, expiry and single-use redemption checked. |
| GET | `/api/users` | ONYX access session | Active tenant-scoped identity picker. |
| GET | `/api/users/hierarchy` | ONYX access session | Canonical JSON array of hierarchy records; All-Father returns an empty array. |
| GET | `/api/admin/users` | ONYX access session | Tenant-scoped user directory. |
| POST | `/api/admin/users` | ORGANIZATION_ADMIN; own tenant | Creates one staff account with a generated staff ID (`onyx_id`) and a generated one-time password. Body: staff-sheet fields (`first_name`, `last_name`, `national_id` required; `contact_email` or `email`, `contact_phone`, `job_title`, `department`, `class_label`, `parent_onyx_id`, `team_ids`, `first_name_fa`, `last_name_fa`, `class` optional; `onyx_id` ignored). A `password` or `username` key is rejected (`400 INVALID_BODY`). Response `201` is the creation report (`.xlsx`, `Content-Type: application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`), the only place the one-time password is returned. The account must change the password at first sign-in. |
| POST | `/api/admin/staff` | ORGANIZATION_ADMIN; own tenant | Legacy alias of `POST /api/admin/users`, same body and response. |
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
| POST | `/api/admin/profiles/import` | ORGANIZATION_ADMIN | Batch staff import. Body: `.xlsx` (detected by content; first sheet, header row required), CSV, or a JSON array of objects, all using the staff-sheet column names (`onyx_id`, `first_name`, `last_name`, `national_id` are required columns for `.xlsx` and CSV). **Every non-empty row creates an account, a generated staff ID, a profile and a one-time password.** A supplied `onyx_id` is ignored and reported in the row notes. Row results are per row in `account_status` (`created`, or `failed: <CODE>`); a failed row creates nothing and consumes no ID number. Response `200` is the creation report (`.xlsx`), for the whole batch. Counts are in headers `X-Onyx-Import-Total`, `X-Onyx-Import-Created`, `X-Onyx-Import-Failed`. Limits: 2,000,000 bytes, 500 non-empty rows. Errors before any row is processed: `400 INVALID_PROFILE_IMPORT` (unreadable file), `400 IMPORT_MISSING_COLUMN` (with `column`), `400 IMPORT_TOO_MANY_ROWS`, `413 IMPORT_TOO_LARGE`. Replaces the earlier profile-only import, which required an existing account per row (`user_id`). |
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

### Forced password change (backend accounts)

While an account's `must_change_password` is set, `requireSession` returns `409 PASSWORD_CHANGE_REQUIRED` for every route except `POST /api/auth/password` and `POST /api/auth/logout`. This applies to the WebSocket upgrade (`/api/events`) and to relay tickets (`/api/relay-ticket`), because both verify the session through `requireSession`. Refresh (`/api/auth/refresh`) is not gated.

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
