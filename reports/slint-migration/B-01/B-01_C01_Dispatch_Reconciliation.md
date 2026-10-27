# B-01 C-01 Dispatch Reconciliation — Evidence Addendum

**Status:** source reconciliation in progress; NOT a frozen contract.  
**Repository:** `SMozaff/Onyx-Framework`  
**Reviewed revision:** `main` `b056c6ef204080446e84b31f3d35a226e6a40867`  
**Worker source:** `deploy/cloudflare/edge-gateway/src/index.ts`, blob `f587f240c51ce84ec6d19db3b4de5f1a944e2e3f`  
**Existing API contract:** `docs/production/api-contract.md`, blob `4bf90e73dc4387f3610c845631bb985eb20c30c5`  
**Boundary:** this is source inspection only. It does not establish deployed revision, live behavior, D1 migration state, or approval of any shared contract.

## Dispatch-level reconciliation

The route table below records the dispatch predicate and handler observed in `dispatch()`. Route patterns are described literally; a prefix/regex predicate must not be silently documented as an exact path match.

| Method | Path / predicate in source | Dispatch result | Contract review note |
|---|---|---|---|
| Any | `OPTIONS` | 204 preflight | CORS headers are subsequently normalized by `applyCors`; test allowed and rejected origins. |
| Non-POST | `/api/auth/login` | 405 `METHOD_NOT_ALLOWED` | Explicit method guard. |
| Any | other `/api/auth/*` except login/refresh/logout | 404 `AUTH_ROUTE_NOT_FOUND` | Special auth-prefix guard precedes protected-session handling. |
| GET | `/health` | inline 200 health JSON | Does not prove downstream dependencies are ready. |
| GET | `/ready` | inline 200 or 503 based on `missingAuthSchema` | Readiness depends on schema probe; no deployed D1 state inferred. |
| GET | `/api/events` | `websocketEvents` | WebSocket and origin/ownership behavior require handler tests. |
| GET | prefix `/api/relay/` | `relaySocket` | Prefix predicate; not an exact one-segment route. |
| POST | `/api/auth/login` | `authLogin` | Login response shape must be reconciled with current Admin UI types/tests. |
| POST | `/api/auth/refresh` | `authRefresh` | Refresh lifecycle and replay semantics need explicit contract tests. |
| POST | `/api/auth/logout` | `authLogout` | Optional credentials/session-family behavior needs explicit tests. |
| GET | `/api/query` | `queryRoute` | Session required before dispatch. |
| POST | `/api/command` | `commandRoute` | Session required before dispatch. |
| POST | `/api/relay-ticket` | `issueRelayTicket` | Session required before dispatch. |
| GET | `/api/users` | `listUsers` | Session required before dispatch. |
| GET | `/api/users/hierarchy` | `hierarchy` | Session required before dispatch. |
| GET | `/api/admin/users` | `listAdminUsers` | Session required; handler owns role/tenant checks. |
| POST | `/api/admin/users` | `createUser(..., "STAFF")` | Session required; handler owns role/tenant checks. |
| POST | regex `^/api/admin/users/[^/]+/deactivate$` | `setUserActive(..., false)` | Exact suffix pattern; tenant and role behavior belongs to handler. |
| POST | regex `^/api/admin/users/[^/]+/activate$` | `setUserActive(..., true)` | Exact suffix pattern; tenant and role behavior belongs to handler. |
| POST | regex `^/api/admin/users/[^/]+/password$` | `setUserPassword` | Exact suffix pattern. |
| POST | regex `^/api/admin/users/[^/]+/manager$` | `setUserManager` | Exact suffix pattern. |
| POST | regex `^/api/admin/users/[^/]+/parent$` | `setUserParent` | Exact suffix pattern. |
| GET | `/api/admin/mobile-access` | `getMobileAccess` | Session required; authorization is handler-enforced. |
| PUT | `/api/admin/mobile-access` | `putMobileAccess` | Request/version/concurrency behavior needs contract tests. |
| POST | `/api/admin/policies` | `postAdminPolicy` | Policy transition semantics remain blocked by U-04 where applicable. |
| POST | `/api/admin/legal-holds` | `postAdminLegalHold` | Request and tenant-scoping semantics require tests. |
| POST | `/api/todo/lists` | `createTodoAggregate(..., "todo_list")` | Capability policy is server-side; do not infer it from client metadata. |
| POST | `/api/todo/targets` | `createTodoAggregate(..., "target_list")` | Same capability-policy boundary. |
| POST | `/api/todo/staff-loans` | `createTodoAggregate(..., "staff_loan")` | Same capability-policy boundary. |
| POST | regex `^/api/admin/users/[^/]+/class$` | `assignUserClass` | Exact suffix pattern. |
| GET | `/api/profiles` | `listProfiles` | Session required. |
| GET | prefix `/api/profiles/` | `listProfiles(..., last path segment)` | Prefix predicate is broader than the documented single `:ownerId` segment; malformed extra-segment behavior should be specified or tightened separately. |
| PUT | `/api/admin/profiles` | `upsertProfile` | Request/response DTO must be checked against shared profile contract (Q-05). |
| POST | `/api/admin/profiles/import` | `importProfiles` | Import validation/error contract needs fixture coverage. |
| GET | `/api/admin/profiles/export` | `exportProfiles` | Export DTO and error contract need fixture coverage. |
| POST | `/api/push/subscriptions` | `pushSubscribe` | Session required. |
| DELETE | prefix `/api/push/subscriptions/` | `pushDelete(..., last path segment)` | Prefix predicate is broader than a single `:id` segment. |
| POST | regex `^/api/admin/files/[^/]+/acl$` | `grantFileAccess` | Requires matched file hash and no user-id suffix. |
| DELETE | regex `^/api/admin/files/[^/]+/acl/[^/]+$` | `revokeFileAccess` | Requires matched file hash and user-id suffix. |
| GET | prefix `/api/files/` | `fileDownload(..., last path segment)` | Prefix predicate is broader than a single `:hash` segment. |
| GET | `/api/allfather/organizations` | `listOrganizations` | All-Father identity/role enforcement is handler-side. |
| POST | `/api/allfather/organizations` | `createOrganization` | Request/response contract needs a handler-level check. |
| PATCH | regex `^/api/allfather/organizations/[^/]+$` | `renameOrganization` | Exact one-segment identifier pattern. |
| POST | regex `^/api/allfather/organizations/[^/]+/deactivate$` | `setOrganizationActive(..., false)` | Exact suffix pattern. |
| POST | regex `^/api/allfather/organizations/[^/]+/activate$` | `setOrganizationActive(..., true)` | Exact suffix pattern. |
| GET | `/api/allfather/admins` | validates query keys and required `organization_id`, then `listOrganizationAdmins` | **Contract gap:** current table omits required `organization_id` and its 400 validation behavior. |
| POST | `/api/allfather/admins` | `createUser(..., "ORGANIZATION_ADMIN")` | Response DTO mismatch with UI is unresolved; do not fabricate timestamps. |
| POST | regex `^/api/allfather/admins/[^/]+/reactivate$` | `reactivateOrganizationAdmin` | Exact suffix pattern. |
| POST | `/api/admin/staff` | `createUser(..., "STAFF")` | Legacy alias; preserve only if cross-client contract confirms use. |
| DELETE | prefix `/api/admin/users/` | `deactivateUser(..., last path segment)` | **Path-shape risk:** broad prefix predicate, not exact `/api/admin/users/:id`; nested paths may reach the handler with only the final segment as ID. |
| Any unmatched path/method | fallback | 501 `ROUTE_NOT_MIGRATED` | Do not treat as success or rely on the fallback text as evidence that an Axum equivalent exists. |

