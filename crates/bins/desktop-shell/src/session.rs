//! Real per-user login for `desktop-shell`, replacing the previous
//! `TODO(auth/org resolution)` placeholder (`OrganizationId::new_random()`
//! on every launch — see `lib.rs`'s removed comment at the old call
//! site).
//!
//! # Design
//! `desktop-shell` embeds a full local `AppState` (its own SQLite,
//! command/query registries, sync agent) rather than talking to
//! `api-server` for every read like `admin-shell` does. Login here
//! therefore does two things, not one:
//!
//! 1. Calls `api-server`'s real `POST /api/auth/login` (the same
//!    endpoint `admin-shell` uses) to authenticate the person and learn
//!    their real `organization_id`/`user_id`.
//! 2. Persists that identity so the *next* launch uses the same
//!    `organization_id` instead of a fresh random one — otherwise the
//!    local `AppState` would be constructed under an org nothing else
//!    ever queries, silently orphaning all local data (the exact bug
//!    `useSession.ts`'s doc comment flagged on the frontend side).
//!
//! Persistence goes through the existing `SecureStorage` port
//! (`secure_storage/mod.rs`) — that trait's own doc comment already
//! names `"auth.refresh_token"` as an intended key, so this was
//! scaffolded for, just never connected to a real login call.

use std::sync::Arc;

use platform_kernel::{ObjectId, OrganizationId};
use serde::{Deserialize, Serialize};

use crate::secure_storage::SecureStorage;

/// Single storage key for the whole session, stored as one JSON blob
/// rather than five separate `SecureStorage` keys. Atomic save/clear
/// (no risk of e.g. a token saved but the matching org id missing after
/// a partial write) and simpler than coordinating multiple keys that
/// must always agree with each other.
const SESSION_KEY: &str = "auth.session";

/// A persisted, authenticated identity for this device.
///
/// `server_address` travels with the session (not just the tokens)
/// because a session is only meaningful in the context of the server it
/// was issued by — reusing tokens issued by one server against a
/// different `server_address` would be a real correctness bug, not
/// just unlikely.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSession {
    pub server_address: String,
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: ObjectId,
    pub organization_id: OrganizationId,
    pub username: String,
    pub is_admin: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("network error contacting server: {0}")]
    Network(String),
    #[error("server rejected credentials")]
    InvalidCredentials,
    #[error("server returned an unexpected response: {0}")]
    UnexpectedResponse(String),
    #[error("storage error: {0}")]
    Storage(String),
}

/// Mirrors `api-server::routes::auth::LoginRequest` — kept as a
/// separate local type rather than a shared crate dependency, matching
/// how `admin-shell`'s `client.ts` also just POSTs a plain JSON object
/// rather than importing a shared DTO crate (there isn't one shared
/// between server and native clients in this workspace).
#[derive(Serialize)]
struct LoginRequest<'a> {
    username: &'a str,
    password: &'a str,
}

/// Mirrors `api-server::routes::auth::LoginResponse` /
/// `LoginUser` exactly, field-for-field — see that struct's doc
/// comments in `api-server/src/routes/auth.rs` for what each field
/// means server-side.
#[derive(Deserialize)]
struct LoginResponseWire {
    access_token: String,
    refresh_token: String,
    user: LoginUserWire,
}

#[derive(Deserialize)]
struct LoginUserWire {
    id: String,
    username: String,
    organization_id: String,
    is_admin: bool,
}

/// Calls `POST {server_address}/api/auth/login` and, on success, builds
/// a `StoredSession`. Does NOT persist it — the caller decides when to
/// call `save` (in `lib.rs`'s `login` command, only after the new
/// `AppState` has also been rebuilt, so a session is never saved
/// half-applied).
///
/// `server_address` must already include the scheme
/// (`http://192.168.0.250:3000`), matching the same convention used by
/// `admin-shell`'s `utils/serverAddress.ts::isPlausibleServerAddress`.
pub async fn authenticate(
    server_address: &str,
    username: &str,
    password: &str,
) -> Result<StoredSession, SessionError> {
    let url = format!("{}/api/auth/login", server_address.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| SessionError::Network(e.to_string()))?;

    let response = client
        .post(&url)
        .json(&LoginRequest { username, password })
        .send()
        .await
        .map_err(|e| SessionError::Network(e.to_string()))?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(SessionError::InvalidCredentials);
    }
    if !response.status().is_success() {
        let status = response.status();
        return Err(SessionError::UnexpectedResponse(format!(
            "HTTP {status}"
        )));
    }

    let body: LoginResponseWire = response
        .json()
        .await
        .map_err(|e| SessionError::UnexpectedResponse(e.to_string()))?;

    let user_id = ObjectId::from_uuid_str(&body.user.id)
        .map_err(|e| SessionError::UnexpectedResponse(format!("invalid user id: {e}")))?;
    let organization_id = OrganizationId::from_uuid_str(&body.user.organization_id)
        .map_err(|e| SessionError::UnexpectedResponse(format!("invalid organization id: {e}")))?;

    Ok(StoredSession {
        server_address: server_address.trim_end_matches('/').to_string(),
        access_token: body.access_token,
        refresh_token: body.refresh_token,
        user_id,
        organization_id,
        username: body.user.username,
        is_admin: body.user.is_admin,
    })
}

