# C-01 Worker API Contract — Draft Proposal

**State: DRAFT — NOT FROZEN.** This is a source-derived proposal, not an approved canonical contract.  
**Audited baseline:** `main` / `b056c6ef204080446e84b31f3d35a226e6a40867`.

## Contract rules

1. Use the method, path, validation, authorization and DTO observed in current Worker source and current Admin call sites; do not infer new endpoints.
2. Treat Worker validation and authorization as authoritative. UI state is not an authorization boundary.
3. Model nullable/conditional fields exactly; do not synthesize timestamps or defaults not returned by the Worker.
4. Keep transport errors, HTTP errors, schema/decoding failures and application-level command failures distinct.
5. Mutation retries are operation-specific. No generic retry of create/update operations.
6. Redact credentials, tokens, raw request/response bodies and sensitive identifiers from logs and error presentation.

## In-scope operation families

- Login and refresh.
- User list/create and observed class, parent, status and All-Father deactivation operations.
- Profile list/upsert/import/export.
- Mobile-access GET/PUT with version checking.
- Policy/legal-hold creation with idempotency-key handling.
- Existing query/command/detail/action calls.
- Organization and organization-admin list/create/rename/status/reactivation flows.

Exact route strings, body schemas, DTO fields, status codes and authorization predicates must be transcribed from the audited Worker handlers into the final contract table and checked against each caller before implementation. Do not treat this summary as sufficient wire-schema approval.

## Required open decisions

### C-01-D1: source stamp
Existing `docs/production/api-contract.md` embeds Worker source SHA `38c8975fd6616376d51e016b3e718b7736888514`; audited source at this baseline has blob SHA `f587f240c51ce84ec6d19db3b4de5f1a944e2e3f`. Refresh or explicitly justify the old stamp.

### C-01-D2: organization-admin create DTO
Worker response has `is_admin` and `role`; it does not contain `created_at`/`updated_at`. Existing UI `OrganizationAdminRow` requires the timestamps and omits the returned fields. B-10 and contract owner must select the canonical response shape. No fabricated timestamps.

### C-01-D3: error union
Worker uses multiple error shapes, including string `error`, optional `code`, `category`, `retryability`, safe metadata and structured command errors. Coordinate parser and localization keys with C-03/B-04.

### C-01-D4: fixture schema
C-09 fixture version and required per-route/error coverage are owned by B-13. Candidate fixtures remain drafts until schema approval and validation.

## Freeze criteria

- Source stamp reconciled.
- Every operation tied to exact Worker handler and caller.
- Request and response fields, nullability, enums, limits, status codes and authorization documented.
- Organization-admin DTO decision recorded.
- Error union and localization mapping approved.
- C-09 schema/version approved and fixtures validated.
- Gate 0b formally authorized before implementation begins.