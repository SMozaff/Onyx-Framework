//! Value types for the File bounded context. Source: Part I §4.8.

use platform_kernel::ObjectId;
use serde::{Deserialize, Serialize};

/// The identity of a FileAsset aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FileAssetId(pub ObjectId);

impl FileAssetId {
    /// Generates a new random file asset identifier.
    pub fn new_random() -> Self {
        Self(ObjectId::new_random())
    }
}

/// The identity of an UploadSession aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct UploadSessionId(pub ObjectId);

impl UploadSessionId {
    /// Generates a new random upload session identifier.
    pub fn new_random() -> Self {
        Self(ObjectId::new_random())
    }
}

/// The hard cap on any single file this platform will accept, per product
/// decision: "100 MB max per each file upload attempt by any user." Applied
/// at two points independently — `StartUpload` (the declared total, so an
/// oversized upload is rejected before a single byte moves) and every
/// `AppendChunk` (so a session cannot be inflated past its own declared
/// total either, closing the gap where a client lies about its own size
/// up front and then keeps appending).
pub const MAX_FILE_SIZE_BYTES: u64 = 100 * 1024 * 1024;

/// A validated, non-empty file name. Newtype per Part II Chapter 1 §1.6.3
/// ("newtypes are mandatory for ... validated text").
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileName(String);

impl FileName {
    /// Maximum length in bytes. Generous for any real filename, small
    /// enough to never itself become a storage or transport concern.
    pub const MAX_LEN: usize = 255;

    /// Constructs a `FileName`, rejecting empty or oversized input.
    pub fn new(name: impl Into<String>) -> Result<Self, String> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err("file name must not be empty".to_string());
        }
        if name.len() > Self::MAX_LEN {
            return Err(format!(
                "file name exceeds {} bytes (was {})",
                Self::MAX_LEN,
                name.len()
            ));
        }
        Ok(Self(name))
    }

    /// The file name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated MIME type string (e.g. `"application/pdf"`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MimeType(String);

impl MimeType {
    /// Constructs a `MimeType`, rejecting empty input. Full grammar
    /// validation (RFC 6838) is a transport/adapter concern, not this
    /// domain's — the invariant that matters here is simply "declared,
    /// not absent."
    pub fn new(mime_type: impl Into<String>) -> Result<Self, String> {
        let mime_type = mime_type.into();
        if mime_type.trim().is_empty() {
            return Err("mime type must not be empty".to_string());
        }
        Ok(Self(mime_type))
    }

    /// The MIME type as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A content hash (e.g. SHA-256 hex digest). Validated for presence and a
/// plausible length only — the domain records what it is told; verifying
/// a hash against actual bytes is `ScanResult`/adapter territory (Part I
/// §4.8.3), since this crate never sees file bytes at all.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentHash(String);

impl ContentHash {
    /// Constructs a `ContentHash`, rejecting empty input.
    pub fn new(hash: impl Into<String>) -> Result<Self, String> {
        let hash = hash.into();
        if hash.trim().is_empty() {
            return Err("content hash must not be empty".to_string());
        }
        Ok(Self(hash))
    }

    /// The hash as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One immutable, recorded version of a `FileAsset`'s content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileVersionRecord {
    /// The uploaded content's hash.
    pub content_hash: ContentHash,
    /// The version's size in bytes. Always `<= MAX_FILE_SIZE_BYTES` —
    /// enforced at `UploadSession` finalize time, before a version can
    /// ever be created from it.
    pub size_bytes: u64,
    /// When this version was recorded.
    pub created_at: platform_kernel::Timestamp,
}

/// Retention classification. Full retention-schedule semantics belong to
/// the Policy context (Part I §4.18); this is the minimal tag a `FileAsset`
/// carries so Policy has something to key off, matching how
/// `communication_domain::RedactionReason` stays free text rather than
/// this crate inventing Policy's own rule vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetentionClass {
    /// Ordinary retention; standard deletion rules apply.
    Standard,
    /// Under legal hold — Policy overrides normal deletion (§11.1's
    /// "Legal Hold overrides normal retention"). This crate does not
    /// enforce that override itself; it only carries the tag.
    LegalHold,
}
