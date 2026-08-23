//! The Mission aggregate root.
//!
//! Implements the full ruled lifecycle state machine. See `state_machine.rs`
//! for the `MissionStatus` enum and the workspace-root `DECISIONS.md` for
//! every architectural ruling this implementation follows.

use crate::command::MissionCommand;
use crate::error::MissionError;
use crate::event::MissionEvent;
use crate::state_machine::MissionStatus;
use crate::value::{MissionId, MissionSettings, MissionTimelineRef};
use platform_contracts::{AggregateRoot, DecisionContext, HasOwner};
use platform_kernel::{AuthorityEpoch, LifecycleEpoch, ObjectId, ObjectVersion, UserId};
use serde::{Deserialize, Serialize};

/// The Mission aggregate: the authoritative record of a mission's lifecycle.
///
/// `Serialize`/`Deserialize` added per Architectural Ruling ("Mission
/// Aggregate Serialization & Accessibility") — required so
/// `persistence-postgres`/`persistence-sqlite`'s `Repository`
/// implementations can round-trip aggregate state through
/// `serde_json::Value`, per the Team 1/Team 2 Interface Reconciliation
/// ruling. This is an amendment to Team 1's originally-delivered
/// `#[derive(Clone, Debug)]`; see DECISIONS.md item S1. All field types
/// already derived `Serialize`/`Deserialize` (verified before applying
/// this change), so no further amendments were needed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mission {
    id: MissionId,
    version: ObjectVersion,
    lifecycle_epoch: LifecycleEpoch,
    authority_epoch: AuthorityEpoch,
    status: MissionStatus,
    name: String,
    description: Option<String>,
    owner_id: UserId,
    /// The most recently created (not necessarily submitted) blueprint
    /// revision, if any.
    blueprint_revision_id: Option<ObjectId>,
    /// The blueprint revision that was actually submitted for planning,
    /// if any. Distinct from `blueprint_revision_id` because a mission may
    /// create several revisions before submitting one.
    submitted_blueprint_revision_id: Option<ObjectId>,
    /// Reference to the mission's timeline (owned by a separate bounded
    /// context introduced later). `None` in Increment 1 since no command
    /// populates it yet.
    #[allow(dead_code)]
    timeline_ref: Option<MissionTimelineRef>,
    /// Mission-level configuration. Defaulted at creation; no Increment 1
    /// command mutates it.
    #[allow(dead_code)]
    settings: MissionSettings,
}

impl Mission {
    /// Constructs a new Mission from a `CreateMission` command.
    ///
    /// # Increment 1 Ruling (C)
    /// Creation is not routed through `decide()`, which assumes an existing
    /// aggregate. This associated function is the sole entry point for
    /// mission creation; the future `Repository` (Increment 2) calls this
    /// when `load()` returns `None`.
    ///
    /// # Errors
    /// Returns [`MissionError::MissingField`] if `name` is empty, or
    /// [`MissionError::InvalidTransition`] if called with any command
    /// other than `CreateMission`.
    pub fn create(
        command: MissionCommand,
        context: &DecisionContext,
    ) -> Result<Vec<MissionEvent>, MissionError> {
        let MissionCommand::CreateMission { name, owner_id, .. } = command else {
            return Err(MissionError::InvalidTransition(
                "Mission::create() called with a non-CreateMission command".to_string(),
            ));
        };

        if name.trim().is_empty() {
            return Err(MissionError::MissingField("name".to_string()));
        }

        let mission_id = MissionId(context.generated_id_generator.generate_object_id());

        Ok(vec![MissionEvent::MissionCreated {
            mission_id,
            name,
            owner_id,
            created_at: context.trusted_now,
        }])
    }

    /// Rehydrates a `Mission` by folding its first event (`MissionCreated`),
    /// producing the initial aggregate state. All subsequent events are
    /// folded via [`AggregateRoot::apply`].
    ///
    /// # Panics
    /// Panics if `event` is not a `MissionCreated` event. Callers (the
    /// future `Repository`) are expected to always pass the first event of
    /// a mission's event stream, which is guaranteed by construction to be
    /// `MissionCreated`.
    pub fn from_created_event(event: &MissionEvent) -> Self {
        let MissionEvent::MissionCreated {
            mission_id,
            name,
            owner_id,
            ..
        } = event
        else {
            panic!("from_created_event called with a non-MissionCreated event");
        };

        Self {
            id: *mission_id,
            version: ObjectVersion::INITIAL,
            lifecycle_epoch: LifecycleEpoch::INITIAL,
            authority_epoch: AuthorityEpoch::INITIAL,
            status: MissionStatus::Draft,
            name: name.clone(),
            description: None,
            owner_id: *owner_id,
            blueprint_revision_id: None,
            submitted_blueprint_revision_id: None,
            timeline_ref: None,
            settings: MissionSettings::default(),
        }
    }

    /// The mission's current status.
    pub fn status(&self) -> MissionStatus {
        self.status
    }

    /// The mission's display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The mission's optional longer description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// The mission's owner.
    pub fn owner_id(&self) -> UserId {
        self.owner_id
    }

