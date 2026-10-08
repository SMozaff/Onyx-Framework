//! Errors produced by the Mission aggregate.

use serde::{Deserialize, Serialize};

/// An error produced while deciding or constructing a Mission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum MissionError {
    /// The command is not valid from the mission's current status.
    #[error("Invalid mission status transition: {0}")]
    InvalidTransition(String),

    /// A required field was missing or empty.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// The actor lacks the authority required for this command.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Closure was requested but closure criteria were not satisfied.
    #[error("Closure criteria not met: {0}")]
    ClosureCriteriaNotMet(String),

    /// The supplied blueprint failed validation.
    #[error("Blueprint invalid: {0}")]
    InvalidBlueprint(String),
}
