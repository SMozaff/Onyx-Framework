//! Events produced by the Mission aggregate.

use crate::value::MissionId;
use platform_kernel::{ObjectId, Timestamp, UserId};
use serde::{Deserialize, Serialize};

/// An event recording a fact that occurred to a Mission aggregate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MissionEvent {
    /// A new mission was created.
    MissionCreated {
        /// The new mission's identifier.
        mission_id: MissionId,
        /// The mission's display name.
        name: String,
        /// The mission's initial owner.
        owner_id: UserId,
        /// When the mission was created.
        created_at: Timestamp,
    },
    /// A blueprint revision was created (mission remains in `Draft`).
    MissionBlueprintRevisionCreated {
        /// The new revision's identifier.
        revision_id: ObjectId,
        /// The blueprint's version label.
        version: String,
        /// When the revision was created.
        created_at: Timestamp,
    },
    /// A blueprint was submitted, advancing `Draft → Planning`.
    MissionBlueprintSubmitted {
        /// The submitted revision's identifier.
        revision_id: ObjectId,
        /// When the blueprint was submitted.
        submitted_at: Timestamp,
    },
    /// The mission's approval was submitted for review, advancing
    /// `Planning → AwaitingApproval`.
    ///
    /// # Increment 1 Ruling (M1) — naming correction
    /// The ruling's illustrative test reused `MissionBlueprintSubmitted`
    /// for this transition, but that event already denotes the distinct
    /// `Draft → Planning` transition (`SubmitBlueprint`). Reusing one event
    /// name for two different transitions would make `apply()` ambiguous
    /// during event-sourced replay, so a dedicated event is used here
    /// instead. Substance of the ruling (command names, transition table)
    /// is unchanged.
    MissionApprovalRequested {
        /// When approval was requested.
        requested_at: Timestamp,
        /// The reason approval was requested.
        reason: String,
        /// Supporting evidence supplied with the request.
        evidence: Vec<ObjectId>,
    },
    /// A pending approval request was rejected, returning the mission to
    /// `Planning` (`AwaitingApproval → Planning`).
    MissionApprovalRejected {
        /// When the rejection occurred.
        rejected_at: Timestamp,
        /// The reason approval was rejected.
        reason: String,
    },
    /// The mission was activated.
    MissionActivated {
        /// When the mission was activated.
        activated_at: Timestamp,
        /// The reason for activation.
        reason: String,
    },
    /// The mission was paused.
    MissionPaused {
        /// When the mission was paused.
        paused_at: Timestamp,
        /// The reason for pausing.
        reason: String,
    },
    /// The mission was emergency-halted.
    MissionOperationallyHalted {
        /// When the halt occurred.
        halted_at: Timestamp,
        /// The reason for the halt.
        reason: String,
        /// The emergency authorization code used.
        emergency_code: String,
    },
    /// The mission was resumed from a paused state.
    MissionResumed {
        /// When the mission was resumed.
        resumed_at: Timestamp,
        /// The reason for resuming.
        reason: String,
    },
    /// The mission was restarted from a halted state.
    ///
    /// # Increment 1 Ruling (A)
    /// Carries `checklist_completed` (copied from the command) so that
    /// `apply()` can determine the resulting status (`Active` if
    /// completed, `Paused` otherwise) purely from the event, without
    /// depending on any transient aggregate state.
    MissionRestarted {
        /// When the restart occurred.
        restarted_at: Timestamp,
        /// The reason for the restart.
        reason: String,
        /// Whether the emergency restart checklist was completed at the
        /// time of restart. Determines whether the mission lands in
        /// `Active` (`true`) or `Paused` (`false`).
        checklist_completed: bool,
    },
    /// A formal review of the mission was triggered.
    ///
    /// # Increment 1 Ruling (A)
    /// Added alongside `MissionCommand::TriggerReview` to make the
    /// `Active → Review` transition reachable.
    MissionReviewTriggered {
        /// When the review was triggered.
        triggered_at: Timestamp,
        /// The reason the review was triggered.
        reason: String,
    },
    /// The mission was closed.
    MissionClosed {
        /// When the mission was closed.
        closed_at: Timestamp,
        /// The reason for closure.
        reason: String,
    },
    /// The mission was archived.
    MissionArchived {
        /// When the mission was archived.
        archived_at: Timestamp,
        /// The reason for archival.
        reason: String,
    },
    /// The mission was cancelled.
    MissionCancelled {
        /// When the mission was cancelled.
        cancelled_at: Timestamp,
        /// The reason for cancellation.
        reason: String,
    },
}
