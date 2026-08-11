//! # file-domain
//!
//! ONYX FileAsset/UploadSession aggregates: pure domain logic for team
//! file sharing (Part I §4.8). No I/O, no persistence, no async — the
//! actual bytes never pass through this crate; it only tracks identity,
//! access, versions, and the chunk-accounting of an in-flight upload.
//!
//! The 100 MB per-file cap is enforced independently at both
//! `UploadSession::create` (the declared total) and every `AppendChunk`
//! (the running total), per product decision — see
//! [`value::MAX_FILE_SIZE_BYTES`].
//!
//! # Status
//! `FileAsset` and `UploadSession` are complete `AggregateRoot`
//! implementations with full state-transition test coverage — every valid
//! transition, every rejected one, both terminal states, and the size-cap
//! invariant at both enforcement points. Not yet wired into
//! `client-composition`'s command/query registries or any persistence
//! adapter — that integration work has not started.

#![deny(warnings)]
#![deny(missing_docs)]

pub mod aggregate;
pub mod command;
pub mod error;
pub mod event;
pub mod state_machine;
pub mod value;

/// Deterministic test fixtures, exposed unconditionally for the same
/// reason as `communication_domain::test_support` (see its docs) — other
/// crates' integration tests need to construct valid `FileAsset`/
/// `UploadSession` fixtures without duplicating this crate's construction
/// logic.
pub mod test_support;

pub use aggregate::{FileAsset, UploadSession};
pub use command::{FileAssetCommand, UploadSessionCommand};
pub use error::{FileAssetError, UploadSessionError};
pub use event::{FileAssetEvent, UploadSessionEvent};
pub use state_machine::{FileAssetStatus, UploadSessionStatus};
pub use value::{
    ContentHash, FileAssetId, FileName, FileVersionRecord, MimeType, RetentionClass,
    UploadSessionId, MAX_FILE_SIZE_BYTES,
};
