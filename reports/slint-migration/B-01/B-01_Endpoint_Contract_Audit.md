# B-01 Endpoint / Contract Audit — Wave 0

**Status: DRAFT — NOT FROZEN.** Source-level audit only; no production implementation authorized.  
**Baseline:** `main` at `b056c6ef204080446e84b31f3d35a226e6a40867`.

## Observed operation groups

| Group | Operations observed in current Admin call sites | Contract checks before implementation |
|---|---|---|
| Authentication | Login, refresh | Required response fields, refresh rotation/replay behavior, invalid/inactive session handling; B-02/B-05 approval |
| Users | List, create, class/parent/status updates, All-Father deactivation | Role/tenant authorization, required fields, enum/range validation, response DTOs |
| Profiles | List, upsert, import, export | Conditional fields, role-aware DTOs, CSV/JSON shapes, partial row failures, content type/filename |
| Mobile access | Read and versioned update | `expected_version`, conflict response/current version, no blind retry |
| Policy/legal hold | Create operations with idempotency keys | Key format, same-key replay semantics, changed-payload conflict, retry only with same key after ambiguous failure |
| Query/command | Query and command/detail/action flows | Target/version/epoch fields, nested command error shape, `success:false` is never success |
| Organizations/admins | List/create/rename/status/reactivate; admin list/create | All-Father-only boundaries, DTO consistency, idempotency not assumed |

This inventory is limited to observed source call sites. It does not assert that every possible Worker route is needed by B-01.

## Confirmed mismatch requiring decision

`POST /api/allfather/admins` returns `is_admin` and `role` without `created_at` or `updated_at`; the current UI `OrganizationAdminRow` expects those timestamps and does not declare the other fields. B-10 and the contract owner must choose the canonical DTO. Do not invent timestamps or silently discard required response fields.

## Error and retry audit

Worker error envelopes are not uniform. Preserve optional code/category/retryability and structured command errors where present; validate content type and body shape; never expose raw unsafe payloads or credentials. Distinguish HTTP failure from application-level command failure. Retry only when operation-specific source/contract semantics explicitly make it safe. Existing evidence supports idempotency keys for policy/legal-hold creation and `expected_version` for mobile-access updates, not a generic mutation retry policy.

## Baseline freshness

The checked-in API contract contains an older Worker source stamp (`38c8975fd6616376d51e016b3e718b7736888514`) than the audited Worker source blob at this baseline (`f587f240c51ce84ec6d19db3b4de5f1a944e2e3f`). Refresh or explicitly reconcile this stamp before C-01 approval.

## Test status

Existing route-presence and Worker auth/authorization test files were observed, not run. The route-presence test alone does not prove request/response schemas, error envelopes, authorization matrix, timeout handling or retry safety. Required additions are listed in `B-01_Test_Coverage_and_Gaps.md`.