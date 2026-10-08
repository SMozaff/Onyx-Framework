//! Errors produced by the StaffProfile aggregate.

use serde::{Deserialize, Serialize};

/// An error produced while deciding or constructing a `StaffProfile`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum ProfileError {
    /// The command is not valid from the profile's current status.
    #[error("Invalid profile status transition: {0}")]
    InvalidTransition(String),

    /// A required field was missing or empty.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// The actor lacks the authority required for this command. Per
    /// the confirmed product decision, every mutating command on this
    /// aggregate is Admin-only — no exceptions for the profile's own
    /// owner. `decide()` checks the generic
    /// `context.authority.is_authorized(...)` stub, same as every other
    /// aggregate in this workspace; real per-role enforcement
    /// (specifically requiring `is_admin`, not merely "authorized") is
    /// the composing application's responsibility, matching how every
    /// other domain in this workspace defers real authority to the
    /// composition root.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
}
