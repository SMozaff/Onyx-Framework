# ONYX Production Baseline Re-verification — Step 1

**Assessment date:** 2026-10-12  
**Repository baseline:** `main` at `bff822b464e9b95d80393c4b0f2545e0ed032263`  
**Scope:** Read-only verification of Worker configuration, production D1 schema/ledger, Supabase Auth census, and source-level API contracts.  
**Safety:** No production writes, D1 mutations, identity provisioning, secret changes, bindings changes, or deployments were performed.

## 1. Executive status

**Step 1 is partially complete.** The live D1 database and Worker binding are positively identified and the auth schema is present. The current All-Father path is fail-closed, but no Supabase Auth users exist and the canonical D1 principal has no subject mapping, so the intended All-Father → ONYX session exchange cannot currently complete. The route contract is also incomplete for the desktop shells and Android.

| Gate | Current evidence | Status |
|---|---|---|
| Identify production Worker and D1 binding | Cloudflare Worker settings API returned 200; `DB` points at the pinned UUID below | Verified |
| Read production migration ledger/schema | Read-only D1 queries succeeded; five migration records and required auth schema/indexes present | Verified |
| Verify expected binding/secret presence | Expected binding names/types exist; secret values were not read | Presence verified, values not inspected |
| Establish Supabase → ONYX identity mapping | Supabase Auth census: 0 users; D1 `__allfather__` has `supabase_user_id = NULL` | Blocked |
| Route contract across clients and Worker | Several deployed-reference routes are not implemented in the Worker; hierarchy and WebSocket contracts differ | Blocked |
| Direct production `/health` and `/ready` in this audit session | Current web/container environment could not access these endpoints; deployment workflow contains live smoke steps | Still unverified here |
| Source commit → active Worker version | Cloudflare reports Worker version #10, but returned version metadata has no source commit annotation | Not conclusively pinned |
| Acceptance evidence attached to PR | This document records the read-only evidence; CI/owner review remain required | In progress |

## 2. Production binding, D1, and identity facts

### Cloudflare Worker configuration

Read-only `GET /accounts/{account_id}/workers/scripts/onyx-framework/settings` returned HTTP 200.

- Worker: `onyx-framework`
- Compatibility date: `2026-10-09`
- D1 binding: `DB`
- **Pinned D1 database UUID:** `7b757d0f-3484-48a5-9422-eadabed2a995`
- CORS allowlist currently contains the Pages origins `https://onyx-web-ui.pages.dev` and `https://onyx-mobile-pwa.pages.dev`.

Expected Worker bindings were present by name/type: `ONYX_JWT_SECRET`, `SUPABASE_SECRET_KEY`, `SUPABASE_PUBLISHABLE_KEY`, `SUPABASE_URL`, `SUPABASE_JWKS_URL`, `ONYX_ALLFATHER_EMAIL`, `ONYX_CORS_ORIGINS`, and the optional S3 credentials/configuration. No secret contents were retrieved, printed, or evaluated; presence does not prove correctness, strength, freshness, or provider-side validity.

### Production D1 read-only queries

All D1 queries in this audit were SELECT/PRAGMA statements; Cloudflare metadata reported `changed_db=false` and `rows_written=0`.

- `d1_migrations` contains five records: `0001_foundation.sql`, `0002_auth_hierarchy.sql`, `0002_onyx_identity_model.sql`, `0003_supabase_identity.sql`, and `0004_auth_schema_repair.sql`.
- Required auth tables exist: `users`, `organizations`, `token_revocations`, and `audit_log`; supporting tables `aggregates`, `domain_events`, `idempotency`, `push_subscriptions`, and `file_assets` also exist.
- `users` has the required auth columns, including `email`, `role`, and `supabase_user_id`.
- Required indexes exist, including unique partial `idx_users_supabase_user`, unique partial `idx_users_email_lower`, unique `idx_users_username_lower`, `idx_users_org_role_active`, and `idx_users_role`.
- Exactly one user exists: the active canonical principal `id='__allfather__'`, `username='allfather'`, `role='ALL_FATHER'`, organization `'__allfather__'`.
- The canonical principal's `supabase_user_id` is **NULL**.
- One active system organization exists; there are no Admin or Staff users and no users with a non-NULL Supabase subject mapping.

The two `0002_` filenames are still present as migration history. The current `0002_onyx_identity_model.sql` is an idempotent, column-free bridge, and `0004_auth_schema_repair.sql` ensures the canonical indexes/principal. The live ledger and schema match this repaired layout. **Do not rename applied migration files or reset/recreate this database.**

### Supabase Auth census

A read-only query against project `uhpqjhbciimklheohvfr` returned:

- `auth.users` total: **0**
- Configured designated All-Father identity users: **0**

