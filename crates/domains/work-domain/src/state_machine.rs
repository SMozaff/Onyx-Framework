//! The Task lifecycle state machine.
//!
//! See the workspace-root `DECISIONS.md` for the ruled transition table,
//! including ruling B (added commands: `MarkReady`, `ResumeTask`,
//! `UnblockTask`, `RejectTask`) and ruling T1 (reinstated `Reopened`
//! status per the Handover Document, with `Reopened → Active` via
//! `StartTask` and `Reopened → Cancelled` via `CancelTask`).

use serde::{Deserialize, Serialize};

/// The lifecycle status of a Task aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Newly created; not yet ready for assignment/execution.
    Draft,
    /// Ready to be started.
    Ready,
    /// Actively being worked.
    Active,
    /// Temporarily paused.
    Paused,
    /// Blocked on an external dependency.
    Blocked,
    /// Completion submitted; awaiting review.
    Submitted,
    /// Completion approved.
    Approved,
    /// Closed out.
    Closed,
    /// Cancelled.
    Cancelled,
    /// A closed task that has been reopened for further work.
    ///
    /// # Increment 1 Ruling (T1)
    /// Reinstated per the Handover Document, which is authoritative.
    Reopened,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
