# B-01 Worker Client — Wave 0 Final Report

**Status: PARTIAL — audit and draft preparation only.**  
**Gate 0b: NOT APPROVED. Production implementation is not authorized.**

## Repository baseline
- Repository: [SMozaff/Onyx-Framework](https://github.com/SMozaff/Onyx-Framework)
- Base branch: `main`
- Audited HEAD: `b056c6ef204080446e84b31f3d35a226e6a40867`
- Worker source: `deploy/cloudflare/edge-gateway/src/index.ts`
- No production code was changed and no live Worker calls or production mutations were performed.

## Findings
1. Current Admin call sites exercise login/refresh; user listing and mutations; profile list/upsert/import/export; mobile-access read/write; policy/legal-hold creation; query/command flows; and organization/admin management.
2. The checked-in API contract carries an older Worker source stamp than the source audited at the baseline SHA. C-01 must be reconciled before freezing.
3. `POST /api/allfather/admins` returns `is_admin` and `role` but does not return `created_at` or `updated_at`, while the current UI row type requires those timestamps. B-10 and the contract owner must decide the canonical DTO; the client must not synthesize fields.
4. Worker error responses are heterogeneous: string `error`, optional `code`, `category`, `retryability`, safe metadata, and structured command errors. A single error parser must preserve these distinctions and avoid surfacing raw unsafe payloads.
5. Policy/legal-hold creation uses idempotency keys; mobile-access writes use `expected_version`. Do not assume generic idempotency or retry safety for other mutations.
6. The proposed `src/client/*.rs` paths do not exist at repository root or as exact suffix matches. `crates/bins/admin-slint/src/` is only a possible future anchor, not an approved destination.
7. Existing Worker and Admin UI test sources were inspected, not executed in this task.

## CI and test evidence
- Root CI run [40336191173](https://github.com/SMozaff/Onyx-Framework/actions/runs/40336191173): **FAIL** — security license/source-policy audit failed; merge gate failed as a consequence.
- Admin Slint scaffold run [40336190967](https://github.com/SMozaff/Onyx-Framework/actions/runs/40336190967): **PASS** for the scaffold workflow on Ubuntu/Windows only.
- Local Worker tests/check, Admin UI tests, and Rust build/tests: **NOT RUN**. The available environment had Node 22/npm 10, no `rustc`/Cargo; Worker CI specifies Node 24 and the repo pins Rust 1.97.1.
- No current-main dedicated Worker CI pass was verified.

## Deliverables
This directory contains the seven requested Wave 0 reports, README, and draft fixture candidates. All contract and fixture materials are drafts, not frozen specifications. Candidate fixtures are synthetic and must be validated against the approved C-09 schema before being treated as tests.

## Blockers and required decisions
- Gate 0b authorization and integration coordinator sign-off.
- Confirm the owning crate/path for the proposed Rust client modules.
- Refresh/reconcile C-01 source metadata and decide the organization-admin-create DTO.
- Obtain B-04 approval for C-03 error-to-localization mapping and B-13 approval for C-09 fixture schema/version.
- Resolve B-02 Q-01 session/token lifecycle, B-08 Q-05 membership representation, and B-09 U-04 policy state transitions.
- Resolve the required CI license-audit failure and obtain a fresh green required-check run.

## Cross-team dependencies
B-02 Session & Secrets; B-04 Localization; B-05 Login; B-07 Users; B-08 Profiles; B-09 Policy & Settings; B-10 Organizations; B-13 Verification & CI; repository owner/integration coordinator; security/CI maintainers.

## Next actions
1. Integration coordinator confirms the crate anchor and contract/document ownership.
2. B-01/B-10 settle the organization-admin response DTO; update C-01 source evidence.
3. B-01/B-04 approve the error mapping; B-13 freezes C-09 schema and validates fixtures.
4. Close Gate 0b decisions and restore green required CI.
5. Only after formal authorization, implement the approved client and contract tests in a separate bounded implementation PR.

**Gate status: CLOSED / NOT APPROVED. No production client implementation is included in this documentation-only change.**
