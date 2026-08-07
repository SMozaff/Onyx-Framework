//! `SecureStorage` port — frozen contract per Team Prompt 5 §3.1.
//! Copied verbatim (the prompt states "These definitions are copied
//! verbatim from the Handover Document. You MUST implement them exactly
//! as specified. No deviations are permitted.").
//!
//! # Implementation (ruling S2, `DECISIONS.md`)
//! Team Prompt 5 §2.1/§5.1 specifies three separate hand-written
//! per-OS adapter files/dependencies (`security-framework` for macOS,
//! `windows` for Windows, `secret-service` for Linux). Per ruling S2,
//! these were replaced with a single implementation
//! (`keyring_adapter.rs`) backed by the `keyring` crate (v4.x,
//! `v1` feature — its simple, direct `Entry`-based API), which wraps all
//! three platforms' native credential stores behind one safe, actively
//! maintained interface. This is a permitted implementation detail, not
//! an architectural deviation — the `SecureStorage` port itself is
//! unchanged.

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

pub mod keyring_adapter;
