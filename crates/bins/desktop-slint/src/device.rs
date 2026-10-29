//! Per-device storage for the native Slint desktop: the app data folder.

use std::path::PathBuf;

/// Folder for this app's database, replica id and blobs. Kept separate from
/// the Tauri desktop shell's folder so the two never share a database.
pub fn data_dir() -> Result<PathBuf, String> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    base.map(|b| b.join("onyx-slint"))
        .ok_or_else(|| "no per-user data folder is available".to_string())
}

/// Shared replica-id helper (see `client_composition::device_identity`).
pub use client_composition::device_identity::load_or_create_replica_id;