## Contract gaps to resolve before C-01 freeze

1. **All-Father admin list query:** specify `GET /api/allfather/admins?organization_id=<id>`; missing/blank value or any query key other than `organization_id` returns a 400 `INVALID_ADMIN_QUERY` envelope according to the dispatch guard.
2. **Path shape and matching:** the profile, push-subscription, file-download, and admin-user DELETE routes use prefix checks and extract the last segment. Their effective behavior is broader than the single-parameter route notation in the existing contract. Decide whether to document the exact source behavior or separately authorize a route-hardening change; do not silently change runtime semantics in a client-only PR.
3. **Method mismatch/fallback:** dispatch matching is method-sensitive for most routes, so an unsupported method often falls through to 501 rather than a uniform 405. The login route is a special case. The client must not infer a generic 405 or retryability from route names.
4. **Session boundary:** except for health/readiness, preflight, auth handlers and WebSocket-specific handling, dispatch calls `requireSession` before ordinary routes. Handler-level role, tenant and ownership checks still determine the effective authorization contract.
5. **DTO/error evidence:** route predicates alone do not establish request/response DTOs or all error cases. Finish handler-level inspection and correlate with tests before freezing C-01. In particular, resolve organization-admin create DTO and C-03 error taxonomy.
6. **No deployment inference:** all findings above are source-only at the pinned commit. The deployed Worker revision and live D1 schema were not inspected.

## Gate / authorization status

The project owner has authorized the assistant to proceed with Gate 0b closure and, once mandatory evidence and approvals are actually satisfied, B-01 implementation. This authorization does **not** supply missing product decisions, B-13 feasibility reports, C-03/C-09 approvals, a security-policy disposition, or a signed Contract Freeze v1. Gate 0b therefore remains blocked until those artifacts exist. No production client code is authorized by this addendum.
