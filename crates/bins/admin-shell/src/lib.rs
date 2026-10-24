//! # admin-shell
//!
//! The ONYX Admin platform is a thin Tauri shell. The React Admin UI sends
//! username/password credentials to the configured ONYX Cloudflare Worker;
//! Tauri provides native file dialogs only. Authentication is server-owned
//! and no Google/Supabase callback command is exposed to the webview.

mod login;

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
        .invoke_handler(tauri::generate_handler![login::admin_login])
        .run(tauri::generate_context!())
        .expect("error while running the ONYX Admin platform");
}
