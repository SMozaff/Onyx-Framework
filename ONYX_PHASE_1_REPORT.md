# ONYX Phase 1 Report — Authentication, Authorization, and Sessions

**Repository:** SMozaff/Onyx-Framework  
**Remediation branch:** security/phase-1-authz-sessions  
**Status:** Incomplete — no production deployment, migration, secret change, or merge performed.

## 1. Baseline and commits

- **Observed main candidate SHA:** 54221e10f1dff46730aef33826db0c631c3d293d (merge commit for PR #267, All-Father ONYX credential login).
- **Branch created from:** 54221e10f1dff46730aef33826db0c631c3d293d.
- **Remediation commits so far:**
  - 348714c968bbbb5009ea55ad19fee9a707e4021c — add Phase 1 authentication trust-boundary regression tests.
  - 3b807bec5228767b5021e5f3b9f9c93f5df97cb5 — correct the schema-index mock used by those tests.
- **Final SHA:** pending further implementation and CI.
- The GitHub connector exposed recent commit history and file contents for this main candidate, but did not expose an independent direct refs/heads/main SHA read. Reconfirm the main ref before final acceptance.

## 2. Phase 0 inputs and source review

Reviewed the current Worker implementation and existing repair records, including:

- deploy/cloudflare/edge-gateway/src/index.ts
- deploy/cloudflare/edge-gateway/src/authorization.ts
- deploy/cloudflare/edge-gateway/src/schema-contract.ts
- deploy/cloudflare/edge-gateway/test/auth.test.ts
- docs/SECURITY_AUTHENTICATION_MODEL.md
- reports/auth-repair/01-baseline.md
- reports/auth-repair/02-auth-contract.md

The baseline contract states that D1 is authoritative for ONYX principal identity, role, active state, organization membership, and authorization. The current main candidate uses ONYX-managed credentials for All-Father login, and the canonical principal is id='__allfather__', with explicit ALL_FATHER role and admin flag. This supersedes the older Supabase-only identity-exchange design for the All-Father login path in the current Worker.

## 3. Files changed

| File | Change | Purpose |
|---|---|---|
| deploy/cloudflare/edge-gateway/test/auth-phase1-regressions.test.ts | Added | Exercises malformed/missing password-hash failure, client metadata privilege-spoofing resistance, and rejection of an ALL_FATHER role on a non-canonical principal. |
| ONYX_PHASE_1_REPORT.md | Added | Records evidence, test status, gaps, and Phase 2 recommendations. |

**No runtime authentication or authorization implementation has been changed yet.** The added tests are not a substitute for fixing any newly confirmed runtime defect.

## 4. Security evidence observed in source

| Requirement | Source evidence | Current assessment |
|---|---|---|
| Principal derived from verified credentials and server-side account data | Worker login queries D1 and verifies the stored Argon2id hash; response/session role is derived from the D1 row. | Present in source; test coverage is being expanded. |
| Canonical All-Father identity | Login checks canonical principal ID, explicit role, and admin flag; protected-request and refresh checks revalidate the role/principal relationship. | Present in source; must be verified by tests and complete route audit. |
| Account and organization state | Login and refresh join the account to an active organization; protected-request session revalidation joins the active organization and active user. | Present in inspected handlers; every route still requires route-level verification. |
| Password hash failures | verifyPassword rejects hashes without the Argon2id prefix and catches verifier failures; login emits a generic INVALID_CREDENTIALS response. | Fail-closed behavior present in source; new regression tests added, execution pending. |
| Session signature, issuer, audience, expiry, revocation | jwtVerify verifies HS256 signature, issuer/audience, required claims, expiry, and token/session revocation. Protected requests require access-token type. | Present in source; full lifecycle tests remain pending. |
| Refresh rotation and replay | Refresh verifies current account/role/organization and inserts the old refresh-token hash with INSERT OR IGNORE; it rejects if no row was inserted. | Rotation/replay rejection is present in source; race and session-family semantics still need explicit tests and review. |
| Tenant/resource ownership | Worker has centralized command/query policy helpers and resource ownership checks, plus route-specific scoping in handlers. | Not yet proven complete across every alternate route/handler; high-priority audit gap. |
| Client metadata spoofing | Session identity and role are built from the database row, not body-supplied id, sub, role, is_admin, or organization_id. | New regression test added; execution pending. |
| Credential leakage | Generic login credential errors and generic refresh errors are used in the inspected handlers. | Broader logs/telemetry/response scan not yet complete. |
| API compatibility | Current Worker contract documents POST /api/auth/login, /refresh, and /logout; All-Father uses ONYX-managed credentials on the current candidate. | Client compatibility and alternate Axum/API entry points still need review. |

These are source observations, not claims that the requirements are fully implemented or production-verified.

## 5. Commands and tests

- Repository/source inspection was performed through GitHub connector file and commit operations.
- No local build, test, or dependency download was performed.
- **Tests executed:** 0 in this phase so far.
- **Test count/results:** not yet available; no test has been reported as passing.
- **CI checks:** pending a draft PR and GitHub Actions execution against the exact branch head.
- **Skipped/unverified:** live Cloudflare Worker behavior, production D1 state, live credentials, session lifecycle end-to-end, Axum alternate entry points, and full cross-tenant/resource matrix.

## 6. Known gaps and residual risks

1. Runtime implementation has not yet been changed; the branch currently adds regression tests and this report only.
2. The new tests must run in GitHub Actions and their test double must be reconciled with the actual D1 schema contract.
3. Full route-by-route authorization review is still required for every admin endpoint, command handler, resource lookup, and alternate API entry point.
4. Session lifecycle needs explicit tests for malformed/expired/revoked tokens, inactive accounts and organizations, refresh rotation races/replay, logout, and session revocation.
5. Cross-tenant and cross-resource negative tests are not yet implemented or executed.
6. The accepted Phase 0 report should be confirmed by its canonical artifact/acceptance status; this report used the current auth-repair baseline and contract documents as available evidence.
7. The exact current main ref SHA needs independent confirmation before final acceptance.
8. No production configuration, credentials, data, or deployment has been changed.

## 7. Compatibility changes

No runtime/API contract changes have been made in this branch. The current candidate's ONYX-managed All-Father login behavior is treated as the active contract because it is reflected in merged PR #267 and the current Worker documentation. Do not restore the superseded Supabase exchange path without an explicit architecture decision.

## 8. Recommended Phase 2 prompt

> Audit and remediate every remaining legacy authentication and authorization path in SMozaff/Onyx-Framework from the verified current main SHA. Use the accepted Phase 0 report and Phase 1 findings as inputs. Inventory all Worker, Axum, desktop, web, and mobile routes; prove each protected operation has server-side identity, role, account-state, tenant, and resource-ownership checks; add executable negative tests for every alternate entry point; test session refresh replay/races and logout/revocation; reconcile client contracts without weakening security. Do not deploy, mutate production data/secrets, or merge. Report exact commit SHAs and only observed CI/test results.

## 9. Completion decision

**PHASE INCOMPLETE.** Regression tests and evidence documentation have been added to the remediation branch, but runtime fixes and executed verification are still required. No production security claim is made.
