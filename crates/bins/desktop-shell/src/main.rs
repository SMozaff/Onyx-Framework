//! Thin binary entry point. All real setup lives in `lib.rs`'s `run()`,
//! per Tauri 2's convention (commands are defined in `lib.rs`, not
//! `main.rs` — verified against the current, authoritative Tauri 2 docs
//! before writing this; this differs from Tauri 1's convention, where
//! `main.rs` held everything, which is the shape Team Prompt 5's own
//! §4.1 snippet uses. See ruling T2 in `DECISIONS.md`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    desktop_shell_lib::run();
}
