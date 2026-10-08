//! Errors produced by the Task aggregate.

use serde::{Deserialize, Serialize};

/// An error produced while deciding or constructing a Task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum TaskError {
    /// The command is not valid from the task's current status.
    #[error("Invalid task status transition: {0}")]
    InvalidTransition(String),

    /// A required field was missing or empty.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// The actor lacks the authority required for this command.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// The dependency being added is invalid (self-dependency or
    /// duplicate). Full acyclicity checking is deferred to a later
    /// increment (ruling B).
    #[error("Invalid dependency: {0}")]
    InvalidDependency(String),

    /// Completion was submitted without required evidence.
    #[error("Completion evidence missing: {0}")]
    MissingCompletionEvidence(String),
}
