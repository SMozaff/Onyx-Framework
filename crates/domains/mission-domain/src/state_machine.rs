//! The Mission lifecycle state machine.
//!
//! See the Team Prompt §4.2 (transition table) as amended by the Increment 1
//! rulings recorded in `DECISIONS.md` at the workspace root. In particular:
//! `Archived → Closed` (restore) is out of scope for Increment 1 and has no
//! reachable transition; `Cancelled → Archived` is included per the
//! Handover Document (ruled authoritative over the Team Prompt's omission).

use serde::{Deserialize, Serialize};

/// The lifecycle status of a Mission aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionStatus {
    /// Newly created; blueprint not yet submitted.
    Draft,
    /// Blueprint submitted; awaiting activation or approval.
    Planning,
    /// Blueprint submitted for formal approval.
    AwaitingApproval,
    /// Mission is actively executing.
    Active,
    /// Mission execution is temporarily suspended.
    Paused,
    /// Mission execution has been emergency-halted.
    Halted,
    /// Mission is undergoing formal review.
    Review,
    /// Mission has been closed out.
    Closed,
    /// Mission has been archived (post-closure retention state).
    Archived,
    /// Mission has been cancelled.
    Cancelled,
}

impl std::fmt::Display for MissionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
