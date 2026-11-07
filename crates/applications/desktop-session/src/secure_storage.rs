//! `SecureStorage` port. Platform adapters live in the host crate.
//! Frozen contract per Team Prompt 5 §3.1 (copied verbatim from the
//! handover; no deviations).

use async_trait::async_trait;

/// Store a secret under a given key (scoped to current user/organization).
/// Keys are strings (e.g., "auth.refresh_token", "sync.sync_key").
#[async_trait]
pub trait SecureStorage: Send + Sync {
    async fn store_secret(&self, key: &str, value: &[u8]) -> Result<(), StorageError>;

    /// Retrieve a secret; returns None if not found.
    async fn get_secret(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError>;

    /// Delete a secret.
    async fn delete_secret(&self, key: &str) -> Result<(), StorageError>;
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum StorageError {
    #[error("Key not found")]
    NotFound,
    #[error("Access denied: {0}")]
    AccessDenied(String),
    #[error("Platform error: {0}")]
    Platform(String),
}

/// Platform adapter (OS credential store). Enabled by the `keyring` feature.
#[cfg(feature = "keyring")]
pub mod keyring_adapter;
