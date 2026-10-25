# Organizational Data Model Evidence

Source revision: `d8f64889cfe4473d40d50c41b0992bd22955eb7b`. This register describes source fields and observed API contracts; it is not a normalized conceptual redesign.

## Entities and relationships observed

| Entity | Stored or declared fields | Relationship/usage evidence | Evidence classification |
|---|---|---|---|
| Organization | `id`, `name`, `is_active`, `created_at`, `updated_at` in Admin UI type | Admin page lists, creates, renames and activates/deactivates organizations. | Source-confirmed: `admin-shell/ui/src/pages/organizationsApi.ts`; Worker migrations/API |
| User/account | D1 migration includes `id`, `username`, `organization_id`, `password_hash`, `is_admin`, `is_active`, `class`, `parent_user_id`, timestamps; auth migration adds `email` and `role` | Organization-scoped account lists; class and parent actions in Admin UI; role-based routes. | Source-confirmed: `deploy/cloudflare/edge-gateway/migrations/0001_foundation.sql`, `0002_auth_hierarchy.sql`, `admin-shell/ui/src/pages/Users.tsx` |
| User class | Values in UI include `top_level_manager`, `senior_manager`, `team_leader`, `supervisor`, `staff`; nullable/no-class selection is presented | User class is independently assignable; mobile-access settings are based on configured classes. | Source-confirmed: `admin-shell/ui/src/pages/Users.tsx`; `admin-shell/ui/src/pages/Settings.tsx` |
| Reporting relationship | `parent_user_id` nullable user reference in migrations and user UI | User row includes a parent select populated from the user directory; migrations describe the field as a self-reference/reporting line. | Source-confirmed: `Users.tsx`; Postgres/SQLite hierarchy migrations; actual UI prevention of cycles not established |
| Legacy manager compatibility | `is_manager` field/index added to the D1 user table by `0006_b1_admin_routes.sql` | Exists as compatibility field alongside class/parent hierarchy in migration history. | Source-confirmed; effect on current visible UI not independently established |
| Staff profile | `profile_id`, `owner_id`, `full_name`, `photo_blob_key`, `job_title`, `contact_email`, `contact_phone`, `department`, `class_label`, `parent_display_name`, `team_memberships: string[]` | Profile list and edit overlay use `/api/profiles` and `PUT /api/admin/profiles`; CSV/JSON batch transfer. | Source-confirmed: `admin-shell/ui/src/pages/Profiles.tsx`; API contract |
| Mobile-access policy | Organization ID, JSON `allowed_classes`, version, updated_by, updated_at, last_operation_id | Settings panel displays/saves allowed classes; contract declares GET/PUT of `/api/admin/mobile-access`. | Source-confirmed: `deploy/cloudflare/edge-gateway/migrations/0005_admin_settings.sql`; Settings page; API contract |
| Policy aggregate | Policy ID/version/commands and rule fields shown in policy editor; persistent aggregate/event architecture in D1 foundation | UI looks up, creates, drafts, publishes, retires and evaluates policies. | Source-confirmed UI; underlying runtime state unverified |
| Legal hold | Target ID/type/reason and selected hold ID are UI fields; policy/settings backend contract includes legal-hold endpoints | UI creates and loads/releases holds. Exact record schema not exhaustively inspected here. | Source-confirmed UI/API contract; detailed persistence shape not enumerated |
| File asset and ACL | File asset hash, organization, blob key, content type, size, creation timestamp; ACL rows include hash, organization, user, granted_by, created_at | Files UI has upload/download and grant/revoke/quarantine/archive actions. | Source-confirmed: `0001_foundation.sql`, `0007_file_asset_acl.sql`, Files page |
| Aggregate/event record | D1 `aggregates` include aggregate_type/version/lifecycle_epoch/authority_epoch/state/org; `domain_events` include event, aggregate, payload, time and actor context | Desktop page commands use envelope-based command/query paths; this table is not a direct UI editor. | Source-confirmed schema/integration map |

## Authorization and access paths

- The Admin UI route requires a session and `user.is_admin`; the organization route/navigation additionally requires role `ALL_FATHER`. The code comments explicitly distinguish client-side UX gates from the server as actual authorization boundary.
- Admin organization operations use `/api/allfather/organizations` and `/api/allfather/admins` paths; account management uses `/api/admin/users` and row-specific subpaths.
- Staff profile list/update/import/export endpoints are documented as authenticated/organization-admin operations in `docs/production/api-contract.md`. The report did not exercise these endpoints.
- The mobile-access setting's schema contains organization scope and optimistic version; actual conflict and permission behavior was not tested here.

## Hierarchy evidence

The Postgres hierarchy migration defines classes `top_level_manager`, `senior_manager`, `team_leader`, `supervisor`, and `staff`, plus nullable `parent_user_id` with a self-reference. The migration comments state that a self-reference alone does not prevent cycles. The SQLite mirror explicitly notes its CHECK-constraint and foreign-key-enforcement differences. These are schema comments and constraints; live data validity was not inspected.

## Department, team and org-chart boundaries

- `department` in the profile interface is nullable scalar text.
- `team_memberships` is represented in the profile interface as a string array.
- No dedicated department/team entity tables or desktop org-chart/graph/diagram component was found in the inspected app UI paths and migration files searched. This is a bounded search result; it does not establish that no relevant concept appears elsewhere in the repository.
- There is no spreadsheet engine or XLS/XLSX path found in the inspected desktop package manifests. Existing profile data transfer is CSV/JSON.
