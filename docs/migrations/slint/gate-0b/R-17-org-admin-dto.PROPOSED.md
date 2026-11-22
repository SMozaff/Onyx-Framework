# R-17 — canonical organization-admin create DTO — PROPOSED (2026-11-23)

**Status: PROPOSED — requires B-10 + Worker/API contract-owner approval.** G0B-12 stays OPEN.
Do not fabricate timestamps or cast around the mismatch.

## Source evidence

Worker `deploy/cloudflare/edge-gateway/src/index.ts`:

- `POST /api/allfather/admins` → `createUser(...)` returns (line ~654):
  `{id, username, email, organization_id, is_admin:true, is_active:true, role:"ORGANIZATION_ADMIN"}`
  with HTTP 201 — **no `created_at` / `updated_at`**.
- `GET /api/allfather/admins` (line ~623) **does** include
  `created_at: Number(row.created_at)` and `updated_at: Number(row.updated_at)`.

Tauri admin UI `crates/bins/admin-shell/ui/src/pages/organizationsApi.ts:11-19` declares
`OrganizationAdminRow` with `id, username, email, organization_id, is_active, created_at, updated_at`
and uses it as the return type of `createAdmin(...)` (lines 49–50). Earlier B-01 audit records the
mismatch (`B-01_Endpoint_Contract_Audit.md:22`, `B-01_C01_Worker_API_Contract_Draft.md:89`,
`B-01_Final_Report.md:27`).

Slint Admin host `crates/bins/admin-slint/src/orgs.rs` declares only the fields it uses
(`id`, `username`, `is_active`), so it does not depend on the disputed fields; the canonical DTO
decision still affects the shared Worker contract and the Tauri admin.

## PROPOSED options for owner decision

| Option | Canonical create DTO | Consequence |
|---|---|---|
| A (recommended for source fidelity) | Worker response as-is: `id, username, email, organization_id, is_admin, is_active, role` | Tauri `OrganizationAdminRow` must stop requiring timestamps for create; list DTO remains separate. No fabricated data. |
| B | Worker create route adds authoritative `created_at` / `updated_at` from the inserted row | Requires a Worker change plus a route test; changes the response contract surface — owner approval needed. |

The Slint Admin host is compatible with either option. The canonical decision must include a
regression test against the verified response shape.

## Acceptance (not met)

Owner-approved DTO with a pinned source revision and a passing regression test. This draft closes
nothing.
