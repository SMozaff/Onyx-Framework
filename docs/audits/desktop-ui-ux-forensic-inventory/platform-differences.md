# Windows/Linux Platform Differences

Source revision: `d8f64889cfe4473d40d50c41b0992bd22955eb7b`. This is a static configuration/source comparison. No Windows or Linux executable was launched.

| Area | Windows-related evidence | Linux-related evidence | Classification / limit |
|---|---|---|---|
| Target packaging | Both Tauri configs set bundle targets to `all`; both binaries use `cfg_attr(not(debug_assertions), windows_subsystem = "windows")` in `main.rs`. | Both configs explicitly map ten Hicolor PNG size paths into the Linux DEB package. | Source-confirmed config only; actual produced formats/artifacts not built in this audit. |
| Native window | Each config declares one window (1280 × 800) and common webview frontend. | Same declared window dimensions and frontend structure. | Source-confirmed; OS titlebar/system-decoration rendering not observed. |
| Sign-in HTTP transport | Admin sign-in Rust source explicitly mentions WebView2 on Windows as a reason to send the request using native Rust/reqwest. | Same source explicitly mentions WebKitGTK on Linux. | Documentation in executable source plus native code. Actual transport success/latency is unverified. |
| Staff session credential store | Shared `KeyringSecureStorage` implementation uses the `keyring` crate’s v1 API and labels its service `com.onyx.platform`. | Same adapter is intended to use the platform’s native credential store behind one implementation. | Source-confirmed abstraction; exact backend chosen by dependency and credential-store availability at runtime not observed. |
| UI framework | React/TypeScript/Vite over Tauri webview. | Same React/TypeScript/Vite implementation over Tauri webview. | Source-confirmed; per-OS font rendering and native webview differences not measured. |
| App icon assets | `icon.ico` is listed with `icon.icns` and `icon.png` in both Tauri configs; source references Windows and cross-platform packaging. | Per-size Hicolor PNG paths are mapped into each Linux DEB package. | Source-confirmed config/tree; installed shortcut/menu icon appearance is not observed. |
| Asset handling | Desktop login uses root-relative URL `/onyx-ring.png`. | Same code uses same URL. | The file is not listed under `desktop-shell/ui/public/` in the returned tree, while root `assets/branding/onyx-ring.png` exists. Whether a build/copy mechanism supplies it is unresolved. |
| OS menu/tray/shortcuts | No explicit app menu, tray, or shortcut registration was identified in the inspected desktop-shell source paths. | No explicit app menu, tray, or shortcut registration was identified in the inspected desktop-shell source paths. | Unverified absence within inspected paths, not a repository-wide proof of absence. |
| Filesystem interaction | UI collects path-like input; native Rust performs upload/download commands. | Same UI/Rust path. | Actual path permission rules and native dialogs by OS were not exercised. |
| Locale and RTL | Shared locale context, selector, dictionaries and RTL stylesheet. | Same. | Source-confirmed; layout correctness/font availability not rendered per OS. |
| Accessibility | Focus-visible CSS and a custom accessible modal focus loop exist in source. | Same shared code. | Runtime keyboard/screen-reader behavior not tested. |

## Files directly compared

- `crates/bins/desktop-shell/tauri.conf.json`
- `crates/bins/admin-shell/tauri.conf.json`
- `crates/bins/desktop-shell/src/main.rs`
- `crates/bins/admin-shell/src/main.rs`
- `crates/bins/desktop-shell/src/secure_storage/mod.rs`
- `crates/bins/desktop-shell/src/secure_storage/keyring_adapter.rs`
- `crates/bins/admin-shell/src/login.rs`
- Both `ui/src/index.css` files and common localization paths

No platform difference is marked “test-confirmed.”
