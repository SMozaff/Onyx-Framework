//! Errors produced by the Policy and LegalHold aggregates.

use serde::{Deserialize, Serialize};

/// An error produced while deciding or constructing a Policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum PolicyError {
    /// The command is not valid from the policy's current status.
    #[error("Invalid policy status transition: {0}")]
    InvalidTransition(String),

    /// A required field was missing or empty.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// The actor lacks the authority required for this command.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// `PublishPolicyVersion` was called with no draft version pending.
    #[error("No draft version to publish")]
    NoDraftVersion,

    /// `CreatePolicyVersion` was called while a draft is already
    /// pending. §4.18.4 invariant 111 ("concurrent policy revisions
    /// require controlled publication") — a second concurrent draft is
    /// exactly the uncontrolled-concurrency case that invariant rules
    /// out; the existing draft must be published or explicitly
    /// discarded (not modeled in Phase 1 — see crate-level docs) first.
    #[error("A draft version is already pending publication")]
    DraftAlreadyPending,

    /// `EvaluatePolicy`/`RegisterViolation` was called with no published
    /// version to evaluate against.
    #[error("No published version exists to evaluate")]
    NoPublishedVersion,
}

/// An error produced while deciding or constructing a LegalHold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum LegalHoldError {
    /// The command is not valid from the hold's current status.
    #[error("Invalid legal hold status transition: {0}")]
    InvalidTransition(String),

    /// A required field was missing or empty.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// The actor lacks the authority required for this command.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
}