    /// The most recently created blueprint revision, if any.
    pub fn blueprint_revision_id(&self) -> Option<ObjectId> {
        self.blueprint_revision_id
    }

    fn invalid(&self, command: &str) -> MissionError {
        MissionError::InvalidTransition(format!("{} from {:?}", command, self.status))
    }
}

impl HasOwner for Mission {
    fn owner_id(&self) -> UserId {
        self.owner_id
    }
}

impl AggregateRoot for Mission {
    type Id = MissionId;
    type Command = MissionCommand;
    type Event = MissionEvent;
    type Error = MissionError;

    fn id(&self) -> &Self::Id {
        &self.id
    }

    fn version(&self) -> ObjectVersion {
        self.version
    }

    fn lifecycle_epoch(&self) -> LifecycleEpoch {
        self.lifecycle_epoch
    }

    fn authority_epoch(&self) -> AuthorityEpoch {
        self.authority_epoch
    }

    fn decide(
        &self,
        command: Self::Command,
        context: &DecisionContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        // Increment 1 stub: all authority checks pass (see
        // `platform_kernel::VerifiedAuthority` docs and ruling E).
        if !context.authority.is_authorized("mission.command") {
            return Err(MissionError::Unauthorized(
                "actor lacks required authority".to_string(),
            ));
        }

        match command {
            MissionCommand::CreateMission { .. } => Err(MissionError::InvalidTransition(
                "CreateMission must be dispatched via Mission::create(), not decide()".to_string(),
            )),

            MissionCommand::CreateBlueprintRevision {
                blueprint_data,
                version,
            } => {
                // Self-caught fix (property test
                // `archived_is_terminal_under_any_further_commands`): the
                // frozen contract specifies no explicit status guard for
                // this command, but allowing it unconditionally would let
                // a terminal-state mission (Closed/Archived/Cancelled)
                // accept new blueprint revisions, contradicting the
                // "Archived is terminal" intent (ruling A). Guarded to the
                // same non-terminal state set as CancelMission.
                match self.status {
                    MissionStatus::Draft
                    | MissionStatus::Planning
                    | MissionStatus::AwaitingApproval
                    | MissionStatus::Active
                    | MissionStatus::Paused
                    | MissionStatus::Halted
                    | MissionStatus::Review => {}
                    MissionStatus::Closed | MissionStatus::Archived | MissionStatus::Cancelled => {
                        return Err(self.invalid("CreateBlueprintRevision"));
                    }
                }
                if blueprint_data.trim().is_empty() {
                    return Err(MissionError::InvalidBlueprint(
                        "blueprint_data must not be empty".to_string(),
                    ));
                }
                // Never changes status by itself (ruling A): only
                // SubmitBlueprint advances Draft -> Planning.
                let revision_id = context.generated_id_generator.generate_object_id();
                Ok(vec![MissionEvent::MissionBlueprintRevisionCreated {
                    revision_id,
                    version,
                    created_at: context.trusted_now,
                }])
            }

            MissionCommand::SubmitBlueprint { revision_id } => {
                if self.status != MissionStatus::Draft {
                    return Err(self.invalid("SubmitBlueprint"));
                }
                match self.blueprint_revision_id {
                    Some(existing) if existing == revision_id => {
                        Ok(vec![MissionEvent::MissionBlueprintSubmitted {
                            revision_id,
                            submitted_at: context.trusted_now,
                        }])
                    }
                    Some(_) => Err(MissionError::InvalidBlueprint(
                        "revision_id does not match the mission's current blueprint revision"
                            .to_string(),
                    )),
                    None => Err(MissionError::InvalidBlueprint(
                        "no blueprint revision exists to submit".to_string(),
                    )),
                }
            }

            MissionCommand::RequestApproval { reason, evidence } => {
                if self.status != MissionStatus::Planning {
                    return Err(self.invalid("RequestApproval"));
                }
                if self.submitted_blueprint_revision_id.is_none() {
                    return Err(MissionError::InvalidBlueprint(
                        "cannot request approval without a submitted blueprint".to_string(),
                    ));
                }
                Ok(vec![MissionEvent::MissionApprovalRequested {
                    requested_at: context.trusted_now,
                    reason,
                    evidence,
                }])
            }

            MissionCommand::RejectApproval { reason } => {
                if self.status != MissionStatus::AwaitingApproval {
                    return Err(self.invalid("RejectApproval"));
                }
                Ok(vec![MissionEvent::MissionApprovalRejected {
                    rejected_at: context.trusted_now,
                    reason,
                }])
            }

            MissionCommand::ActivateMission { reason } => match self.status {
                // Planning -> Active guard "Approval received" is stubbed
                // true in Increment 1 (ruling A).
                MissionStatus::Planning
                | MissionStatus::AwaitingApproval
                | MissionStatus::Review => Ok(vec![MissionEvent::MissionActivated {
                    activated_at: context.trusted_now,
                    reason,
                }]),
                _ => Err(self.invalid("ActivateMission")),
            },

            MissionCommand::PauseMission { reason } => match self.status {
                MissionStatus::Planning | MissionStatus::Active => {
                    Ok(vec![MissionEvent::MissionPaused {
                        paused_at: context.trusted_now,
                        reason,
                    }])
                }
                _ => Err(self.invalid("PauseMission")),
            },

            MissionCommand::OperationalHaltMission {
                reason,
                emergency_code,
            } => match self.status {
                MissionStatus::Active | MissionStatus::Paused => {
                    if emergency_code.trim().is_empty() {
                        return Err(MissionError::MissingField("emergency_code".to_string()));
                    }
                    Ok(vec![MissionEvent::MissionOperationallyHalted {
                        halted_at: context.trusted_now,
                        reason,
                        emergency_code,
                    }])
                }
                _ => Err(self.invalid("OperationalHaltMission")),
            },

            MissionCommand::ResumeMission { reason } => match self.status {
                MissionStatus::Paused => Ok(vec![MissionEvent::MissionResumed {
                    resumed_at: context.trusted_now,
                    reason,
                }]),
                _ => Err(self.invalid("ResumeMission")),
            },

            MissionCommand::RestartMission {
                reason,
                checklist_completed,
            } => match self.status {
                // Ruling A: checklist_completed decides the resulting
                // status. Carried on the event itself (see event.rs docs)
                // so `apply()` remains pure.
                MissionStatus::Halted => Ok(vec![MissionEvent::MissionRestarted {
                    restarted_at: context.trusted_now,
                    reason,
                    checklist_completed,
                }]),
                _ => Err(self.invalid("RestartMission")),
            },

            MissionCommand::TriggerReview { reason } => match self.status {
                MissionStatus::Active => Ok(vec![MissionEvent::MissionReviewTriggered {
                    triggered_at: context.trusted_now,
                    reason,
                }]),
                _ => Err(self.invalid("TriggerReview")),
            },

            MissionCommand::CloseMission {
                reason,
                closure_evidence,
            } => match self.status {
                MissionStatus::Active | MissionStatus::Review => {
                    if closure_evidence.is_empty() {
                        return Err(MissionError::ClosureCriteriaNotMet(
                            "closure_evidence must not be empty".to_string(),
                        ));
                    }
                    Ok(vec![MissionEvent::MissionClosed {
                        closed_at: context.trusted_now,
                        reason,
                    }])
                }
                _ => Err(self.invalid("CloseMission")),
            },

            MissionCommand::ArchiveMission { reason } => match self.status {
                MissionStatus::Closed | MissionStatus::Cancelled => {
                    Ok(vec![MissionEvent::MissionArchived {
                        archived_at: context.trusted_now,
                        reason,
                    }])
                }
                _ => Err(self.invalid("ArchiveMission")),
            },

            MissionCommand::CancelMission { reason } => match self.status {
                MissionStatus::Draft
                | MissionStatus::Planning
                | MissionStatus::AwaitingApproval
                | MissionStatus::Active
                | MissionStatus::Paused
                | MissionStatus::Halted
                | MissionStatus::Review => Ok(vec![MissionEvent::MissionCancelled {
                    cancelled_at: context.trusted_now,
                    reason,
                }]),
                _ => Err(self.invalid("CancelMission")),
            },
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            MissionEvent::MissionCreated { .. } => {
                // Handled by `from_created_event`. Applying it again on an
                // already-constructed instance is a no-op by design
                // (idempotent replay safety).
            }
            MissionEvent::MissionBlueprintRevisionCreated { revision_id, .. } => {
                self.blueprint_revision_id = Some(*revision_id);
            }
            MissionEvent::MissionBlueprintSubmitted { revision_id, .. } => {
                self.submitted_blueprint_revision_id = Some(*revision_id);
                self.status = MissionStatus::Planning;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            MissionEvent::MissionApprovalRequested { .. } => {
                self.status = MissionStatus::AwaitingApproval;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            MissionEvent::MissionApprovalRejected { .. } => {
                self.status = MissionStatus::Planning;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            MissionEvent::MissionActivated { .. } => {
                self.status = MissionStatus::Active;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            MissionEvent::MissionPaused { .. } => {
                self.status = MissionStatus::Paused;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            MissionEvent::MissionOperationallyHalted { .. } => {
                self.status = MissionStatus::Halted;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            MissionEvent::MissionResumed { .. } => {
                self.status = MissionStatus::Active;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            MissionEvent::MissionRestarted {
                checklist_completed,
                ..
            } => {
                self.status = if *checklist_completed {
                    MissionStatus::Active
                } else {
                    MissionStatus::Paused
                };
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            MissionEvent::MissionReviewTriggered { .. } => {
                self.status = MissionStatus::Review;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            MissionEvent::MissionClosed { .. } => {
                self.status = MissionStatus::Closed;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            MissionEvent::MissionArchived { .. } => {
                self.status = MissionStatus::Archived;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            MissionEvent::MissionCancelled { .. } => {
                self.status = MissionStatus::Cancelled;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
        }
        self.version = self.version.next();
    }
}
