# ONYX Remediation 02 — Admin API contracts

**Source baseline:** `71f316804d483dfe79139ceec6b72fb5899b8bec`  
**Implementation branch:** `fix/p1-admin-api-contracts-20261012`  
**Status:** Source and CI changes only. Not merged or deployed.

## Canonical contracts

All protected routes require a verified ONYX access token. Role, active-user status, active-organization status, and tenant scope come from current D1 records, not caller-supplied role or organization fields. API errors use the Worker convention `{error, category, retryability}`; unknown fields and invalid values are rejected.

| Method/path | Role | Request | Success |
|---|---|---|---|
| `GET /api/admin/mobile-access` | `ORGANIZATION_ADMIN` or `ALL_FATHER`, scoped only to the caller's organization | No body | `200 {allowed_classes: string[], version: number, updated_at: number|null, enforcement_status: "disabled_untrusted_client_type"}` |
| `PUT /api/admin/mobile-access` | `ORGANIZATION_ADMIN`, own organization | `{allowed_classes: string[], expected_version?: number}` | `200` with the canonical saved representation |
| `POST /api/admin/policies` | `ORGANIZATION_ADMIN` | `{name: string}` | `201 {policy_id, policy, version}` |
| `POST /api/admin/legal-holds` | `ORGANIZATION_ADMIN` | `{target_id: UUID, target_type: supported resource type, reason: string}` | `201 {legal_hold_id, legal_hold, version}` |
| `POST /api/admin/users/:id/class` | `ORGANIZATION_ADMIN`, same organization | `{class: UserClass|string|null}` | `200 {success: true, class}` |
| `GET /api/users/hierarchy` | Any authenticated role; scoped to caller's organization | No body | JSON array of minimal `{id, parent_user_id, is_admin}` records for all active same-org users, sorted by `username`; empty is `[]`; All-Father returns `[]` because its sentinel ID is not a UUID |

The special All-Father principal returns an empty hierarchy array: its reserved `__allfather__` ID is not a UUID accepted by Rust's hierarchy cache, and it is not part of a regular organization reporting tree.

The existing Rust mobile-access contract establishes the five values: `top_level_manager`, `senior_manager`, `team_leader`, `supervisor`, and `staff`. Duplicate/unknown values are rejected. A mobile-access PUT is a **replace-all** operation; a missing configuration reads as the intended restrictive default of an empty list, version `0`, with no update timestamp.

## Persistence, concurrency, and auditing

- `mobile_access_settings` is a single row per organization, keyed by `organization_id`. It stores canonical JSON `allowed_classes`, monotonically increasing `version`, actor ID, update timestamp, and last operation ID. It has an index on update time and no secret fields.
- PUT reads the current version, applies compare-and-swap in a D1 batch, writes its audit record in the same batch, and reads back the saved canonical representation. Mismatched versions return `409 VERSION_CONFLICT`; invalid values make no write. Legacy callers may omit `expected_version`; they still receive atomic server-side conflict detection, but new clients should send it.
- Policy/legal-hold creation is routed through the Worker's existing command semantics, aggregate/event persistence, idempotency record, and audit write. Optional `Idempotency-Key` values are scoped to actor, organization, and route. Reuse with a different payload is a `409 IDEMPOTENCY_CONFLICT`.
- Legal-hold targets must be a valid UUID and must exist within the caller's organization. Aggregate-backed targets are looked up by `(id, organization_id, aggregate_type)`. Unknown or out-of-scope resources return an explicit error without a write. `file_asset` is intentionally excluded and returns a validation error: the Rust domain requires UUID target IDs, while `file_assets` is keyed by `content_hash`, so this identifier contract must be reconciled before file-asset legal holds can be supported. The Admin Settings form defaults to the UUID-backed `task` aggregate.
- Class assignment updates only active users in the caller's organization and writes an audit row atomically with the update. New staff are created with `class = NULL`; authentication role and operational user class are distinct.

## Mobile policy enforcement — explicit release blocker

The Worker persists the intended class grants, but **does not enforce them at login**. The API returns `enforcement_status: "disabled_untrusted_client_type"`, and the Admin Settings UI warns that the saved policy is not effective access control. This is deliberate: the client-supplied `client_type` is untrusted, and using it to deny or grant login would violate the P0 rule that authorization cannot depend on that label. A caller could change `"mobile"` to `"desktop"` to evade a label-based gate.

The login path validates supported client-type values for request-contract hygiene, but the value does not decide authorization. Role and command permissions remain based on D1-backed server identity and policy. Secure mobile-only class gating needs a verifiable client identity/attestation design and client integration (including supported platforms) before enforcement can be enabled. Do not mark the mobile-access acceptance criterion complete until that mechanism and its negative tests exist.

## Migration and release gates

Migration `0005_admin_settings.sql` is additive; it does not rename/replay earlier migrations, reset data, or seed production settings. Migration tests apply the complete migration sequence to an in-memory SQLite database and verify required columns and indexes. Worker readiness now checks the new table/index and must remain degraded until the additive migration is present.

**Not claimed by this branch:** deployment, preview endpoint reachability, production runtime behavior, or production migration application. The repository deploy workflow targets the production Worker and remote D1 on pushes to `main`; do not merge before preview integration tests and release approval. In the connected Cloudflare account, direct read-only queries showed the production migration ledger through `0004_auth_schema_repair.sql` and no `mobile_access_settings` table before these source changes. No production D1 writes or deployments were performed for this remediation.
