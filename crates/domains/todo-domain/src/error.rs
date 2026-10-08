//! Errors produced by the `TodoList`, `TargetList`, and `StaffLoan`
//! aggregates.

use serde::{Deserialize, Serialize};

/// An error produced while deciding or constructing a `TodoList`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum TodoError {
    /// The command is not valid from the list's current status.
    #[error("Invalid todo list status transition: {0}")]
    InvalidTransition(String),

    /// A required field was missing or empty.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// The actor lacks the authority required for this command.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
}

/// An error produced while deciding or constructing a `TargetList`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum TargetError {
    /// The command is not valid from the list's current status.
    #[error("Invalid target list status transition: {0}")]
    InvalidTransition(String),

    /// A required field was missing or empty.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// The actor lacks the authority required for this command.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// `time_window.end_at` is not after `time_window.start_at`.
    #[error("time_window.end_at must be after time_window.start_at")]
    InvalidTimeWindow,

    /// `VerifyTargetList` was invoked before `time_window.end_at` was
    /// reached. Design doc §4.0.2, resolved 2026-08-16: hit/miss is
    /// judged once the window closes, not before — enforced here
    /// rather than left to caller discipline.
    #[error("Cannot verify a target before its time window has closed")]
    WindowNotClosed,
}

/// An error produced while deciding or constructing a `StaffLoan`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum StaffLoanError {
    /// The command is not valid from the loan's current status.
    #[error("Invalid staff loan status transition: {0}")]
    InvalidTransition(String),

    /// A required field was missing or empty.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// The actor lacks the authority required for this command.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// `window.end_at` is not after `window.start_at` at request time,
    /// or `ExtendStaffLoan`'s `new_end_at` is not after the loan's
    /// current `end_at`.
    #[error("Loan window end must be after its start")]
    InvalidLoanWindow,

    /// The staff member being loaned and the real owner (or borrowing
    /// manager) were the same user — a loan requires two distinct
    /// managers around one staff member, not a no-op.
    #[error("real_owner_id and borrowing_manager_id must differ")]
    OwnerAndBorrowerMustDiffer,
}
