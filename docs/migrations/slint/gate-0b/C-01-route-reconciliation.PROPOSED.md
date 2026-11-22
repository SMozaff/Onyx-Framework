# C-01 route reconciliation — PROPOSED draft (2026-11-23)

**Status: PROPOSED — DRAFT, NOT FROZEN.** Requires B-01 + Worker/API contract-owner review.
This is a source-level reconciliation, not deployment proof. Blocker G0B-03 stays `OPEN/IN PROGRESS`.

## Baseline pinning

- Decision log `2026-11-01`: `docs/production/api-contract.md` re-pinned to main `d330451` with 42
  route-completeness tests passing on that commit. This closes only the earlier stale-baseline
  part of G0B-12; the master record still requires a current source review/freeze.
- BOUND earlier baselines: repo `b056c6ef204080446e84b31f3d35a226e6a40867`; Worker
  `38c8975fd6616376d51e016b3e718b7736888514`. A `PROPOSED` reconciliation must be re-run against
  the exact revision chosen for freeze; the pin above is not a deployed revision.
- Worker source reviewed for this draft: `deploy/cloudflare/edge-gateway/src/index.ts`
  (`dispatch()` at lines ~2233–2306). Dispatch exposes 30 distinct URL paths (grep of
  `url.pathname==="…"`), including health/ready.

## Routes consumed by the Admin Slint host (source-level)

| Method | Route | Worker handler / line | Consumer | Notes / gap |
|---|---|---|---|---|
| POST | `/api/auth/login` | `authLogin` (2248) | `admin-slint/src/auth.rs` | Accepts optional `organization`; response carries `must_change_password`. |
| GET | `/api/query` | `queryRoute` (2254) | `policy::query` | Envelope base64url; tenant check; query types `policy.list/detail`, `legal_hold.list/detail`. |
| POST | `/api/command` | `commandRoute` (2255) | `policy::send_command` | Concurrency codes per error envelope; see C-03. |
| GET | `/api/admin/users` | `listAdminUsers` (2260) | `users::list_users` | Canonical row includes `id`, `username`, `class`, `parent_user_id`, `is_active`. |
| POST | `/api/admin/users` | `createStaffRoute` (2261) | `users::create_user` | — |
| POST | `/api/admin/users/:id/class` | `assignUserClass` (2274) | `users::set_class` | — |
| POST | `/api/admin/users/:id/parent` | `setUserParent` (2266) | `users::set_parent` | — |
| POST | `/api/admin/users/:id/{activate,deactivate}` | `setUserActive` (2262–2263) | `users::set_active` | — |
| GET / PUT | `/api/admin/mobile-access` | `getMobileAccess` / `putMobileAccess` (2267–2268) | `settings.rs` | `expected_version` in PUT body. |
| POST | `/api/admin/policies` | `postAdminPolicy` (2269) | `policy::create_policy` | `Idempotency-Key` header; response `policy_id`. |
| POST | `/api/admin/legal-holds` | `postAdminLegalHold` (2270) | `legal_hold::apply` | `Idempotency-Key`; response `legal_hold_id`. |
| PUT | `/api/admin/profiles` | `upsertProfile` (2277) | `profiles::save_profile` | Before/after audit values still open (R-14). |
| POST | `/api/admin/profiles/import` | `importProfiles` (2278) | `profiles::import_bytes` | CSV/JSON/XLSX; report contains one-time passwords (D-04). |
| GET | `/api/admin/profiles/export` | `exportProfiles` (2279) | `profiles::export_bytes` | `?format=`. |
| GET / POST | `/api/allfather/organizations` | `listOrganizations` / `createOrganization` (2287–2288) | `orgs.rs` | All-Father only. |
| POST | `/api/allfather/organizations/:id/{activate,deactivate}` | `setOrganizationActive` (2290–2291) | `orgs.rs` | — |
| GET | `/api/organizations/lookup` | `lookupOrganizationServer` (2292) | Desktop login (S7) | Public; `null` server URL = this server. |
| GET | `/api/allfather/audit` | `listAuditRecords` (2294) | Not used by Slint host | All-Father only. |
| GET | `/api/allfather/users` | `listAllUsersReadOnly` (2296) | Not used by Slint host | Read-only cross-org. |
| GET / POST | `/api/allfather/admins` | lines 2297–2303 | `orgs.rs` | **DTO mismatch (R-17)** — see dedicated draft. |

## Gaps that prevent freezing

1. R-17 canonical org-admin create DTO is unresolved (Worker response lacks timestamps that the
   Tauri UI model expects). Do not fabricate fields.
2. C-03 error taxonomy is not frozen (see C-03 draft).
3. No deployed revision evidence: a source match cannot establish what revision is actually live.
4. Route-completeness tests (42 passing) were recorded on `d330451`; re-run against the freeze pin.
5. Dispatch gaps recorded by the earlier B-01 reconciliation remain: required `organization_id`
   query handling, prefix-vs-exact path shape, method mismatch/501 fallback, `requireSession`
   boundary and DTO/error evidence (`B-01_C01_Dispatch_Reconciliation.md`).

## Acceptance (not yet met)

A frozen C-01 requires a reviewed handler-by-handler table for **every** UI-called endpoint at the
chosen pin, including request/response fields, error variants, auth/role rules, idempotency and
timeout, plus per-route contract tests. No blocker is closed by this draft.
