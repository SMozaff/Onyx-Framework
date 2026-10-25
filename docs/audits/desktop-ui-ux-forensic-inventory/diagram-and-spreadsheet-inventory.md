# Diagram and Spreadsheet Feature Inventory

Source revision: `d8f64889cfe4473d40d50c41b0992bd22955eb7b`. This report records existing source evidence only.

## Result summary

| Capability queried | Observed evidence | Status in inspected desktop scope |
|---|---|---|
| Organization chart / hierarchy diagram | Admin exposes class and parent-user selectors; migrations contain hierarchy fields. No org-chart rendering component was located in the inspected desktop UI trees. | **Not found in inspected UI**; underlying hierarchy data is present |
| Diagram/graph renderer | No graph-node/edge diagram component, diagramming package, or chart library was found in the inspected desktop UI package manifests and named UI paths. | **Not found in inspected desktop paths** |
| Spreadsheet grid/editor | No spreadsheet editor or grid-specific dependency was found in the inspected desktop UI package manifests. | **Not found in inspected desktop paths** |
| XLS/XLSX import/export | Profile file input accepts `.csv,.json,text/csv,application/json`; transfer type is the union `"csv" | "json"`; no XLS/XLSX extension/MIME branch was found. | **Not found** in inspected profile transfer |
| CSV import/export | `Profiles.tsx` exposes Import, Export CSV. `profileTransfer.ts` posts text as `text/csv` and requests export using `format=csv`. | **Source-confirmed** |
| JSON import/export | `Profiles.tsx` exposes Export JSON and accepts `.json` / `application/json`; transfer module requests `format=json`. | **Source-confirmed** |
| User hierarchy editing | Users page has class/parent select controls; source includes hierarchy migrations. | **Source-confirmed**, but not a visual diagram |
| Native file dialogs in Admin | Tauri dialog plugin is initialized and listed as a dependency; direct UI invocation was not established in the inspected profile-transfer path. | Plugin presence **source-confirmed**; actual desktop feature use **unverified** |

## Relevant evidence paths

- `crates/bins/admin-shell/ui/src/pages/Users.tsx`
- `crates/bins/admin-shell/ui/src/pages/Profiles.tsx`
- `crates/bins/admin-shell/ui/src/pages/profileTransfer.ts`
- `crates/bins/admin-shell/ui/package.json`
- `crates/bins/desktop-shell/ui/package.json`
- `migrations/postgres/20260107000000_add_user_class_hierarchy.up.sql`
- `migrations/sqlite/20260107000000_add_user_class_hierarchy.up.sql`
- `deploy/cloudflare/edge-gateway/migrations/0001_foundation.sql`

## Scope boundary

“Not found” means no matching implementation was identified in the paths/packages listed above during this source review. It does not establish repository-wide impossibility or assert a product decision. No screenshots, running app state, external spreadsheet files, or separate user-provided diagrams were inspected.
