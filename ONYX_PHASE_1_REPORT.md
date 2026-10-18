# ONYX Phase 1 Report — Authentication, Authorization, and Sessions

**Repository:** `SMozaff/Onyx-Framework`  
**PR:** [#268](https://github.com/SMozaff/Onyx-Framework/pull/268)  
**Branch:** `security/phase-1-authz-sessions`  
**Status:** PHASE INCOMPLETE — the Worker defect is fixed and its CI path is green, but the full backend audit is not complete and an active-backend contract conflict requires an explicit architecture decision.

## 1. Baseline and exact revisions

- **Starting main/base SHA:** `54221e10f1dff46730aef33826db0c631c3d293d`.
- **Starting branch head from the prior handoff:** `24cbb960c41fd7bf78aa2c811c8986681c35e40b`.
- **Last observed code/test head while authoring this report:** `99e7ac19d71fabdad108aecfda34cfb24ce577f2`.
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
- **Passing Worker run:** [39137849084](https://github.com/SMozaff/Onyx-Framework/actions/runs/39137849084), SHA `99e7ac19d71fabdad108aecfda34cfb24ce577f2`:
  - bundle dry-run: passed;
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
- Both successful logs show the configured nested Worker root, the intended preview command, preview URL creation, and “Build completed.” The build for the final report-containing head must be checked separately.

### Full CI / Security

At the time this report was authored, full CI and Security workflows for the latest code head were still in progress. They must not be described as passed until their final exact-head runs finish. No local test command was run by this assistant; test and build counts above come from GitHub Actions logs.

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
