//! Device identity shared by every native client.
//!
//! The replica id identifies an install, not a login. It is created once,
//! stored in a file in the client's data folder, and reused on every launch.
//! The Worker keys outbox uploads by replica id and checks its ownership, so a
//! new id on each launch would strand rows saved in earlier sessions.

use std::path::Path;

use platform_kernel::ReplicaId;

const REPLICA_FILE: &str = "replica_id";

/// Reads `dir/replica_id`, creating it on first run.
pub fn load_or_create_replica_id(dir: &Path) -> Result<ReplicaId, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("create data folder: {e}"))?;
    let path = dir.join(REPLICA_FILE);
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

    fn scratch_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("onyx-device-identity-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn replica_id_is_created_once_and_reused() {
        let dir = scratch_dir();
        let first = load_or_create_replica_id(&dir).unwrap();
        let second = load_or_create_replica_id(&dir).unwrap();
        assert_eq!(first, second);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn invalid_stored_replica_id_is_an_error() {
        let dir = scratch_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(REPLICA_FILE), "not-a-uuid").unwrap();
        assert!(load_or_create_replica_id(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
