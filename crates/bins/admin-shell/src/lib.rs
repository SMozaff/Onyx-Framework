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

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::Command,
    thread,
    time::Duration,
};

use tauri::{AppHandle, Emitter};

const OAUTH_CALLBACK_ADDRESS: &str = "127.0.0.1:17645";
const OAUTH_CALLBACK_PREFIX: &str = "http://127.0.0.1:17645/auth/callback";

/// Opens the Google/Supabase authorization URL in the system browser and
/// forwards the one-time loopback callback into the running Admin webview.
/// The fixed callback must be allow-listed in Supabase Auth redirect URLs.
#[tauri::command]
fn start_allfather_oauth(app: AppHandle, authorize_url: String) -> Result<(), String> {
    if !authorize_url.starts_with("https://")
        || !authorize_url.contains("/auth/v1/authorize?")
        || !authorize_url.contains("redirect_to=http%3A%2F%2F127.0.0.1%3A17645%2Fauth%2Fcallback")
        || authorize_url.contains('\\r') || authorize_url.contains('\\n')
    {
        return Err("Rejected invalid All-Father authorization URL".into());
    }

    // Bind before launching the browser so the callback cannot race startup.
    let listener = TcpListener::bind(OAUTH_CALLBACK_ADDRESS)
        .map_err(|_| "Could not start the local sign-in callback listener (port 17645 may be busy)".to_string())?;
    thread::spawn(move || {
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else { continue };
            stream.set_read_timeout(Some(Duration::from_secs(15))).ok();
            if let Some(callback_url) = read_oauth_callback(&mut stream) {
                let body = "<!doctype html><title>ONYX Admin</title><p>Sign-in received. Return to ONYX Admin.</p>";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(), body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = app.emit("allfather-oauth-callback", callback_url);
                break;
            }
            let body = "Not found";
            let response = format!(
                "HTTP/1.1 404 Not Found\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                body.len(), body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    open_system_browser(&authorize_url)
        .map_err(|_| "Could not open the system browser for Google sign-in".to_string())
}

fn read_oauth_callback(stream: &mut TcpStream) -> Option<String> {
    let mut buffer = [0_u8; 8192];
    let count = stream.read(&mut buffer).ok()?;
    if count == 0 {
        return None;
    }
    let request = std::str::from_utf8(&buffer[..count]).ok()?;
    let first_line = request.lines().next()?;
    let mut parts = first_line.split_whitespace();
    if parts.next()? != "GET" {
        return None;
    }
    let target = parts.next()?;
    let host = request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("host").then(|| value.trim())
    })?;
    if host != OAUTH_CALLBACK_ADDRESS || !target.starts_with("/auth/callback?") {
        return None;
    }
    Some(format!("{}{}", OAUTH_CALLBACK_PREFIX, &target["/auth/callback".len()..]))
}

fn open_system_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(url);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };
    command.spawn().map(|_| ())
}

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
        .invoke_handler(tauri::generate_handler![start_allfather_oauth])
        .run(tauri::generate_context!())
        .expect("error while running the ONYX Admin platform");
}
