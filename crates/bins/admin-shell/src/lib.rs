//! # admin-shell
//!
//! The ONYX Admin platform — a separate, thin-HTTP-client Tauri
//! desktop app. Unlike `desktop-shell` (which embeds
//! `client-composition` for full local-first operation with its own
//! SQLite database and sync engine), this shell has **no local state at
//! all**: `admin-ui` (the React app this wraps) talks directly to
//! `api-server` over HTTP for every action, matching the confirmed
//! product decision that "changes go through all platforms via the
//! server" — see `DECISIONS.md`'s "Admin Platform" section.
//!
//! This means `lib.rs` here is deliberately much smaller than
//! `desktop-shell`'s — there is no `AppState`, no database pool, no
//! sync agent, no Tauri command surface for domain commands/queries
//! (the webview calls `api-server`'s HTTP routes directly via
//! `fetch`/`axios`, the same way `web-ui` does). The only Tauri-level
//! capability this shell adds beyond a plain browser tab is a native
//! file picker (via `tauri-plugin-dialog`) for the batch profile
//! import/export flow, since a native "choose a file" dialog is a
//! better experience than a bare HTML `<input type="file">` in a
//! desktop app.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running the ONYX Admin platform");
}