Current Worker source requires an already-bound D1 `supabase_user_id` matching the verified Supabase JWT subject and denies unmapped identities with `403 ALLFATHER_NOT_PROVISIONED`; login performs no mapping write. Therefore the safe current behavior is denial, not privilege escalation. A Google/Supabase identity must be created through the approved identity-provider flow, then provisioned to the canonical D1 row using the guarded owner-controlled provisioning path before an All-Father ONYX session can succeed. Provisioning must not be replaced by email matching or a login-time update.

## 3. API route contract: clients versus Cloudflare Worker

Reference sources:
- [Worker router](https://github.com/SMozaff/Onyx-Framework/blob/bff822b464e9b95d80393c4b0f2545e0ed032263/deploy/cloudflare/edge-gateway/src/index.ts)
- [Rust API route registration](https://github.com/SMozaff/Onyx-Framework/blob/bff822b464e9b95d80393c4b0f2545e0ed032263/crates/bins/api-server/src/routes/mod.rs)
- [Android AuthApi](https://github.com/SMozaff/Onyx-Framework/blob/bff822b464e9b95d80393c4b0f2545e0ed032263/mobile-android/app/src/main/kotlin/com/onyx/net/AuthApi.kt)
- [Admin Settings UI](https://github.com/SMozaff/Onyx-Framework/blob/bff822b464e9b95d80393c4b0f2545e0ed032263/crates/bins/admin-shell/ui/src/pages/Settings.tsx)
- [Staff desktop login](https://github.com/SMozaff/Onyx-Framework/blob/bff822b464e9b95d80393c4b0f2545e0ed032263/crates/bins/desktop-shell/ui/src/pages/Login.tsx)

| Route or surface | Worker status | Contract result |
|---|---|---|
| `POST /api/auth/login`, `/api/auth/refresh`, `/api/auth/logout` | Implemented in source | Not proven end-to-end; production D1 has no Admin/Staff accounts |
| `POST /api/auth/supabase` | Implemented; requires an existing canonical subject mapping | No Supabase Auth user or mapping currently exists, so privileged session exchange is blocked |
| `GET /api/query`, `POST /api/command` | Implemented, but command authorization is incomplete | Security blocker; see §4 |
| `GET /api/users/hierarchy` | Returns `{ "users": [...] }` | **Incompatible:** Android calls `JSONArray(responseBody)`; Rust/mobile-core documents the top-level JSON array contract |
| `GET /api/users`; `GET/POST /api/admin/users` | Not implemented | `501 ROUTE_NOT_MIGRATED` |
| `POST /api/admin/users/:id/{deactivate,activate,password,manager,class,parent}` | Not implemented with these route/method contracts | `501 ROUTE_NOT_MIGRATED`; Worker has a different `DELETE /api/admin/users/:id` path |
| `GET/PUT /api/admin/mobile-access` | Not implemented | Admin Settings reads/writes this route; both calls will fail against Worker |
| `PUT /api/admin/profiles`, profile import/export | Not implemented | `501 ROUTE_NOT_MIGRATED` |
| `POST /api/admin/policies`, `POST /api/admin/legal-holds` | Not implemented | Admin Settings directly calls both; policy creation and legal-hold application cannot work through Worker |
| `POST /api/todo/lists`, `/api/todo/targets`, `/api/todo/staff-loans` | Not implemented | `501 ROUTE_NOT_MIGRATED` |
| `GET /api/files/:content_hash`, push-subscription POST/DELETE, profiles GET | Implemented in source | Route presence only; authorization and runtime behavior still need contract tests |
| `GET /api/events` | Worker returns a JSON event list | **Incompatible:** reference service and clients use a WebSocket upgrade on this path |
| `POST /api/relay-ticket`, `GET /api/relay/:target_id` WebSocket | Not implemented | Desktop relay client cannot acquire a ticket or open its relay WebSocket against Worker |
| `GET /health`, `GET /ready` | Implemented in source | `/ready` uses the schema contract; direct endpoint invocation not completed in this audit session |

Unported methods deliberately return `501 ROUTE_NOT_MIGRATED`; this is predictable but still blocks client features. Do not mark an endpoint production-ready merely because the router fails explicitly.

## 4. Confirmed authorization and desktop/mobile blockers

1. **Client-controlled capability class:** Worker login copies request `client_type` into the session. `POST /api/command` only applies its observer denial when `client_type === 'mobile_observer'` and the session is not admin. A caller can choose a different client type, so this check is not an authorization boundary. Enforce a D1-backed server-side role/action/aggregate policy; do not trust `client_type`, and deny every unapproved command by default.
2. **Inactive organization check missing:** `requireSession` re-reads the user's active state, organization ID, and role, but does not verify that the associated `organizations.is_active` remains true on each protected request. Apply the organization-state check to protected requests and refresh.
3. **No complete command policy:** The generic command path can create/update/delete aggregates and persist domain events based on the submitted envelope. It lacks a complete explicit role × action × aggregate authorization matrix. Tenant equality alone does not authorize an operation.
4. **Hierarchy serialization mismatch:** Worker object envelope versus Android/Rust top-level array is a direct compatibility failure. Select one canonical schema and add cross-client contract tests before deploying a fix.
5. **Staff desktop production default:** Staff shell `Login.tsx` falls back to `http://127.0.0.1:3000`. A fresh install without a persisted server address therefore targets loopback instead of the production service.
6. **All-Father native OAuth callback:** Browser PKCE exists in `admin-shell/ui/src/auth/allfatherOAuth.ts` and tests exist. However, the callback is built from `window.location.origin + window.location.pathname` and its verifier/state are stored in that web context's `sessionStorage`. I found no Tauri deep-link callback handler in the current source search. This does not establish a working system-browser → native-shell return path; verify and implement the approved Tauri deep-link/platform callback before calling native All-Father login complete.
7. **Native-shell CORS origin:** Current Worker allowlist lists the two Pages origins. The top-level `applyCors` wrapper strips wildcard headers and only returns an allowlisted origin, so this audit **does not** treat the inner JSON helper's initial wildcard as an effective wildcard-CORS vulnerability. The packaged Tauri WebView origin has not been verified against the allowlist; test the actual origin on each target and add only the required exact origins.

## 5. Deployment and verification controls

- [Worker deployment workflow](https://github.com/SMozaff/Onyx-Framework/blob/bff822b464e9b95d80393c4b0f2545e0ed032263/.github/workflows/deploy-cloudflare-edge.yml) finds `onyx-free-db` by name and **creates a new database if it is not found**. It then writes the discovered UUID into a generated config. This is not safe for a pinned production service: a wrong account/name or missing DB can silently lead to a new empty DB. The production workflow must require UUID `7b757d0f-3484-48a5-9422-eadabed2a995` and fail before migrations/deploy if the ID is missing or different. Remove automatic production DB creation.
- Worker tests and local D1 migration tests run in [Worker CI](https://github.com/SMozaff/Onyx-Framework/blob/bff822b464e9b95d80393c4b0f2545e0ed032263/.github/workflows/cloudflare-worker-check.yml), but the production deployment workflow does not itself depend on that workflow. Make deployment require passing Worker auth/migration tests; `workflow_dispatch` must not bypass the gate.
- The deployment workflow contains live `/health`, `/ready`, and anonymous `GET /api/users/hierarchy = 401` checks. GitHub workflow success is useful deployment evidence, but this audit runtime could not directly access the public `/health` or `/ready` URLs, and Cloudflare's returned version metadata did not identify a source SHA. Attach the successful run/commit and a recorded live response to the acceptance evidence.
- The most recent Pages production deployment records previously inspected for `onyx-web-ui` and `onyx-mobile-pwa` were skipped for `path_config`; earlier deployments had succeeded. This alone does not prove either site is broken, but the live deployment/alias and client build SHA still need to be tied to the tested contract.

## 6. Required next gates (no production execution yet)

1. **Complete and test the contract** for Admin, Staff desktop, Android, Web/PWA and WebSocket routes. Resolve route/method/JSON-shape differences; add Worker contract tests for every client-consumed endpoint and negative authorization cases.
2. **Close security blockers** with server-authoritative role/action/aggregate rules, active-organization enforcement, fail-closed command allowlisting, and verified native CORS origin handling.
3. **Close All-Father bootstrap safely:** create/verify the designated Supabase Google identity through the provider flow, then bind the resulting immutable subject to the canonical D1 row through the guarded owner-controlled provisioning tool. Do not infer from email or mutate the mapping during login. Establish the first Org Admin only after the All-Father session is verified.
4. **Fix deployment guards in a reviewed PR:** pin the expected D1 UUID, remove auto-create behavior from the production workflow, and make test success a hard prerequisite to deploy.
5. **Verify actual deployed state:** record fresh direct `/health` and `/ready` response bodies/status, anonymous protected-route status, active Worker version/source SHA, and Pages deployment/alias SHAs. Then run an authorized end-to-end Supabase → Worker → D1 login with negative identity/tenant/role tests.
6. **Production change controls:** snapshot D1 before any write; preview/test migrations; preserve applied migration names; use forward-only repair; no production provisioning/configuration/migration/deploy until the responsible owner has approved that specific step.

## 7. Overall disposition

**NO-GO for All-Father production sign-in and full cross-client backend connectivity.** Positive findings: production D1 UUID is pinned here by direct observation, the deployed Worker binding matches it, the auth schema and canonical inert principal are present, the schema migration ledger is populated, expected runtime binding names exist, and the current source rejects unmapped All-Father identities without an implicit database write. Remaining blockers are the absent Supabase subject/user bootstrap, incomplete API surface and WebSocket relay, hierarchy JSON mismatch, unsafe command authorization, organization-state enforcement gap, native callback verification, and deployment fail-closed/test gating.

No production data, secrets, Worker configuration, migrations, identities, or deployments were changed while collecting this evidence.