/// Persists a session as one JSON blob via `SecureStorage`.
pub async fn save(storage: &Arc<dyn SecureStorage>, session: &StoredSession) -> Result<(), SessionError> {
    let json = serde_json::to_vec(session)
        .map_err(|e| SessionError::Storage(format!("serializing session: {e}")))?;
    storage
        .store_secret(SESSION_KEY, &json)
        .await
        .map_err(|e| SessionError::Storage(e.to_string()))
}

/// Loads a previously-saved session, if any. `Ok(None)` (not an error)
/// when nothing has been saved yet — the normal, expected state on a
/// fresh install or after logout.
pub async fn load(storage: &Arc<dyn SecureStorage>) -> Result<Option<StoredSession>, SessionError> {
    let bytes = storage
        .get_secret(SESSION_KEY)
        .await
        .map_err(|e| SessionError::Storage(e.to_string()))?;
    match bytes {
        None => Ok(None),
        Some(b) => serde_json::from_slice(&b)
            .map(Some)
            .map_err(|e| SessionError::Storage(format!("corrupt stored session: {e}"))),
    }
}

/// Clears a saved session (logout).
pub async fn clear(storage: &Arc<dyn SecureStorage>) -> Result<(), SessionError> {
    storage
        .delete_secret(SESSION_KEY)
        .await
        .map_err(|e| SessionError::Storage(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_storage::StorageError;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// An in-memory `SecureStorage` fake, standing in for the real
    /// OS-keyring-backed `KeyringSecureStorage` — which needs an actual
    /// platform credential store (Windows Credential Manager / macOS
    /// Keychain / Linux Secret Service) and so cannot run in this build
    /// environment. Tests `save`/`load`/`clear`'s logic (JSON
    /// serialization, key naming, error mapping), not the OS storage
    /// backend itself, which is exercised only by manual/real-device
    /// testing.
    #[derive(Default)]
    struct FakeStorage {
        entries: Mutex<HashMap<String, Vec<u8>>>,
    }

    #[async_trait::async_trait]
    impl SecureStorage for FakeStorage {
        async fn store_secret(&self, key: &str, value: &[u8]) -> Result<(), StorageError> {
            self.entries
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_vec());
            Ok(())
        }

        async fn get_secret(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
            Ok(self.entries.lock().unwrap().get(key).cloned())
        }

        async fn delete_secret(&self, key: &str) -> Result<(), StorageError> {
            self.entries.lock().unwrap().remove(key);
            Ok(())
        }
    }

    fn sample_session() -> StoredSession {
        StoredSession {
            server_address: "http://192.168.0.250:3000".to_string(),
            access_token: "access-abc".to_string(),
            refresh_token: "refresh-xyz".to_string(),
            user_id: ObjectId::new_random(),
            organization_id: OrganizationId::new_random(),
            username: "admin".to_string(),
            is_admin: true,
        }
    }

    #[tokio::test]
    async fn load_returns_none_when_nothing_saved() {
        let storage: Arc<dyn SecureStorage> = Arc::new(FakeStorage::default());
        let loaded = load(&storage).await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn save_then_load_round_trips_every_field() {
        let storage: Arc<dyn SecureStorage> = Arc::new(FakeStorage::default());
        let original = sample_session();

        save(&storage, &original).await.unwrap();
        let loaded = load(&storage).await.unwrap().expect("session should exist");

        assert_eq!(loaded.server_address, original.server_address);
        assert_eq!(loaded.access_token, original.access_token);
        assert_eq!(loaded.refresh_token, original.refresh_token);
        assert_eq!(loaded.user_id, original.user_id);
        assert_eq!(loaded.organization_id, original.organization_id);
        assert_eq!(loaded.username, original.username);
        assert_eq!(loaded.is_admin, original.is_admin);
    }

    #[tokio::test]
    async fn clear_removes_a_saved_session() {
        let storage: Arc<dyn SecureStorage> = Arc::new(FakeStorage::default());
        save(&storage, &sample_session()).await.unwrap();

        clear(&storage).await.unwrap();

        let loaded = load(&storage).await.unwrap();
        assert!(loaded.is_none(), "session should be gone after clear");
    }

    #[tokio::test]
    async fn clear_on_empty_storage_is_not_an_error() {
        // Logging out when nothing was ever saved (e.g. a fresh install)
        // must not fail — SecureStorage::delete_secret on a FakeStorage
        // with no matching key is a no-op, and this asserts that stays
        // true through session::clear's error mapping too.
        let storage: Arc<dyn SecureStorage> = Arc::new(FakeStorage::default());
        clear(&storage).await.unwrap();
    }

    #[tokio::test]
    async fn load_surfaces_corrupt_stored_data_as_an_error_not_a_panic() {
        let storage: Arc<dyn SecureStorage> = Arc::new(FakeStorage::default());
        storage
            .store_secret(SESSION_KEY, b"not valid json")
            .await
            .unwrap();

        let result = load(&storage).await;
        assert!(matches!(result, Err(SessionError::Storage(_))));
    }
}
