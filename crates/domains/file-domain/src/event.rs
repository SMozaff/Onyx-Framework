//! Domain events emitted by the FileAsset and UploadSession aggregates.
//! Source: Part I §4.8.5.

use crate::value::{ContentHash, FileAssetId, UploadSessionId};
use platform_kernel::{Timestamp, UserId};
use serde::{Deserialize, Serialize};

/// Events emitted by the FileAsset aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FileAssetEvent {
    /// A new file asset was created.
    FileAssetCreated {
        /// The new asset's identity.
        file_asset_id: FileAssetId,
        /// The declared file name.
        file_name: String,
        /// The declared MIME type.
        mime_type: String,
        /// The creating (and initially only-authorized) actor.
        owner_id: UserId,
        /// When it was created.
        created_at: Timestamp,
    },
    /// A new version's content was recorded.
    FileVersionCreated {
        /// The uploaded content's hash.
        content_hash: ContentHash,
        /// The version's size in bytes.
        size_bytes: u64,
        /// When it was recorded.
        created_at: Timestamp,
    },
    /// Access was granted to a user.
    FileAccessGranted {
        /// The user granted access.
        user_id: UserId,
        /// When.
        granted_at: Timestamp,
    },
    /// Access was revoked from a user.
    FileAccessRevoked {
        /// The user whose access was revoked.
        user_id: UserId,
        /// When.
        revoked_at: Timestamp,
    },
    /// The file was quarantined.
    FileQuarantined {
        /// Why.
        reason: String,
        /// When.
        quarantined_at: Timestamp,
    },
    /// The file was archived.
    FileArchived {
        /// Why, if given.
        reason: Option<String>,
        /// When.
        archived_at: Timestamp,
    },
}

/// Events emitted by the UploadSession aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UploadSessionEvent {
    /// A new upload session was started.
    UploadStarted {
        /// The new session's identity.
        upload_session_id: UploadSessionId,
        /// The file asset this upload is a version of.
        file_asset_id: FileAssetId,
        /// The declared total size in bytes.
        declared_total_size: u64,
        /// When it was started.
        started_at: Timestamp,
    },
    /// A chunk was accepted.
    ChunkAccepted {
        /// The chunk's position in sequence.
        chunk_index: u32,
        /// The chunk's size in bytes.
        chunk_size: u64,
        /// The chunk's own content hash.
        chunk_hash: ContentHash,
        /// When.
        accepted_at: Timestamp,
    },
    /// The upload was finalized.
    UploadFinalized {
        /// The full content's hash, over all chunks.
        final_hash: ContentHash,
        /// When.
        finalized_at: Timestamp,
    },
}
