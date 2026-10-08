//! Thin binary entry point — same convention as `desktop-shell`'s
//! `main.rs` (see that file's own comment for the Tauri 2 rationale).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    admin_shell_lib::run();
}
