# Application and Window Inventory

Source revision: `d8f64889cfe4473d40d50c41b0992bd22955eb7b`.

## Applications

| ID | Application | Product ID | UI technology | Entry points | Target platforms/evidence |
|---|---|---|---|---|---|
| APP-DS | ONYX desktop shell | `com.onyx.platform` | Tauri 2, Rust command layer, React 19 / TypeScript / Vite | `crates/bins/desktop-shell/src/main.rs`; `src/lib.rs`; `ui/src/main.tsx`; `ui/src/App.tsx` | Tauri bundle targets are configured as `all`; Linux DEB icon mapping is explicit. Windows/Linux packaging is intended; runtime launches were not performed. |
| APP-AD | ONYX Admin | `com.onyx.admin` | Tauri 2, Rust sign-in command, React 19 / TypeScript / Vite | `crates/bins/admin-shell/src/main.rs`; `src/lib.rs`; `ui/src/main.tsx`; `ui/src/App.tsx` | Tauri bundle targets are configured as `all`; Linux DEB icon mapping is explicit. Windows/Linux packaging is intended; runtime launches were not performed. |

## Top-level windows

| Window ID | App | Title | Initial width × height | Minimum dimensions | Resizable | Modality/ownership | Initial visible state | Evidence |
|---|---|---|---|---|---|---|---|---|
| WIN-DS-01 | APP-DS | `ONYX` | 1280 × 800 | Not specified in the window object | Not specified in the window object | One Tauri top-level window; no additional native child-window registration found in inspected app configuration | Session loading, then login/startup error or the route outlet | [desktop-shell/tauri.conf.json](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/crates/bins/desktop-shell/tauri.conf.json#L1-L25); [App.tsx](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/crates/bins/desktop-shell/ui/src/App.tsx#L1-L90) |
| WIN-AD-01 | APP-AD | `ONYX Admin` | 1280 × 800 | Not specified in the window object | Not specified in the window object | One Tauri top-level window; dialogs observed in inspected UI are DOM overlays, not separately declared native windows | React Suspense loading fallback; /login is the unauthenticated route | [admin-shell/tauri.conf.json](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/crates/bins/admin-shell/tauri.conf.json#L1-L25); [App.tsx](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/crates/bins/admin-shell/ui/src/App.tsx#L1-L90) |

## Manifests and implementation roots

| Area | Path | Source evidence |
|---|---|---|
| Workspace package | `Cargo.toml` | Repository root workspace |
| Desktop Rust crate | `crates/bins/desktop-shell/Cargo.toml` | `desktop-shell` package |
| Desktop Tauri config | `crates/bins/desktop-shell/tauri.conf.json` | Window, asset targets, build/dev commands, capabilities |
| Desktop UI package | `crates/bins/desktop-shell/ui/package.json` | React, React Router, Tauri API, Vite, TypeScript |
| Admin Rust crate | `crates/bins/admin-shell/Cargo.toml` | `admin-shell` package |
| Admin Tauri config | `crates/bins/admin-shell/tauri.conf.json` | Window, asset targets, build/dev commands, `admin-main` capability |
| Admin UI package | `crates/bins/admin-shell/ui/package.json` | React, React Router, Axios, Zustand, Tauri API, Vite |
| Localization source | `shared/i18n/en.json`, `shared/i18n/fa.json` | Shared canonical locale catalogs |
| Desktop UI source | `crates/bins/desktop-shell/ui/src/` | 28 tracked file entries under the path in the returned tree |
| Admin UI source | `crates/bins/admin-shell/ui/src/` | 35 tracked file entries under the path in the returned tree |

## Resource notes

Both Tauri configs point at `ui/dist` and run `npm run dev` / `npm run build` in their UI working directories. The configured development ports are 5173 (desktop shell) and 5174 (admin shell). Both have `csp: null` in the inspected Tauri configuration. Admin config explicitly names the `admin-main` capability; this is not evidence of runtime capability behavior.

The native binary entry points set `windows_subsystem = "windows"` in non-debug builds and delegate setup to the respective library `run()` function: [desktop main.rs](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/crates/bins/desktop-shell/src/main.rs#L1-L12), [admin main.rs](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/crates/bins/admin-shell/src/main.rs#L1-L7).
