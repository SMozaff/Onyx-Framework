//! # platform-contracts-ext
//!
//! Local Team-3 copy of `platform-contracts`, additively extended per
//! Architectural Ruling Q2 (Team 3 Initiation — Contract Reconciliation)
//! with `AggregateRoot::conflict_pending()` and `ConflictPendingMarker`.
//! Otherwise identical to Team 1's delivered `platform-contracts`: same
//! `CommandEnvelope`, `DomainEventEnvelope`, `DomainError` protocol.
//! Depends only on `platform-kernel`.
//!
//! **Not the authoritative crate.** Team 1 owns `platform-contracts`; this
//! exists only because that crate doesn't yet have `conflict_pending()`.
//! See `DECISIONS.md` for the ruling.

#![deny(warnings)]
#![deny(missing_docs)]

pub mod command;
pub mod error;
pub mod event;
pub mod traits;

pub use command::CommandEnvelope;
pub use error::{DomainError, DomainErrorResponse, RepositoryError};
pub use event::{AuditMetadata, DataClassification, DomainEventEnvelope};
pub use traits::{AggregateRoot, ConflictPendingMarker, DecisionContext, IdGenerator};
