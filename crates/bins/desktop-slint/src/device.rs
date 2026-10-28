//! Per-device storage for the native Slint desktop.
//!
//! The replica id identifies this install, not a login. It is created once,
//! stored in the app data folder, and reused on every launch. The Worker keys
//! outbox uploads by replica id, so a new id on each launch would strand rows
//! saved in earlier sessions.

use std::path::{Path, PathBuf};

use platform_kernel::ReplicaId;

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

/// Reads the device replica id from `dir/replica_id`, creating it on first run.
pub fn load_or_create_replica_id(dir: &Path) -> Result<ReplicaId, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("create data folder: {e}"))?;
    let path = dir.join("replica_id");
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let uuid = uuid::Uuid::parse_str(text.trim())
                .map_err(|e| format!("stored replica id is invalid: {e}"))?;
            Ok(ReplicaId(*uuid.as_bytes()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let fresh = ReplicaId::new_random();
            let text = uuid::Uuid::from_bytes(fresh.0).to_string();
            std::fs::write(&path, text).map_err(|e| format!("write replica id: {e}"))?;
            Ok(fresh)
        }
        Err(e) => Err(format!("read replica id: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replica_id_is_created_once_and_reused() {
        let dir = std::env::temp_dir().join(format!("onyx-slint-test-{}", uuid::Uuid::new_v4()));
        let first = load_or_create_replica_id(&dir).unwrap();
        let second = load_or_create_replica_id(&dir).unwrap();
        assert_eq!(first, second);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn invalid_stored_replica_id_is_an_error() {
        let dir = std::env::temp_dir().join(format!("onyx-slint-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("replica_id"), "not-a-uuid").unwrap();
        assert!(load_or_create_replica_id(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
