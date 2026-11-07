//! OS credential-store adapter for `SecureStorage`, backed by `keyring-core`.
//!
//! The credential store is injected. `for_service` uses the platform default
//! (initialised once by the `keyring` crate); `with_store` takes any store,
//! which lets tests run against the in-memory mock without an OS keyring.
//!
//! `keyring-core` calls are synchronous platform I/O, so each is wrapped in
//! `tokio::task::spawn_blocking` to avoid stalling the async runtime.

use std::sync::Arc;

use super::{SecureStorage, StorageError};
use async_trait::async_trait;
use keyring_core::{CredentialStore, Error as KeyringError};

/// Default service identity for the Tauri desktop shell. Matches Team Prompt 5
/// §3.1's macOS entry (`com.onyx.platform`).
pub const DEFAULT_SERVICE: &str = "com.onyx.platform";

pub struct KeyringSecureStorage {
    service: String,
    store: Arc<CredentialStore>,
}

impl KeyringSecureStorage {
    /// Platform store, scoped to `DEFAULT_SERVICE`.
    pub fn new() -> Result<Self, StorageError> {
        Self::for_service(DEFAULT_SERVICE)
    }

    /// Platform store, scoped to `service`. Two hosts installed on one device
    /// must use different service names, or one host's session overwrites the
    /// other's (both store the same keys).
    pub fn for_service(service: impl Into<String>) -> Result<Self, StorageError> {
        keyring::v1::Entry::store_status()
            .as_ref()
            .map_err(|e| StorageError::Platform(format!("credential store unavailable: {e}")))?;
        let store = keyring_core::get_default_store()
            .ok_or_else(|| StorageError::Platform("no default credential store".to_string()))?;
        Ok(Self {
            service: service.into(),
            store,
        })
    }

    /// Any credential store, scoped to `service`. Used by tests (mock store).
    pub fn with_store(service: impl Into<String>, store: Arc<CredentialStore>) -> Self {
        Self {
            service: service.into(),
            store,
        }
    }
}

#[async_trait]
impl SecureStorage for KeyringSecureStorage {
    async fn store_secret(&self, key: &str, value: &[u8]) -> Result<(), StorageError> {
        let (store, service, key, value) = (
            Arc::clone(&self.store),
            self.service.clone(),
            key.to_string(),
            value.to_vec(),
        );
        tokio::task::spawn_blocking(move || {
            let entry = store
                .build(&service, &key, None)
                .map_err(map_keyring_error)?;
            entry.set_secret(&value).map_err(map_keyring_error)
        })
        .await
        .map_err(|e| StorageError::Platform(format!("blocking task panicked: {e}")))?
    }

    async fn get_secret(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let (store, service, key) = (
            Arc::clone(&self.store),
            self.service.clone(),
            key.to_string(),
        );
        tokio::task::spawn_blocking(move || {
            let entry = store
                .build(&service, &key, None)
                .map_err(map_keyring_error)?;
            match entry.get_secret() {
                Ok(bytes) => Ok(Some(bytes)),
                Err(KeyringError::NoEntry) => Ok(None),
                Err(e) => Err(map_keyring_error(e)),
            }
        })
        .await
        .map_err(|e| StorageError::Platform(format!("blocking task panicked: {e}")))?
    }

    async fn delete_secret(&self, key: &str) -> Result<(), StorageError> {
        let (store, service, key) = (
            Arc::clone(&self.store),
            self.service.clone(),
            key.to_string(),
        );
        tokio::task::spawn_blocking(move || {
            let entry = store
                .build(&service, &key, None)
                .map_err(map_keyring_error)?;
            match entry.delete_credential() {
                Ok(()) => Ok(()),
                Err(KeyringError::NoEntry) => Err(StorageError::NotFound),
                Err(e) => Err(map_keyring_error(e)),
            }
        })
        .await
        .map_err(|e| StorageError::Platform(format!("blocking task panicked: {e}")))?
    }
}

/// Maps `keyring_core::Error` to `StorageError`, distinguishing the
/// frozen contract's `AccessDenied` (the credential store existing but
/// being inaccessible — e.g. locked, or a permission failure) from a
/// generic `Platform` failure, using `Error::NoStorageAccess` (the real
/// variant documented for exactly this case — "the credential store is
/// locked... the underlying platform error will typically give the
/// reason") rather than collapsing every non-`NoEntry` error into
/// `Platform` indiscriminately.
fn map_keyring_error(e: KeyringError) -> StorageError {
    match e {
        KeyringError::NoStorageAccess(inner) => StorageError::AccessDenied(inner.to_string()),
        other => StorageError::Platform(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each test gets its own in-memory store, so tests do not share state and
    /// do not depend on the machine's OS credential store.
    fn mock_store() -> Arc<CredentialStore> {
        keyring_core::mock::Store::new().expect("mock store")
    }

    fn test_key() -> String {
        format!("onyx-test-{}", uuid::Uuid::new_v4())
    }

    fn storage(service: &str) -> KeyringSecureStorage {
        KeyringSecureStorage::with_store(service, mock_store())
    }

    #[tokio::test]
    async fn store_then_get_then_delete_round_trips() {
        let storage = storage("com.onyx.platform.test");
        let key = test_key();

        storage
            .store_secret(&key, b"hello secret world")
            .await
            .unwrap();
        assert_eq!(
            storage.get_secret(&key).await.unwrap(),
            Some(b"hello secret world".to_vec())
        );

        storage.delete_secret(&key).await.unwrap();
        assert_eq!(storage.get_secret(&key).await.unwrap(), None);
    }

    #[tokio::test]
    async fn get_secret_for_unknown_key_returns_none_not_error() {
        let storage = storage("com.onyx.platform.test");
        assert_eq!(storage.get_secret(&test_key()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn delete_secret_for_unknown_key_returns_not_found() {
        let storage = storage("com.onyx.platform.test");
        let result = storage.delete_secret(&test_key()).await;
        assert!(matches!(result, Err(StorageError::NotFound)));
    }

    #[tokio::test]
    async fn services_are_isolated_from_each_other() {
        let store = mock_store();
        let key = test_key();
        let tauri = KeyringSecureStorage::with_store("com.onyx.platform", Arc::clone(&store));
        let slint = KeyringSecureStorage::with_store("com.onyx.platform.slint", store);

        tauri.store_secret(&key, b"tauri-session").await.unwrap();
        assert_eq!(slint.get_secret(&key).await.unwrap(), None);
        slint.store_secret(&key, b"slint-session").await.unwrap();
        assert_eq!(
            tauri.get_secret(&key).await.unwrap(),
            Some(b"tauri-session".to_vec())
        );
    }

    /// Real OS credential store. Device test only, on a machine with an
    /// unlocked keychain or Secret Service:
    /// `cargo test -p desktop-session --features keyring -- --ignored`
    #[tokio::test]
    #[ignore = "device test: needs an unlocked OS credential store"]
    async fn real_os_store_round_trips() {
        let storage = KeyringSecureStorage::for_service("com.onyx.platform.device-test").unwrap();
        let key = test_key();
        storage.store_secret(&key, b"device-secret").await.unwrap();
        assert_eq!(
            storage.get_secret(&key).await.unwrap(),
            Some(b"device-secret".to_vec())
        );
        storage.delete_secret(&key).await.unwrap();
    }
}
