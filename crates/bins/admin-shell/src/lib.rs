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
    thread,
    time::Duration,
};

use tauri::{AppHandle, Emitter};

const OAUTH_CALLBACK_ADDRESS: &str = "127.0.0.1:17645";
const OAUTH_CALLBACK_PREFIX: &str = "http://127.0.0.1:17645/auth/callback";
const CALLBACK_PORT_BUSY_ERROR: &str = "ALLFATHER_CALLBACK_PORT_BUSY";
const BROWSER_LAUNCH_ERROR: &str = "ALLFATHER_BROWSER_LAUNCH_FAILED";

/// Opens the Google/Supabase authorization URL in the system browser and
/// forwards the one-time loopback callback into the running Admin webview.
/// The fixed callback must be allow-listed in Supabase Auth redirect URLs.
#[tauri::command]
fn start_allfather_oauth(app: AppHandle, authorize_url: String) -> Result<(), String> {
    if !authorize_url.starts_with("https://uhpqjhbciimklheohvfr.supabase.co/auth/v1/authorize?")
        || !authorize_url.contains("redirect_to=http%3A%2F%2F127.0.0.1%3A17645%2Fauth%2Fcallback")
        || authorize_url.contains('\r')
        || authorize_url.contains('\n')
    {
        return Err("Rejected invalid All-Father authorization URL".into());
    }

    // Bind before launching the browser so the callback cannot race startup.
    // If launch fails, this helper drops the listener before returning.
    let listener = bind_and_launch(OAUTH_CALLBACK_ADDRESS, &authorize_url, |_, url| {
        open_system_browser(url)
    })?;

    thread::spawn(move || {
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else { continue };
            stream.set_read_timeout(Some(Duration::from_secs(15))).ok();
            if let Some(callback_url) = read_oauth_callback(&mut stream) {
                let body = "<!doctype html><title>ONYX Admin</title><p>Sign-in received. Return to ONYX Admin.</p>";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = app.emit("allfather-oauth-callback", callback_url);
                break;
            }
            let body = "Not found";
            let response = format!(
                "HTTP/1.1 404 Not Found\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    Ok(())
}

fn bind_and_launch(
    address: &str,
    authorize_url: &str,
    launch: impl FnOnce(&TcpListener, &str) -> std::io::Result<()>,
) -> Result<TcpListener, String> {
    let listener = TcpListener::bind(address).map_err(|_| CALLBACK_PORT_BUSY_ERROR.to_string())?;
    if launch(&listener, authorize_url).is_err() {
        drop(listener);
        return Err(BROWSER_LAUNCH_ERROR.to_string());
    }
    Ok(listener)
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
    Some(format!(
        "{}{}",
        OAUTH_CALLBACK_PREFIX,
        &target["/auth/callback".len()..]
    ))
}

#[cfg(target_os = "windows")]
fn open_system_browser(url: &str) -> std::io::Result<()> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn ShellExecuteW(
            hwnd: *mut std::ffi::c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show_command: i32,
        ) -> *mut std::ffi::c_void;
    }

    let operation: Vec<u16> = OsStr::new("open").encode_wide().chain(Some(0)).collect();
    let file: Vec<u16> = OsStr::new(url).encode_wide().chain(Some(0)).collect();
    // Hand the URL directly to Windows' registered URL handler. Query
    // ampersands stay data and cannot be parsed as shell operators.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1, // SW_SHOWNORMAL
        )
    };
    if (result as isize) <= 32 {
        return Err(std::io::Error::other(format!(
            "Windows could not open the default browser (ShellExecuteW result {})",
            result as isize
        )));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn open_system_browser(url: &str) -> std::io::Result<()> {
    launch_browser_program(
        std::ffi::OsStr::new("xdg-open"),
        url,
        Duration::from_secs(2),
    )
}

/// Detect immediate xdg-open failures without waiting indefinitely. Desktop
/// handlers may remain alive after dispatching the URL, so a child still
/// running after the bounded grace period is reaped asynchronously.
#[cfg(target_os = "linux")]
fn launch_browser_program(
    program: &std::ffi::OsStr,
    url: &str,
    timeout: Duration,
) -> std::io::Result<()> {
    use std::{
        process::{Command, Stdio},
        time::Instant,
    };

    let mut child = Command::new(program)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            std::io::Error::new(error.kind(), "system browser could not be started")
        })?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait().map_err(|error| {
            std::io::Error::new(error.kind(), "system browser launch status is unavailable")
        })? {
            Some(status) if status.success() => return Ok(()),
            Some(_) => {
                return Err(std::io::Error::other(
                    "system browser rejected the authorization request",
                ));
            }
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            None => {
                thread::spawn(move || {
                    let _ = child.wait();
                });
                return Ok(());
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn open_system_browser(url: &str) -> std::io::Result<()> {
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
}

#[cfg(all(test, target_os = "linux"))]
mod linux_browser_tests {
    use super::{bind_and_launch, launch_browser_program, BROWSER_LAUNCH_ERROR};
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    fn test_program(script: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("onyx-xdg-open-test-{nonce}"));
        fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    #[test]
    fn detects_nonzero_helper_exit_without_leaking_the_authorization_url() {
        let program = test_program("exit 17");
        let secret_url = "https://example.invalid/authorize?code=must-not-leak";
        let error = launch_browser_program(program.as_os_str(), secret_url, Duration::from_secs(1))
            .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("rejected"));
        assert!(!message.contains(secret_url));
        assert!(!message.contains("must-not-leak"));
        fs::remove_file(program).unwrap();
    }

    #[test]
    fn reports_spawn_failure_without_leaking_authorization_data() {
        let missing = std::env::temp_dir().join("onyx-nonexistent-xdg-open-helper");
        let error = launch_browser_program(
            missing.as_os_str(),
            "https://example.invalid/authorize?code=redacted",
            Duration::from_millis(50),
        )
        .unwrap_err();
        assert!(error.to_string().contains("could not be started"));
        assert!(!error.to_string().contains("redacted"));
    }

    #[test]
    fn listener_port_is_released_after_browser_launch_failure() {
        let mut failed_address = None;
        let result = bind_and_launch("127.0.0.1:0", "https://example.invalid/", |listener, _| {
            failed_address = Some(listener.local_addr().unwrap());
            Err(std::io::Error::other("discarded process details"))
        });
        assert_eq!(result.unwrap_err(), BROWSER_LAUNCH_ERROR);
        let address = failed_address.unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        loop {
            match std::net::TcpListener::bind(address) {
                Ok(rebound) => {
                    drop(rebound);
                    break;
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::AddrInUse
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("callback port was not released: {error}"),
            }
        }
    }

    #[test]
    fn returns_after_bounded_handoff_when_helper_remains_running() {
        let program = test_program("sleep 0.1");
        let result = launch_browser_program(
            program.as_os_str(),
            "https://example.invalid/",
            Duration::from_millis(5),
        );
        assert!(result.is_ok());
        fs::remove_file(program).unwrap();
    }
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
