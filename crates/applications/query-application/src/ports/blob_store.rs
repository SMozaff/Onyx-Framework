//! BlobStore port. Phase 1 (Desktop & Web Completion) addition.
//!
//! # Why this exists
//! `file_domain::UploadSession`/`FileAsset` are pure domain logic — they
//! validate chunk sequencing, sizes, and hashes (§4.8), but never see or
//! store actual file bytes, by design (no I/O in a domain crate, same as
//! every other aggregate in this workspace). Something has to actually
//! persist the bytes a real upload contains; nothing in the delivered
//! workspace did before this addition — confirmed by search, not
//! assumed, before adding this port (see `DECISIONS.md`).
//!
//! # Content-addressed by design
//! Keyed by [`ContentHash`], not by `FileAssetId`/chunk index — mirrors
//! `file_domain::value::ContentHash`'s own role as the thing
//! `AppendChunk`/`FinalizeUpload` actually validate against. This means
//! two uploads with identical content share one stored blob rather than
//! duplicating bytes on disk, and it keeps this port decoupled from
//! `file-domain`'s own aggregate ids (this crate does not depend on
//! `file-domain` at all — see [`ContentHash`]'s doc comment for why it's
//! redefined here rather than imported).

use async_trait::async_trait;

/// A content hash, formatted identically to how
/// `file_domain::value::ContentHash` validates one (`ContentHash::new`'s
/// own doc comment: a lowercase hex string). Redefined here as a plain
/// `String` newtype rather than depending on `file-domain` directly —
/// `query-application` is a shared application-layer crate every domain
/// eventually composes through, and domain crates deliberately do not
/// depend on each other (§5.2), so this port speaks in a
/// domain-crate-agnostic vocabulary and the composing handler is
/// responsible for using the same hash it already validated through
/// `file-domain`'s own `ContentHash::new`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlobKey(pub String);

impl BlobKey {
    pub fn new(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }
}

/// Stores and retrieves file content by its hash. Implementations must
/// be safe to call concurrently (`Send + Sync`, matching every other
/// port in this module) — a desktop client may have multiple uploads in
/// flight.
#[async_trait]
pub trait BlobStore: Send + Sync {
    /// Writes `content` under `key`. Idempotent: writing the same key
    /// twice with the same bytes must succeed both times (a retried
    /// `AppendChunk`/`FinalizeUpload` after a crash should not fail
    /// merely because the blob already exists — the domain-level
    /// idempotency this mirrors is `IdempotencyStore`'s own role for
    /// commands).
    async fn put(&self, key: &BlobKey, content: &[u8]) -> Result<(), BlobStoreError>;

    /// Reads back the full content previously stored under `key`.
    /// Returns `None` if no blob exists for that key.
    async fn get(&self, key: &BlobKey) -> Result<Option<Vec<u8>>, BlobStoreError>;

    /// Whether a blob exists for `key`, without reading its content.
    async fn exists(&self, key: &BlobKey) -> Result<bool, BlobStoreError>;

    /// Removes the blob stored under `key`, if any. Used when a
    /// `FileAsset`/`UploadSession` is archived or a quarantined upload is
    /// discarded — not currently called by anything (no Phase 1 UI
    /// workflow drives cleanup yet), included now so the port's shape
    /// doesn't need to change when that workflow is built.
    async fn delete(&self, key: &BlobKey) -> Result<(), BlobStoreError>;
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum BlobStoreError {
    #[error("blob storage I/O error: {0}")]
    Io(String),
    #[error("blob content did not match its declared hash")]
    HashMismatch,
}
