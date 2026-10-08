//! Commands accepted by the FileAsset and UploadSession aggregates.
//! Source: Part I §4.8.5.

use crate::value::FileAssetId;
use platform_kernel::UserId;
use serde::{Deserialize, Serialize};

/// Commands accepted by the FileAsset aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FileAssetCommand {
    /// Create a new file asset. Routed through `FileAsset::create()`, not
    /// `decide()` — same rationale as `communication_domain`'s
    /// `CreateConversation`/`PostMessage` (Increment 1 ruling C).
    CreateFileAsset {
        /// The declared file name.
        file_name: String,
        /// The declared MIME type.
        mime_type: String,
    },
    /// Record a new version's content, once an `UploadSession` targeting
    /// this asset has finalized.
    CreateVersion {
        /// The uploaded content's hash.
        content_hash: String,
        /// The version's size in bytes.
        size_bytes: u64,
    },
    /// Grant a user access to this file.
    GrantFileAccess {
        /// The user to grant access to.
        user_id: UserId,
    },
    /// Revoke a user's access to this file.
    RevokeFileAccess {
        /// The user to revoke access from.
        user_id: UserId,
    },
    /// Quarantine the file pending scan/policy review.
    QuarantineFile {
        /// Why.
        reason: String,
    },
    /// Archive the file.
    ArchiveFile {
        /// Optional reason.
        reason: Option<String>,
    },
}

/// Commands accepted by the UploadSession aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UploadSessionCommand {
    /// Start a new upload session. Routed through
    /// `UploadSession::create()`, same rationale as `CreateFileAsset`
    /// above.
    StartUpload {
        /// The file asset this upload will become a version of.
        file_asset_id: FileAssetId,
        /// The declared total size in bytes. Rejected outright if it
        /// exceeds [`crate::value::MAX_FILE_SIZE_BYTES`].
        total_size: u64,
    },
    /// Append one chunk of the upload.
    AppendChunk {
        /// The chunk's position in sequence. Must not repeat or skip
        /// ahead of the next expected index.
        chunk_index: u32,
        /// The chunk's size in bytes.
        chunk_size: u64,
        /// The chunk's own content hash.
        chunk_hash: String,
    },
    /// Finalize the upload once all declared bytes have arrived.
    FinalizeUpload {
        /// The full content's hash, over all chunks.
        final_hash: String,
    },
}
