# ONYX Phase 1 Report — Authentication, Authorization, and Sessions

**Repository:** `SMozaff/Onyx-Framework`  
**PR:** [#268](https://github.com/SMozaff/Onyx-Framework/pull/268)  
**Branch:** `security/phase-1-authz-sessions`  
**Status:** PHASE INCOMPLETE — the Worker defect is fixed and its CI path is green, but the full backend audit is not complete and an active-backend contract conflict requires an explicit architecture decision.

## 1. Baseline and exact revisions

- **Starting main/base SHA:** `54221e10f1dff46730aef33826db0c631c3d293d`.
- **Starting branch head from the prior handoff:** `24cbb960c41fd7bf78aa2c811c8986681c35e40b`.
- **Last tested source/workflow head before this report-only evidence refresh:** `faac00ea430ef946ab3d8fc245ea753b3caa1969` (the report-only commit that follows is the PR head; this report cannot self-reference its own commit SHA).
- This report update creates a new report-only commit; the exact current PR head is recorded by PR #268 metadata and is stated again in the final handoff. The report commit cannot contain its own SHA.
- PR #268 remains open, draft, and unmerged. No production deployment, production D1 mutation, credential rotation, or production secret change was performed.

## 2. Relay-ticket failure — root cause confirmed and fixed

### Evidence

The failing Worker test was not merely a mock/schema problem. The failing path was:

1. `POST /api/relay-ticket` authenticated the token, revalidated the user and organization, and passed the server-managed mobile-capability check.
2. The handler persisted/checked replica ownership.
3. It attempted to create a relay-ticket claim with `exp: now() + RELAY_TICKET_TTL_SECONDS`.
4. The constant was referenced by both issuance and verification but was **not defined in the Worker**.
5. The resulting `ReferenceError: RELAY_TICKET_TTL_SECONDS is not defined` was caught by the handler and correctly mapped to `503 RELAY_TICKET_UNAVAILABLE`.

The direct evidence is GitHub Actions Worker CI run [39137682683](https://github.com/SMozaff/Onyx-Framework/actions/runs/39137682683), commit `ac23092269cc667bb8335ddba89a8638cbf823c9`; its actual test log prints the ReferenceError. The earlier relay test showed 151 passed / 1 failed, but the error was caused by a runtime defect, not by schema readiness.

### Fix and regression

- Added `const RELAY_TICKET_TTL_SECONDS = 30` to `deploy/cloudflare/edge-gateway/src/index.ts`. The 30-second TTL matches the explicit contract in the Rust relay implementation.
- Removed the temporary diagnostic logging used to establish the root cause; no error details are returned to clients.
- Updated `test/relay-route.test.ts` to assert `expires_in === 30` while preserving ticket issuance, forwarding, and replay-rejection assertions.
- Added a deterministic concurrent refresh-token rotation regression in `test/security.test.ts`: exactly one concurrent request may rotate the same token; the other must be rejected.

## 3. Cloudflare preview build — root cause confirmed and build fixed

### Original failure evidence

Cloudflare Workers Build `14e62fc3-a98a-40fa-bffd-592cc349224e`, for `24cbb960c41fd7bf78aa2c811c8986681c35e40b`, failed on 2026-10-18 at 14:38:52Z. The retrieved log shows:

- command: `npx wrangler preview`
- configured root directory: `/`
- error: Wrangler could not find the required `previews` configuration block.

The repository root does not contain the Worker project configuration. The nested `deploy/cloudflare/edge-gateway/wrangler.toml` does contain `[previews]`. This was a Cloudflare Build root/config selection error, not the relay-ticket defect and not a TypeScript compilation error.

### Targeted configuration change and observed results

Updated the Cloudflare Build configuration for the `security/phase-1-authz-sessions` branch to:

- root directory: `/deploy/cloudflare/edge-gateway`
- deploy command: `npx wrangler preview --config wrangler.toml`

This does **not** deploy production. Cloudflare Build `270fa199-fbec-4840-9b31-8d4eaec9647f` completed successfully for `6fd90265f6e6e39a5733b08dc884d0066aef3faa`, and build `d1590685-720f-440f-9ba2-541ab05db6e2` completed successfully for `154336a8fbff8a8a8dbf3a79b59bc5f65f823bf3`. Their logs confirm a preview URL and “Build completed.” These are preview-build results, not production runtime evidence. A preview build for the final report-containing head must still be checked.

## 4. Files changed and why

| File / setting | Change | Reason |
|---|---|---|
| `deploy/cloudflare/edge-gateway/src/index.ts` | Define 30-second relay-ticket TTL; remove temporary diagnostic log | Fix confirmed runtime ReferenceError without changing error semantics. |
| `deploy/cloudflare/edge-gateway/test/relay-route.test.ts` | Add D1 schema metadata behavior and assert 30-second ticket expiry | Preserve and strengthen ticket issuance, forwarding, and replay coverage. |
| `deploy/cloudflare/edge-gateway/test/security.test.ts` | Add concurrent refresh rotation regression | Test one-winner semantics for the same refresh token. |
| `deploy/cloudflare/edge-gateway/test/auth-phase1-regressions.test.ts` | Add password-hash and identity/role spoofing regressions | Verify fail-closed authentication and canonical All-Father rules. |
| `deploy/cloudflare/edge-gateway/test/route-completeness.test.ts` | Align route expectation with current ONYX-managed All-Father credentials | Avoid restoring the superseded Worker Supabase exchange route to satisfy a stale test. |
| `.github/workflows/cloudflare-worker-check.yml` | Add a separate `npx vitest run test/relay-route.test.ts` step before the full suite | Produce the required isolated relay-ticket regression result in CI. |
| Cloudflare Workers Build settings for the remediation branch | Correct root directory and preview command/config | Ensure the preview build loads the actual nested Worker configuration. |
| `ONYX_PHASE_1_REPORT.md` | Update evidence and unresolved audit items | Preserve reproducible evidence and avoid overstating completion. |

The Worker runtime change is narrowly scoped. The Rust/Axum API path was not changed because its All-Father authentication contract conflicts with the current Worker contract and needs an explicit architecture decision.

## 5. Worker route and trust-boundary audit

This table records source evidence plus the test coverage inspected; it is not a claim of production verification.

| Route / operation | Authentication and authorization evidence | Coverage / status |
|---|---|---|
| `GET /health` | Public liveness only; no identity authority is granted. | Route contract checked. |
| `GET /ready` | Public read-only schema-contract check; fails with 503 when required tables, columns, or indexes are missing. | Schema readiness tests and local migration CI pass on the code head. |
| `POST /api/auth/login` | Argon2id credential verification; generic invalid-credentials response; role and tenant are resolved from D1; canonical All-Father ID, role, and admin flag required. | Existing auth tests plus new malformed/missing hash and role-spoofing tests. |
| `POST /api/auth/refresh` | Refresh token type, signature, issuer/audience, expiry, revocation, active account/org and current canonical role are checked; token hash rotation uses `INSERT OR IGNORE`. | Replay and concurrent-rotation tests; current Worker suite passes. |
| `POST /api/auth/logout` | Revokes supplied valid token/session credentials; arbitrary strings do not become valid revocations. | Existing logout/revocation test. |
| `GET /api/query` | Access token required; tenant envelope must match verified session; Staff resource ownership predicate is applied. | Existing malformed/expired/issuer/audience, inactive user/org, cross-tenant and Staff ownership tests. |
| `POST /api/command` | Access token required; command/action policy and target tenant/ownership checks; client-provided actor/role/security metadata is not authority. | Existing role × command, unknown command, cross-tenant, ownership and metadata-spoofing tests. Full command-by-command matrix review remains a release gate. |
| `POST /api/relay-ticket` | Access token required; persisted mobile-capability policy, active user/org, UUID validation and first-claim replica ownership before minting a 30-second ticket. | Fixed undefined TTL; regression asserts 30 seconds and retains ownership/ticket flow. |
| `GET /api/relay/:target` | Separate relay-ticket credential; expiry, target binding, one-time redemption, tenant/user/self/target forwarding headers enforced. | Ticket replay assertion plus local Worker/RelayHub WebSocket integration in CI. |
| `GET /api/events` | Separate WebSocket bearer-subprotocol authentication; origin policy and tenant/resource filtering. | Existing WebSocket origin/auth and Staff filtering tests; live production behavior unverified. |
| `GET /api/users`, `GET /api/users/hierarchy` | Access session; active, same-organization identity listing and hierarchy rules. | Existing route-contract, tenant, and Rust/Android wire-shape tests. |
| `GET/POST /api/admin/users`, `GET /api/admin/users/:id`, activate/deactivate/password/manager/parent/class and `DELETE /api/admin/users/:id` | Current D1 role and tenant checks; Admin is restricted to own tenant and Staff management; sensitive state fields cannot be client-mutated. | Existing admin and authorization tests; per-handler matrix review remains incomplete. |
| `GET/PUT /api/admin/mobile-access` | All-Father/organization-admin boundary; organization-scoped policy, optimistic versioning, validation and audit. | Existing policy persistence, role, version, malformed-policy and storage-failure tests; migration verified locally in CI. |
| `POST /api/admin/policies`, `POST /api/admin/legal-holds` | Admin authorization; target tenant checked before persistence; audit and domain writes. | Existing policy/legal-hold denial, idempotency and cross-tenant tests. |
| `GET /api/profiles`, `GET /api/profiles/:ownerId`, `PUT /api/admin/profiles`, import/export | Access session; Staff own-profile restriction and Admin-only mutation paths. | Source and route registration reviewed; complete negative matrix not yet demonstrated. |
| `POST /api/todo/lists`, `/api/todo/targets`, `/api/todo/staff-loans` | Access session plus server-managed mobile capability and command/resource checks. | Existing command and mobile-capability tests; remaining handler matrix review required. |
| `POST/DELETE /api/push/subscriptions` | Access session; subscription ID cannot be reassigned to a different user/tenant. | Existing ownership regression. |
| `GET /api/files/:hash`, `POST/DELETE /api/admin/files/:hash/acl/...` | Access session; explicit tenant-scoped file ACL; Staff denied without grant. | Existing file ACL negative test and source checks. |
| `POST /api/allfather/organizations`, `POST /api/allfather/admins` | Canonical All-Father authority only; organization/admin creation is server-authorized and audited. | Source-level role gates; complete route-level denial matrix not yet demonstrated. |
| Unknown routes | Return a typed unavailable/not-found response; do not imply successful persistence. | Route-completeness test covers unknown paths and current login contract. |

### Schema and dependency failures

The Worker’s readiness check is read-only and default-deny. Relay-ticket persistence errors return 503; the confirmed missing TTL was a programming error that was incorrectly grouped into the same infrastructure response, and is now fixed. CI uses disposable local D1 and a real local Worker/RelayHub integration, not production data.

## 6. Alternate backend and client/server audit — unresolved P0 contract conflict

The repository contains a separate Rust/Axum API service, independently wired with its own `ApiState`, Ed25519 token codec, token-revocation store, user store, route middleware and deployment chart. It is not protected by the Cloudflare Worker’s `requireSession`.

Inspected Axum protected paths include:

- `/api/users`, `/api/users/hierarchy`
- `/api/admin/users` and user detail/activate/deactivate/password/manager/class/parent handlers
- `/api/admin/mobile-access`
- `/api/profiles`, `/api/profiles/:owner_id`, Admin profile upsert/import/export
- `/api/admin/policies`, `/api/admin/legal-holds`
- `/api/todo/lists`, `/api/todo/targets`, `/api/todo/staff-loans`
- `/api/auth/logout`, `/api/command`, `/api/query`, `/api/relay-ticket`
- `/api/files/:content_hash`, push-subscription endpoints

Axum’s `authenticate_request` middleware calls `authenticate_headers`; that validates the Ed25519 token, expiry/type and shared token/session revocation state. However, `authenticate_headers` itself does **not** re-fetch the principal from `UserStore` or compare the current organization on every protected request. Refresh does re-fetch the user, and individual handlers have narrower role/tenant checks, but those are not equivalent to a global current-account/current-membership check. An out-of-band user deactivation or tenant reassignment that does not also update the revocation watermark could leave a still-valid access token usable on some handlers. This is a confirmed source-level gap; it has not been runtime-tested in this recovery.

There is also a direct contract mismatch requiring a decision: Axum still registers `POST /api/auth/supabase` for All-Father, while the merged/current Worker contract is ONYX-managed All-Father credentials and the Phase 1 instructions prohibit restoring the superseded exchange route. The repository’s Axum deployment and route registration show this is a separate authentication entry point, not something the Worker automatically protects.

**Required decision before completing Phase 1:** confirm whether the deployed Axum API remains a supported production entry point. If yes, explicitly decide how its All-Father login/session format will be aligned with the canonical ONYX credential contract and authorize remediation of its per-request principal/tenant revalidation. If Axum is retired or not user-facing, identify the authoritative deployment evidence and remove/disable the obsolete route through an approved change. I did not guess which path is intended or silently change a second auth provider.

The Tauri desktop shell, Admin Shell, web UI, PWA and Android clients use independent local/session persistence and API transport code. They were inventoried through route/auth call sites, but a complete per-command Tauri authority audit and client-to-selected-backend mapping has not been completed. This is part of the remaining P0 scope, not claimed as passed.

## 7. Verification evidence and exact scope

### Worker CI

- **Failing baseline run:** [39136910881](https://github.com/SMozaff/Onyx-Framework/actions/runs/39136910881), SHA `24cbb960c41fd7bf78aa2c811c8986681c35e40b`: 151 passed / 1 failed; relay ticket returned 503.
- **Root-cause diagnostic run:** [39137682683](https://github.com/SMozaff/Onyx-Framework/actions/runs/39137682683), SHA `ac23092269cc667bb8335ddba89a8638cbf823c9`: 151 passed / 1 failed; actual log showed `ReferenceError: RELAY_TICKET_TTL_SECONDS is not defined`.
- **Passing Worker run on the pre-report source/workflow head:** [39138102100](https://github.com/SMozaff/Onyx-Framework/actions/runs/39138102100), SHA `faac00ea430ef946ab3d8fc245ea753b3caa1969`:
  - bundle dry-run: passed;
  - isolated `npx vitest run test/relay-route.test.ts`: **1 test passed, 0 failed**;
  - `npm test` / `vitest run`: **9 test files passed, 153 tests passed, 0 failed**;
  - disposable local D1 migrations: passed;
  - mobile-access migration verification: passed;
  - local Worker + real RelayHub WebSocket integration: passed.
- The relay-ticket test passed as part of the complete suite, but at this point **an independent relay-only CI command has not yet been recorded**. A dedicated `npx vitest run test/relay-route.test.ts` step is now configured in the Worker workflow; its result on the current exact head is pending.

### Cloudflare Builds

- Failed build `14e62fc3-a98a-40fa-bffd-592cc349224e`, SHA `24cbb960c41fd7bf78aa2c811c8986681c35e40b`: failed due root/config selection.
- Failed build `3749f3d7-3590-4dc1-99e8-5a0bc701d97f`, SHA `d8039de6ef2dd387a3418c8816c5d59bd712ed7d`: also captured the old root/command settings at build start, before the configuration update was in effect.
- Successful preview build `270fa199-fbec-4840-9b31-8d4eaec9647f`, SHA `6fd90265f6e6e39a5733b08dc884d0066aef3faa`.
- Successful preview build `d1590685-720f-440f-9ba2-541ab05db6e2`, SHA `154336a8fbff8a8a8dbf3a79b59bc5f65f823bf3`.
- Successful preview build `058c1094-c545-4d3a-91ff-0c681046ab7c`, SHA `faac00ea430ef946ab3d8fc245ea753b3caa1969`; log confirms preview URL creation and “Build completed.”
- Both successful logs show the configured nested Worker root, the intended preview command, preview URL creation, and “Build completed.” The build for the final report-containing head must be checked separately.

### Full CI / Security

At the time of this evidence refresh, full CI run #1695 and Security run #1237 for source/workflow SHA `faac00ea430ef946ab3d8fc245ea753b3caa1969` were still in progress (no failed jobs observed in the latest status snapshot). They must not be described as passed until completed. This report-only commit triggers new final-head runs; verify their outcomes separately. No local test command was run by this assistant; test and build counts above come from GitHub Actions logs.

## 8. Residual risks and required approvals

1. The separate Axum API has a confirmed source-level gap in global current-principal/current-tenant revalidation, and its All-Father Supabase exchange route conflicts with the current Worker contract. This is a P0 architectural/security blocker.
2. The full per-route authorization matrix for Axum, Tauri commands and each client-to-backend path is not complete.
3. Worker CI proves local test-double and local D1/Worker behavior only. It does not prove production D1 schema, production credentials, or production authentication.
4. Cloudflare preview build success is not production deployment or runtime verification. Preview Worker configuration does not prove production bindings/secrets are correct.
5. Full CI, Security and an isolated relay-only test on the final exact head must pass before considering the Worker portion complete.
6. No production configuration, data, secrets, or deployments were changed; the only Cloudflare change was the remediation branch’s Workers Build root/preview command.

## 9. Recommended next-phase prompt

> Continue from the exact current PR #268 head; do not treat Phase 1 as complete. First resolve the documented architecture decision about whether the separately deployed Rust/Axum API is a supported production entry point and how its All-Father login contract aligns with the current ONYX-managed Worker contract. Then implement and test per-request principal/tenant revalidation in every active backend; audit every Axum route, Tauri command, Admin Shell, web/PWA and Android path for server-side role, active-state, tenant and resource ownership enforcement. Preserve the current Worker relay-ticket TTL fix and all passing regression tests. Do not restore the superseded Worker `/api/auth/supabase` route, weaken authorization, deploy production, mutate production data/secrets, or merge. Record exact SHAs, isolated relay test, full Worker suite, full CI/Security, preview logs and any remaining P0 blocker.

## 10. Completion decision

**PHASE INCOMPLETE.** The Worker relay-ticket runtime defect is fixed, Worker CI and local integration checks passed on the recorded code head, and the Cloudflare preview configuration issue was fixed for successful subsequent builds. Phase 1 is not complete because the independent Axum backend contract/revalidation gap and the full alternate-path audit remain unresolved; final-head checks and the isolated relay-only run are also release gates.


---

# Phase 1 Continuation Addendum — Axum Boundary and Cross-Client Audit

**Continuation starting head:** `fdb86532e88c4e1b5a6de0eb6986d7e399d4f57c`  
**Starting base/main SHA verified:** `54221e10f1dff46730aef33826db0c631c3d293d`  
**Latest code/test head before this report refresh:** `7631040e2b3f66db45e85e7dbce50557420f0361` (this report refresh creates a new head).  
**PR state at start:** #268 open, draft, unmerged; base `main`; 18 commits ahead and 0 behind at initial comparison. The report update adds another commit.

## A. Axum production-role decision — evidence and limits

**Finding: treat Axum as a supported, production-configured backend unless the owner explicitly retires it.** This is based on deployment configuration, not merely Rust source existence:

- `render.yaml` declares `onyx-api-docker`, `runtime: docker`, `ONYX_ENV=production`, `autoDeploy: true`, health check `/ready`, and a managed PostgreSQL database. Its comments configure Supabase variables for All-Father.
- `deploy/helm/onyx-api/values-production.yaml` declares five replicas, production ingress host `api.onyx.example.com`, TLS, and autoscaling.
- `deploy/helm/onyx-api/templates/deployment.yaml` defines an API-server rollout, container port, probes, and environment from a secret.
- `.github/workflows/release.yml` builds the `api-server` binary and container images. Its comments also say the real production server is a Windows 10 office machine and that Linux/macOS native server binaries are not currently deployed.
- `docs/DECISIONS.md` describes Cloud-Only Client Connectivity and says browser/desktop/mobile clients use a deployed ONYX Cloud API. The Admin Shell source says it is a thin HTTP client that talks directly to `api-server`.
- The current `crates/bins/admin-shell/ui/src/pages/Login.tsx` posts username/password to `/api/auth/login`. The OAuth helper and registered Tauri `start_allfather_oauth` command remain in source, but repository search found no active UI call site invoking `beginAllfatherSignIn` or `start_allfather_oauth`. That is evidence of stale/unused in-repository wiring, **not** proof that no older external client calls the public endpoint.

These artifacts conflict about the *currently live* hosting path (Render/Helm versus the Windows-office deployment note). I do not have authorized live Render/Kubernetes deployment metadata in this task, so **actual live deployment is unverified**. However, the production manifests and supported client wiring are sufficient to classify Axum as production-configured and to require its auth boundary to be secured. Do not disable or remove it on an assumption.

## B. Authentication-contract conflict — decision required for client migration

The current `docs/SECURITY_AUTHENTICATION_MODEL.md` states that All-Father uses canonical ONYX-managed credentials and that Google/Supabase are not part of All-Father authentication. The current Axum route table still registers `POST /api/auth/supabase`; `render.yaml` injects its provider configuration; and Admin Shell code actively starts Google/Supabase OAuth and exchanges the provider bearer token for an ONYX session.

Therefore the inconsistency spans the API and a supported client. The correct destination contract is documented as ONYX-managed credentials, but retiring the existing provider exchange without first migrating the Admin Shell login UI would break the current caller. I have **not** silently removed the route, removed provider configuration, or changed the supported client’s login UX. Required owner/product decision: approve the coordinated Admin Shell login migration to ONYX username/password (and confirm whether any separately authorized provider use remains). After that migration, remove the All-Father Supabase exchange route and its exclusive configuration only through a tested, reviewable change. The Worker route was not restored.

## C. Confirmed Axum runtime fix

Changed `crates/bins/api-server/src/routes/mod.rs`:

- After cryptographic validation, expiry/type checks and token/session revocation checks, `authenticate_headers` now re-reads the subject from the authoritative `UserStore` on every access-token protected request.
- It rejects missing/inactive principals and rejects a token whose signed username or organization ID no longer matches the current stored user.
- It uses the current database identity in the returned `AuthenticatedUser`, not stale token identity fields.
- Added required JWT `iss=onyx-api-server` and `aud=onyx-api` claims and validate both before accepting Axum tokens. Relay tickets minted by Axum carry the same explicit issuer/audience and retain their distinct token-type/target checks. Refresh now also rejects a token when its current stored username or organization no longer matches the refresh claims. Because older Axum tokens lack issuer/audience claims, they now fail closed and users must sign in again after this change is deployed; this is an intentional session invalidation, not backward-compatible token acceptance.
- If the user store is unavailable, the request fails closed with generic `503 AUTHORIZATION_STATE_UNAVAILABLE`; no database error details are returned to the client.
- This function is called by the route-layer auth middleware and by handlers that explicitly authenticate headers, so it covers both common paths without adding route-specific duplicate checks.

Added `crates/bins/api-server/tests/session_revalidation.rs`, a real local HTTP + SQLite integration fixture with regressions for:
1. An already-issued signed access token after user deactivation.
2. An already-issued signed access token after the authoritative organization binding changes.
3. Admin authorization after the stored admin flag is revoked.
4. A correctly signed token with a wrong issuer.
5. A correctly signed token with a wrong audience.
6. A refresh token after the principal's organization binding changes.

These are code/test changes; their CI results must be taken from the current exact head’s completed workflow, not inferred from the test source.

## D. Organization active-state limitation — unresolved P0

The Axum `UserRecord` / `UserStore` contract currently has an active-user flag and organization ID, but no organization record or organization-active lookup. The API's checked-in SQLite/Postgres migration sets do not provide the same canonical `organizations.is_active` trust model used by the Worker’s D1 schema. Consequently, the new Axum check enforces current user active state and current tenant binding, but **cannot truthfully claim organization-active-state validation**.

Do not fix this by returning `true` on missing organization data or by inventing a schema assumption. The remaining implementation needs a deliberate Axum organization/membership model: schema migration(s), store port methods implemented for each database adapter, provisioning/backfill policy, and tests proving inactive/missing organization and membership fail closed. This is an unresolved P0 for a potentially active production backend.

## E. Cross-client / protected-route audit status

### Axum route groups inspected

The router in `crates/bins/api-server/src/routes/mod.rs` separates public auth/health/readiness routes from a protected router wrapped by `middleware::authenticate_request`. The protected route inventory includes:

- `/api/users`, `/api/users/hierarchy`
- `/api/admin/users`, `/api/admin/users/:id`, activate/deactivate/password/manager/class/parent
- `/api/admin/mobile-access`
- `/api/profiles`, `/api/profiles/:owner_id`, `/api/admin/profiles`, import/export
- `/api/admin/policies`, `/api/admin/legal-holds`
- `/api/todo/lists`, `/api/todo/targets`, `/api/todo/staff-loans`
- `/api/auth/logout`, `/api/command`, `/api/query`, `/api/relay-ticket`
- `/api/files/:content_hash`, push-subscription routes
- `/api/events` and `/api/relay/:target_id`, which use their own WebSocket/token or one-time ticket authentication path and must remain separately reviewed.

The shared protected-router middleware calls `authenticate_headers`; the fix therefore applies to the protected route group. The explicitly listed special WebSocket paths are not covered by that ordinary bearer middleware and require their own auth semantics. `/api/auth/refresh` uses refresh-token validation and a fresh user lookup; `/api/auth/supabase` remains a separate public identity-exchange route pending the coordinated contract migration.

### Findings / evidence limits by boundary

| Boundary | Verified from source | Remaining gap |
|---|---|---|
| Axum standard protected routes | Signature/type/expiry/revocation checks; now fresh user lookup, active check, username/org claim binding; common router middleware plus explicit handler calls. | No organization-active record in current store contract; not every individual handler has a completed negative authorization matrix. |
| Axum admin routes | `require_admin` re-fetches current `UserRecord` and checks active/admin flags; tenant-scoped lookup/update methods exist and are used by user-management paths. | Need exhaustive per-handler role, ownership and organization-state matrix and integration evidence. |
| Axum query/command | Query compares envelope organization to authenticated organization; command and query handlers perform additional checks. | Need complete command-by-command spoofing/target/ownership review, including every dispatched aggregate and payload actor field. |
| Axum events/relay | Dedicated WebSocket and relay-ticket auth paths exist separately from ordinary access middleware. | Their token binding, target binding, one-time use, expiry and tenant/resource rules need individual negative tests on Axum. |
| Admin Shell | Actual OAuth callback helper, Supabase flow, and HTTP client to API server identified. | Client migration to the documented ONYX credential contract is not implemented; Tauri loopback callback/PKCE lifecycle needs its own review. |
| Web UI / PWA | API client interceptors preserve explicit provider credentials; UI strings describe Google/Supabase All-Father sign-in. | Client-to-deployment mapping and coordinated contract migration are unresolved. |
| Desktop Shell | Repository documentation describes a separate local-first command path via `client-composition`, not the Admin Shell HTTP route. | Full Tauri command audit and whether any local command path can mutate protected data without server authorization remain incomplete. |
| Android/mobile | Uses API/Worker clients and client capability metadata. | Full endpoint-by-endpoint server authorization mapping remains incomplete; client metadata is not itself trusted authority. |
| Cloudflare Worker | Existing Phase 1 report documents the relay TTL fix and 153-test suite on earlier tested SHA; current-head Worker workflow must be refreshed. | Worker tests do not prove Axum security or live production state. |

## F. Verification status for this continuation

- Starting branch `fdb86532e88c4e1b5a6de0eb6986d7e399d4f57c`; latest code/test commit before this report refresh: `7631040e2b3f66db45e85e7dbce50557420f0361`.
- Current branch commits include runtime principal revalidation, explicit Axum JWT issuer/audience validation, relay-ticket issuer/audience binding, refresh tenant-change rejection, and a six-case HTTP/SQLite integration test.
- The previous Worker CI #303 passed on `fdb8653…`; the previous Cloudflare preview build succeeded on that same earlier head. New Worker CI and Cloudflare preview runs are triggered for this continuation and must be checked by exact SHA.
- The previous Security run #1238 passed on `fdb8653…`, but is not evidence for the new Rust changes.
- At this point, the current-head full CI / Security / Worker / preview outcomes are pending or running. No current-head Rust test, formatting, lint, or full-suite pass is claimed until the completed logs are read. This assistant did not execute a local checkout/working-tree test run.
- No production deployment, production database mutation, credential rotation, or production secret change was performed.

## G. Required next steps and recommendation

1. Run `cargo fmt --check`, the targeted `api-server` integration test `session_revalidation`, relevant existing auth/tenant tests, and the repository-required Clippy/build commands through CI; inspect logs and fix any compile/test regressions.
2. Verify full CI, Security, Worker CI and Cloudflare preview on the final exact head after the report commit.
3. Obtain owner approval for coordinated Admin Shell migration away from the legacy All-Father Supabase exchange, then implement client + Axum contract changes together; do not leave the API route in place solely for compatibility.
4. Design and implement the Axum organization/membership active-state store contract with migrations and fail-closed tests.
5. Complete route-by-route authorization matrices for Axum special WebSockets/relay, commands, Admin Shell/Tauri, web/PWA, Desktop Shell and Android before release.

**Continuation decision: PHASE INCOMPLETE.** The API is production-configured and must be treated as an in-scope protected entry point. The stale-principal and missing issuer/audience checks have targeted fixes and integration regressions, but their new CI evidence is pending, the organization-active-state model is absent, and the All-Father client/API contract migration plus the full cross-client audit remain outstanding.


---

## H. Phase 1 continuation — Clippy root cause and special-entry-point hardening

**Continuation starting head:** `ee8c2bbf5ec8ba56811844adfdff15d791144398`  
**Base/main:** `54221e10f1dff46730aef33826db0c631c3d293d`

### H.1 Exact Clippy failure and fix

Full CI run [39139582550](https://github.com/SMozaff/Onyx-Framework/actions/runs/39139582550), job `check` / job ID `117711145542`, failed in the `Clippy` step while compiling `api-server` integration target `session_revalidation`.

The compiler diagnostic was exact: `crates/bins/api-server/tests/session_revalidation.rs:11` imported `SecretProvider` but never used. The workspace treats warnings as errors (`-D warnings`), so this unused import prevented compilation. It was not a missing trait implementation or a runtime-test failure.

The smallest correction removed only `SecretProvider` from `use security_application::{NewUser, SecretProvider};`. Commit: [`9efc58f0fc6f3ac2269160cd22955485de26e2eb`](https://github.com/SMozaff/Onyx-Framework/commit/9efc58f0fc6f3ac2269160cd22955485de26e2eb). On that exact SHA, Full CI reported **Format passed** and **Clippy passed**; Build and tests had not completed when the subsequent special-entry-point work began. Worker CI passed on that SHA and Security was still running at the last check. These results are not treated as evidence for later SHAs.

### H.2 Additional Axum WebSocket and relay findings/fixes

Source inspection confirmed that `/api/events` used `validate_token` directly and then trusted `claims.organization_id`, bypassing the current-user revalidation performed by ordinary HTTP authentication. Relay-ticket redemption likewise validated signature, issuer/audience, expiry, type, revocation, target binding, and one-time use, but did not re-resolve the current principal before the WebSocket upgrade.

The branch now extracts the current-principal lookup into `revalidate_token_principal` and invokes it from:
- the ordinary protected HTTP authentication path;
- the `/api/events` WebSocket handshake; and
- relay-ticket redemption before the relay WebSocket upgrade.

The integration target adds regressions that deactivate a user after access-token issuance and after relay-ticket issuance, then expect the corresponding WebSocket handshake/redemption to return HTTP 401. These are code changes; the final exact-head workflow result must be read before they are considered verified. This check covers the handshake boundary only; an already-established WebSocket is not actively disconnected by a distributed account-deactivation event, and the intended live-connection revocation SLA remains to be specified.

### H.3 Organization and membership authority — blocked architecture decision

The current Axum `UserStore` contract and both adapters model a user with one `organization_id`, `is_active`, user-level `is_admin`, and class/parent fields. The port has no organization lookup, organization-active state, or membership lookup. The checked-in Axum SQLite/PostgreSQL model therefore cannot currently distinguish:
- a valid active organization from a missing or inactive organization; or
- an active membership from a missing or revoked membership.

The repository documents a one-organization-per-user shape for the existing Axum identity record, while the requested contract explicitly requires authoritative membership records. Introducing a table without resolving whether membership is a single row per user or a multi-organization relation—and whether role/class is user-level or membership-scoped—would invent a product authorization model.

**Required owner decision before implementing the schema/port migration:**
1. Confirm whether an ONYX principal can belong to more than one organization.
2. Confirm whether role/admin/class authority belongs to the principal or to each organization membership.
3. Confirm whether the migration must preserve the existing one-organization model or normalize it into membership rows.
4. Approve an existing-data inventory and activation policy. No existing organization may be marked active merely because a user row contains its ID. If an approved backfill cannot prove organization status, the safe migration must leave the record inactive and deny access until explicit provisioning.
5. Define rollback behavior and the operational plan for users denied while organizations/memberships are absent or inactive.

Until those decisions exist, no organization/membership schema was added and no database was migrated. Missing organization or membership cannot be represented as authorized by a default-true fallback.

### H.4 All-Father authentication contract — external-usage decision required

The current Admin Shell login page `crates/bins/admin-shell/ui/src/pages/Login.tsx` posts ONYX username/password to `POST /api/auth/login`. The legacy `crates/bins/admin-shell/ui/src/auth/allfatherOAuth.ts`, the Tauri `start_allfather_oauth` command, the Supabase configuration, and Axum `POST /api/auth/supabase` remain in source. The visible current login page does not invoke the OAuth helper, and repository search did not find a direct call to `start_allfather_oauth`. This establishes the in-repository login screen's current behavior; it does **not** establish that no deployed/third-party client uses the public Axum route.

The documented ONYX-managed All-Father target and the legacy Axum Supabase exchange are not yet reconciled. The Axum exchange verifies a designated Supabase subject and maps it to a configured UUID principal with username `allfather` and admin status; it does not use the Worker contract's canonical `__allfather__` principal plus explicit `ALL_FATHER` role. Axum's `UserRecord` has no explicit role field. The Worker Supabase route must not be restored.

**Required owner decision:** confirm whether any deployed or externally supported caller uses Axum `POST /api/auth/supabase` or the Tauri OAuth command, and whether Supabase has any supported purpose beyond that All-Father login. Then approve one coordinated outcome:
- migrate All-Father to ONYX-managed credentials and a canonical identity/role representation, update all supported clients and provisioning, and retire the Axum exchange/OAuth command only after compatibility is verified; or
- explicitly retain a separate Supabase identity boundary, document its approved scope and canonical ONYX mapping, and reconcile it with the ONYX-managed-credentials requirement.

Do not silently remove the route, assume external callers do not exist, or provision a production identity from guessed data.

### H.5 Cross-client authorization entry-point matrix

| Entry point | Authoritative checks evidenced in source | Negative coverage / remaining gap |
|---|---|---|
| Axum ordinary protected HTTP routes | JWT signature, issuer, audience, expiry, type, token/session revocation; fresh user-store lookup; account active; signed username and tenant binding match current record. | No authoritative organization-active or membership lookup. Not every handler has a completed per-operation role/ownership matrix. |
| Axum login and refresh | ONYX password-hash verification; refresh token validation/rotation; current user lookup and username/tenant binding on refresh. | Organization/membership policy missing. All-Father Supabase route conflicts with the target contract. |
| Axum admin/user-management routes | Current admin flag is re-read by route guards; tenant-scoped user lookup/update methods are present. | Complete route-by-route role, target ownership, org-state and membership-negative test matrix remains outstanding. |
| Axum `GET /api/query` | Ordinary current-principal authentication; envelope organization must equal authenticated organization. | Each query's resource ownership predicate and payload-derived target must be independently mapped and negatively tested. |
| Axum `POST /api/command` | Ordinary current-principal authentication and command dispatch-specific checks. | Exhaustive command-by-command review of actor fields, target resource, ownership, role/class, tenant, and mutation capability is incomplete; client-supplied actor/role metadata must never be authoritative. |
| Axum `GET /api/events` WebSocket | Token issuer/audience/expiry/type/revocation plus current-principal revalidation at handshake after this change; subscription tenant is bound to the verified principal. | New inactive-account handshake regression awaits exact-head CI. Existing sockets are not actively terminated on later revocation; semantics/SLA remain undefined. |
| Axum `POST /api/relay-ticket` and `GET /api/relay/:target_id` | Issuance uses authenticated principal, server-side client capability and replica ownership; redemption checks ticket signature, issuer/audience/expiry/type/revocation, target, self-replica binding and single use, plus current-principal revalidation after this change. | New deactivation-after-issuance regression awaits exact-head CI. Organization/membership state is unavailable. Live-connection revocation semantics remain undefined. |
| Query/command dispatch and aggregate resources | Query envelope tenant binding and existing route/aggregate guards are present. | No completed exhaustive dispatch matrix; tests must cover spoofed actor IDs, target ownership, cross-tenant reads/writes, role revocation, and reassignment for every command family. |
| Admin Shell | Current login UI calls `/api/auth/login` with ONYX username/password; HTTP client carries session credentials. | Legacy OAuth helper/Tauri command still exists. No proof that external callers are absent; canonical All-Father migration is blocked on owner decision. |
| Tauri / Desktop Shell local and remote execution | Admin Shell is documented as a thin HTTP client; Desktop Shell has a separate local-first `client-composition` path. | Full Tauri command and local mutation audit is incomplete. Local UI/command permissions cannot replace server authorization for shared/protected data. |
| Web UI / PWA | First-party clients send a declared client type; API auth is server-validated. | Full endpoint-to-handler matrix and deployed backend mapping are incomplete. Client-provided capability metadata is only an input to server policy, never proof of authorization. |
| Android/mobile | Client type/capability metadata exists and server-side capability ceilings are implemented for relevant API routes. | Full endpoint-by-endpoint server mapping, stale-session/deactivation/membership/reassignment regressions, and client-to-backend deployment mapping remain incomplete. |
| Cloudflare Worker | Its independent D1 model has organization active state, role and user state; its CI suite is separate from Axum. | Worker green does not establish Axum authorization or live production behavior. Do not assume the two identity schemas are interchangeable. |

### H.6 Verification ledger and current status

- Starting SHA for this continuation: `ee8c2bbf5ec8ba56811844adfdff15d791144398`.
- Clippy-fix SHA: `9efc58f0fc6f3ac2269160cd22955485de26e2eb`; exact Full CI Format and Clippy passed, but that SHA is not the final candidate.
- Latest source candidate before the report update: `af63721d4dd5c4f913147525ca3bd5168a345cf5`; the report update itself was committed at `8e62ddc45844cef286b6bc246cd769e27f3d6698`.
- Worker CI [run 39144256137](https://github.com/SMozaff/Onyx-Framework/actions/runs/39144256137) passed on `af63721d4dd5c4f913147525ca3bd5168a345cf5).
- Security [run 39144256212](https://github.com/SMozaff/Onyx-Framework/actions/runs/39144256212) and Full CI [run 39144256232](https://github.com/SMozaff/Onyx-Framework/actions/runs/39144256232) were queued when this section was authored. The new WebSocket and relay integration tests therefore have no pass claim yet.
- The earlier Cloudflare preview build `27bd0c31-9116-47ff-87ed-ecf8741510c0` is for the earlier `ee8c2bbf5ec8ba56811844adfdff15d791144398` candidate, not this continuation's final source. It is preview-build evidence, not production runtime evidence.
- The report-containing commit necessarily has a new SHA; its own CI/Security/Worker status must be read from the checks associated with the actual PR head. Results on any earlier SHA are not proof for that final candidate.
- No production deployment, production database migration/data mutation, production secret change, or credential rotation was performed. PR #268 must remain open and draft.

**Decision: BLOCKED.** The Clippy defect was identified precisely and corrected; special WebSocket and relay-ticket handshake revalidation was added with regression tests. Phase 1 cannot be declared complete because the authoritative organization/membership schema and backfill/activation policy, the All-Father legacy-route compatibility decision, the complete cross-client authorization matrix, and exact-final-head verification remain unresolved.


### H.7 Cross-client tenant-reassignment regression

The Cloudflare Worker already re-resolves its D1 user and organization state on protected requests. Added a focused regression to `deploy/cloudflare/edge-gateway/test/security.test.ts`: it signs an access token for a user in organization `org-1`, changes the authoritative user row to `org-2`, and verifies that a query using the old token receives HTTP 401 with no writes. This test does not create or modify production data; it runs against the disposable in-memory security test double.

The Worker test addition and this report update are committed together so the exact report-containing candidate also triggers Worker CI. At commit time, Full CI, Security, and Worker CI results for that candidate must be checked from the exact PR-head workflow runs; no result from a parent SHA is treated as proof for the final candidate.


### H.8 Explicit isolated integration-test gate

Added a dedicated CI step to run `cargo test --package api-server --release --test session_revalidation --features test-endpoints -- --test-threads=1` before the workspace-wide test invocation. This provides a separately identifiable result for the session-revalidation HTTP/WebSocket regressions rather than relying only on the aggregate workspace test. The step must pass on the exact report-containing PR head; no pass is claimed until the run log confirms it.
