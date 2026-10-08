//! Errors produced by the FileAsset and UploadSession aggregates.

use serde::{Deserialize, Serialize};

/// An error produced while deciding or constructing a FileAsset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum FileAssetError {
    /// The command is not valid from the file asset's current status.
    #[error("Invalid file asset status transition: {0}")]
    InvalidTransition(String),

    /// A required field was missing or empty.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// The actor lacks the authority required for this command.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// The new version's declared size exceeds the platform's cap
    /// ([`crate::value::MAX_FILE_SIZE_BYTES`]).
    #[error("File version exceeds the maximum allowed size: {0}")]
    FileTooLarge(String),

    /// The user being granted/revoked access already has/lacks it.
    #[error("Invalid access grant state: {0}")]
    InvalidAccessState(String),
}

/// An error produced while deciding or constructing an UploadSession.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum UploadSessionError {
    /// The command is not valid from the session's current status.
    #[error("Invalid upload session status transition: {0}")]
    InvalidTransition(String),

    /// The declared or accumulated size exceeds the platform's cap
    /// ([`crate::value::MAX_FILE_SIZE_BYTES`]).
    #[error("Upload exceeds the maximum allowed size: {0}")]
    FileTooLarge(String),

    /// A chunk was appended out of order or with a duplicate index.
    #[error("Invalid chunk: {0}")]
    InvalidChunk(String),

    /// `FinalizeUpload` was called before all declared bytes arrived.
    #[error("Upload incomplete: {0}")]
    Incomplete(String),

    /// The actor lacks the authority required for this command.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
}
