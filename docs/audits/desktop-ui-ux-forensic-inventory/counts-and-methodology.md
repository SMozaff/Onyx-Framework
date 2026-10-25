# Counts and Methodology

## Source revision

- Repository: `SMozaff/Onyx-Framework`
- Branch: `main`
- Commit inspected: `d8f64889cfe4473d40d50c41b0992bd22955eb7b`
- Commit time: `2026-10-25T13:42:04Z`
- Recursive tree entries returned: 1,365; `truncated=false`

## Counts

| Count | Value | Calculation / limitations |
|---|---:|---|
| Desktop app shells | 2 | `crates/bins/desktop-shell`, `crates/bins/admin-shell` |
| Tauri top-level window declarations | 2 | One configured window in each app config |
| React page component templates | 14 | Desktop shell 9; Admin shell 5 |
| Additional app-level UI states | 4 | Desktop session loading/startup error; Admin Suspense fallback/admin-access-denied |
| Screen/state register rows | 18 | 14 page templates + 4 states; modal overlays are separately described rather than added as pages |
| JSX route-table entries | 21 | Desktop 13 entries including wildcard; Admin 8 including wildcard. A route entry is not necessarily a unique screen. |
| Explicit nav item definitions | 11 baseline | Desktop 8; Admin 3 standard. Admin Organizations is a fourth role-conditional item. |
| UI source file entries | 63 | 28 under `desktop-shell/ui/src/`; 35 under `admin-shell/ui/src/`, according to returned Git tree path counts |
| Hicolor icon files | 20 | 10 declared icon-size paths per app |
| App-directory visual resources | 29 | 14 Admin shell asset files + 15 desktop shell asset files |
| Root branding files | 2 | `assets/branding/admin-shell.png`, `assets/branding/onyx-ring.png`; their exact use differs/unresolved |
| Theme CSS declarations | 13 | `@theme` tokens in shared CSS blob |
| Explicit `<form>` tags observed | At least 4 | Admin login (1), Organizations create-org (1), Organizations create-admin (1), Desktop login (1). Other data workflows may use non-form buttons/handlers. |
| Exact rendered controls at runtime | Not measured | Dynamic collections, conditional panels, native webview DOM and viewport-dependent controls were not rendered |
| Test suites executed | 0 | This is a static source audit; test files are listed as evidence but were not run |

## Counting rules

1. **Application:** distinct configured desktop Tauri products, not each packaged format.
2. **Window:** one explicit object in `app.windows`; custom DOM overlays are not separate Tauri windows.
3. **Page template:** a distinct React page module referenced by a route or login branch. Routes that share a component (e.g., collection and detail ID patterns) count once as a page template and separately in the route-entry count.
4. **Additional UI state:** a top-level loading/error/authorization component state with its own visible structure, not a transient loading button label.
5. **Route entry:** literal entry in React Router JSX, including a wildcard fallback.
6. **Navigation item:** a source-defined array item. Conditional item and responsive drawer are reported separately; rendered instances were not observed.
7. **Interactive element:** a statically identifiable JSX control/component template. Rows with repetition say so; they are not claims about how many items will render from an actual dataset. CSV records can share a component template.
8. **Asset:** a Git tree file in the app's icon directory, UI public/assets folders, or root branding directory, as listed. Hicolor directory names provide intended size labels; binary pixel dimensions and transparency were not decoded.
9. **Token:** a custom property declared within the shared `@theme` block, not every literal color used by utility classes.
10. **Animation:** explicit transition/motion behavior found in CSS/Tailwind-class source. A state change or polling interval is not automatically counted as an animation.
11. **Evidence status:** source-confirmed reflects source presence, not successful execution. Tests present in the repo are not test-confirmed because no suite was run.
12. **Unknown/unresolved:** left unquantified when exact runtime behavior would require a launched OS build, live server state, full repository-wide semantic parser, or binary asset decode.

## Method

- Inspected the recursive Git tree and fetched source files from the pinned revision with GitHub repository tools.
- Read application configuration, page modules, layouts, reusable dialog/navigation/i18n modules, Rust Tauri registration/session code, relevant UI packages, D1 migration schema and hierarchy migrations.
- Searched package/UI paths for diagram/spreadsheet evidence and reviewed profile transfer source.
- Collected static elements from the source components, recording the source path and approximate line range visible in the fetched version.
- Did not clone/build locally, execute npm/Cargo/CI tests, launch native builds, inspect actual DOM/screen captures, exercise login/HTTP calls, query production data, or inspect credentials. Thus no runtime claims are labelled test-confirmed.
