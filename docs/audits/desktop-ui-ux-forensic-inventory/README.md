# ONYX Desktop UI/UX Forensic Inventory

**Audit type:** Read-only source inventory  
**Investigation date:** 2026-10-25  
**Source repository:** https://github.com/SMozaff/Onyx-Framework  
**Source branch:** `main`  
**Source revision inspected:** `d8f64889cfe4473d40d50c41b0992bd22955eb7b`  
**Source commit date:** 2026-10-25T13:42:04Z  
**Report state:** Static source evidence captured; runtime enumeration and several exhaustiveness checks remain unverified.

## Scope and method

This inventory follows the attached forensic prompt. It covers the Windows/Linux desktop applications and the code, configuration, tests, Rust boundary, assets, localization, and data contracts directly used by those applications. Android UI, mobile PWA UI, and browser-only web UI are excluded.

The inspected Git tree contained 1,365 entries and was returned with `truncated=false`. Application trees inspected include `crates/bins/desktop-shell/` and `crates/bins/admin-shell/`. Source links below are pinned to the investigated commit; no source code or configuration was changed as part of inspection. The audit branch contains documentation only.

Counts intentionally distinguish route-table entries, component templates, UI states, and runtime-rendered instances. A JSX map template may produce zero, one, or many visible controls; static source alone cannot establish a live DOM count. I did not launch Windows or Linux builds or execute the test suites, so this report does not label any runtime behavior as test-confirmed.

## Findings at a glance

| Measure | Source-confirmed result | Counting note |
|---|---:|---|
| Desktop applications | 2 | ONYX desktop shell and ONYX Admin |
| Top-level window declarations | 2 | One window object in each Tauri config |
| Declared window dimensions | 1280 × 800 each | Minimum dimensions and resizability are not specified in the inspected window objects |
| React screen/page component templates | 14 | 9 desktop-shell pages and 5 admin-shell pages |
| Additional top-level UI states | 4 | Desktop startup loading/error; Admin Suspense loading/admin-access-denied |
| UI screen/state classes tallied | 18 | Templates and startup/access states, not runtime DOM snapshots |
| Route-table entries | 21 | Desktop shell 13; Admin shell 8; includes wildcard fallback entries |
| Admin role-dependent navigation | 3 standard links, plus Organizations for All-Father | Defined in `adminNavigation.ts` |
| Staff desktop navigation items | 8 | Defined in `MainLayout.tsx` |
| Explicit shared CSS design-token declarations | 13 | Seven surface/text/accent tokens and six status tokens |
| App-directory visual-asset files | 29 | 14 Admin-shell asset paths and 15 desktop-shell asset paths from the source tree |
| Additional branding asset found | 1 | `assets/branding/admin-shell.png`; usage and image metadata not established here |
| Explicit literal HTML `<form>` wrappers found in inspected source | At least 4 | Workflow groupings are broader than HTML form tags |
| Exact rendered control instances | Not established | Repeated list/table elements and conditional UI require runtime state to enumerate |

## Main architectural observations

- Both applications are Tauri 2 shells with React/TypeScript/Vite UIs. Their Tauri configurations declare one window each and activate bundles for all configured targets.
- `desktop-shell` has login, dashboard, missions, tasks, approvals, messaging, files, notifications, and account-only settings page components. Its session is loaded through the Rust command `get_current_session`; its routes render only after a session is present.
- `admin-shell` has login, users, policy/settings, staff profiles, and organization management page components. `/organizations` is rendered only when the stored user role is `ALL_FATHER`; the app separately requires authentication and `is_admin` for the administrative layout.
- Admin-shell's credential request is sent by a Rust Tauri command (`admin_login`) to the configured ONYX Worker endpoint. Desktop-shell's Rust `login` command also authenticates against the ONYX Worker and stores the session via the `SecureStorage` port.
- Desktop-shell's persisted session stores identity metadata for the webview while the access/refresh tokens remain in Rust-side storage. Admin-shell UI session data uses browser `sessionStorage`; the inspected source does not use `localStorage` for that store.
- The existing staff-profile transfer UI accepts CSV and JSON. No Excel/XLS/XLSX import/export path was found in the inspected desktop application trees and package manifests.
- User hierarchy evidence includes user classes and a nullable `parent_user_id` in the database migrations and UI. Profile fields also include a department string and `team_memberships: string[]`; those fields do not by themselves prove dedicated department/team tables.
- No desktop org-chart/graph/diagram component or diagramming-library use was found in the inspected app trees and desktop package manifests. This is a scoped search result, not a claim that no such functionality can exist outside those inspected paths.

## Report index

1. [Application and window inventory](application-inventory.md)
2. [Screen register](screen-inventory.csv)
3. [Screen-by-screen structure](screen-structure.md)
4. [Interactive element register](interactive-elements.csv)
5. [Forms and fields](forms-and-fields.csv)
6. [Navigation and flows](navigation-and-flows.md)
7. [Animation and motion inventory](animation-inventory.csv)
8. [Assets and design tokens](assets-and-tokens.csv)
9. [Windows/Linux differences](platform-differences.md)
10. [Rust/UI integration](rust-ui-integration.md)
11. [Organizational data model](organizational-data-model.md)
12. [Diagram and spreadsheet evidence](diagram-and-spreadsheet-inventory.md)
13. [Counts and methodology](counts-and-methodology.md)
14. [Evidence register](evidence-register.csv)
15. [Unresolved facts](unresolved-facts.md)

## Evidence labels

- **Source-confirmed:** directly visible in code, configuration, schemas, or the returned repository tree.
- **Test-confirmed:** a test was run and its result recorded. No test suite was executed during this audit.
- **Documentation-only:** found in repository documentation but not independently corroborated in executable source.
- **Unverified:** the inspected evidence is incomplete, runtime-only, conditional, or ambiguous.

## Revision and scope limitations

The audit was conducted against `main` at `d8f64889cfe4473d40d50c41b0992bd22955eb7b`. The source revision was not changed while inspecting. Runtime screenshots, native focus and window behavior, pixel dimensions/alpha of image binaries, Windows/Linux desktop session behavior, and fully enumerated dynamic DOM instances were not obtained. See [unresolved-facts.md](unresolved-facts.md) for the exact boundaries.
