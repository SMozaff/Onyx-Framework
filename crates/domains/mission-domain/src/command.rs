//! Commands accepted by the Mission aggregate.
//!
//! # Increment 1 Ruling (A)
//! `TriggerReview` is added to reach the `Active → Review` transition,
//! which the frozen Team Prompt's `MissionCommand` enum could not reach.
//! `CreateMission` is included for completeness but is dispatched via
//! `Mission::create()`, not `Mission::decide()` — see ruling C.

use platform_kernel::{ObjectId, UserId};
use serde::{Deserialize, Serialize};

/// A command targeting the Mission aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MissionCommand {
    /// Creates a new mission. Dispatched via `Mission::create()`, not
    /// `decide()` — see ruling C.
    CreateMission {
        /// The mission's display name.
        name: String,
        /// An optional longer description.
        description: Option<String>,
        /// The mission's initial owner.
        owner_id: UserId,
    },
    /// Creates a new blueprint revision. Does not itself change status
    /// (see ruling A: only `SubmitBlueprint` advances `Draft → Planning`).
    CreateBlueprintRevision {
        /// The raw blueprint content.
        blueprint_data: String,
        /// The blueprint's version label.
        version: String,
    },
    /// Submits a blueprint revision for planning, advancing the mission
    /// from `Draft` to `Planning`.
    SubmitBlueprint {
        /// The blueprint revision being submitted.
        revision_id: ObjectId,
    },
    /// Submits the mission for formal approval review, advancing
    /// `Planning → AwaitingApproval`.
    ///
    /// # Increment 1 Ruling (M1)
    /// Added to reach a transition ("Blueprint and timeline sufficient")
    /// present in the state table but unreachable by the originally ruled
    /// command set.
    RequestApproval {
        /// The reason approval is being requested.
        reason: String,
        /// Optional supporting evidence for the approval request.
        evidence: Vec<ObjectId>,
    },
    /// Rejects a pending approval request, returning the mission to
    /// `Planning` (`AwaitingApproval → Planning`).
    ///
    /// # Increment 1 Ruling (M1)
    RejectApproval {
        /// The reason approval was rejected.
        reason: String,
    },
    /// Activates the mission (`Planning`/`AwaitingApproval`/`Halted`/
    /// `Paused`/`Review` → `Active`, depending on current state).
    ActivateMission {
        /// The reason for activation.
        reason: String,
    },
    /// Pauses the mission (`Planning`/`Active` → `Paused`).
    PauseMission {
        /// The reason for pausing.
        reason: String,
    },
    /// Emergency-halts the mission (`Active`/`Paused` → `Halted`).
    OperationalHaltMission {
        /// The reason for the halt.
        reason: String,
        /// An emergency authorization code, subject to additional
        /// validation.
        emergency_code: String,
    },
    /// Resumes the mission (`Paused` → `Active`).
    ResumeMission {
        /// The reason for resuming.
        reason: String,
    },
    /// Restarts the mission from a halted state.
    ///
    /// # Increment 1 Ruling (A)
    /// If `checklist_completed` is `true`, transitions `Halted → Active`.
    /// If `false`, transitions `Halted → Paused` (the mission must then be
    /// separately resumed via `ResumeMission`).
    RestartMission {
        /// The reason for the restart.
        reason: String,
        /// Whether the emergency restart checklist has been completed.
        checklist_completed: bool,
    },
    /// Triggers a formal review (`Active → Review`).
    ///
    /// # Increment 1 Ruling (A)
    /// Added to reach a transition the frozen contract's command set could
    /// not otherwise reach.
    TriggerReview {
        /// The reason the review was triggered.
        reason: String,
    },
    /// Closes the mission (`Active`/`Review` → `Closed`).
    CloseMission {
        /// The reason for closure.
        reason: String,
        /// References to evidence supporting closure criteria.
        closure_evidence: Vec<ObjectId>,
    },
    /// Archives the mission (`Closed`/`Cancelled` → `Archived`).
    ArchiveMission {
        /// The reason for archival.
        reason: String,
    },
    /// Cancels the mission.
    CancelMission {
        /// The reason for cancellation.
        reason: String,
    },
}
