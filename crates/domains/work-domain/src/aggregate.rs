//! The Task aggregate root.
//!
//! Implements the full ruled lifecycle state machine. See `state_machine.rs`
//! for the `TaskStatus` enum and the workspace-root `DECISIONS.md` for every
//! architectural ruling this implementation follows (rulings B and T1 in
//! particular).

use crate::command::TaskCommand;
use crate::error::TaskError;
use crate::event::TaskEvent;
use crate::state_machine::TaskStatus;
use crate::value::{Dependency, TaskId, TaskMissionRef, TaskPriority};
use platform_contracts::{AggregateRoot, DecisionContext};
use platform_kernel::{AuthorityEpoch, LifecycleEpoch, ObjectVersion, UserId};
use serde::{Deserialize, Serialize};

/// The Task aggregate: the authoritative record of a task's lifecycle.
///
/// `Serialize`/`Deserialize` added per Architectural Ruling ("Mission
/// Aggregate Serialization & Accessibility") — same rationale as
/// `mission_domain::Mission`; see DECISIONS.md item S1.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    id: TaskId,
    version: ObjectVersion,
    lifecycle_epoch: LifecycleEpoch,
    authority_epoch: AuthorityEpoch,
    status: TaskStatus,
    mission_id: TaskMissionRef,
    title: String,
    description: Option<String>,
    owner_id: UserId,
    priority: TaskPriority,
    dependencies: Vec<Dependency>,
}

impl Task {
    /// Constructs a new Task from a `CreateTask` command.
    ///
    /// # Increment 1 Ruling
    /// Mirrors ruling C for Mission: creation is not routed through
    /// `decide()`, which assumes an existing aggregate.
    ///
    /// # Errors
    /// Returns [`TaskError::MissingField`] if `title` is empty, or
    /// [`TaskError::InvalidTransition`] if called with any command other
    /// than `CreateTask`.
    pub fn create(
        command: TaskCommand,
        context: &DecisionContext,
    ) -> Result<Vec<TaskEvent>, TaskError> {
        let TaskCommand::CreateTask {
            mission_id,
            title,
            owner_id,
            ..
        } = command
        else {
            return Err(TaskError::InvalidTransition(
                "Task::create() called with a non-CreateTask command".to_string(),
            ));
        };

        if title.trim().is_empty() {
            return Err(TaskError::MissingField("title".to_string()));
        }

        let task_id = TaskId(context.generated_id_generator.generate_object_id());

        Ok(vec![TaskEvent::TaskCreated {
            task_id,
            mission_id,
            title,
            owner_id,
            created_at: context.trusted_now,
        }])
    }

    /// Rehydrates a `Task` by folding its first event (`TaskCreated`).
    ///
    /// # Panics
    /// Panics if `event` is not a `TaskCreated` event.
    pub fn from_created_event(event: &TaskEvent) -> Self {
        let TaskEvent::TaskCreated {
            task_id,
            mission_id,
            title,
            owner_id,
            ..
        } = event
        else {
            panic!("from_created_event called with a non-TaskCreated event");
        };

        Self {
            id: *task_id,
            version: ObjectVersion::INITIAL,
            lifecycle_epoch: LifecycleEpoch::INITIAL,
            authority_epoch: AuthorityEpoch::INITIAL,
            status: TaskStatus::Draft,
            mission_id: *mission_id,
            title: title.clone(),
            description: None,
            owner_id: *owner_id,
            priority: TaskPriority::default(),
            dependencies: Vec::new(),
        }
    }

    /// The task's current status.
    pub fn status(&self) -> TaskStatus {
        self.status
    }

    /// The task's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The task's optional longer description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// The task's current owner.
    pub fn owner_id(&self) -> UserId {
        self.owner_id
    }

    /// The task's current priority.
    pub fn priority(&self) -> TaskPriority {
        self.priority
    }

    /// The mission this task belongs to.
    pub fn mission_id(&self) -> TaskMissionRef {
        self.mission_id
    }

    /// The dependencies currently recorded on this task.
    pub fn dependencies(&self) -> &[Dependency] {
        &self.dependencies
    }

    fn invalid(&self, command: &str) -> TaskError {
        TaskError::InvalidTransition(format!("{} from {:?}", command, self.status))
    }

    /// Whether self-loop commands (`AssignOwner`, `ChangePriority`,
    /// `AddDependency`) are permitted from the task's current status.
    ///
    /// # Increment 1 Ruling (B, extended)
    /// Ruled: `Draft`, `Ready`, `Active` only. Extended (self-caught,
    /// analogical) to include `Reopened`, since it is functionally an
    /// in-flight, resumable state equivalent to the ruled three; work has
    /// not yet been re-submitted for review.
    fn self_loop_commands_permitted(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Draft | TaskStatus::Ready | TaskStatus::Active | TaskStatus::Reopened
        )
    }
}

impl AggregateRoot for Task {
    type Id = TaskId;
    type Command = TaskCommand;
    type Event = TaskEvent;
    type Error = TaskError;

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
        // Increment 1 stub: all authority checks pass (ruling E).
        if !context.authority.is_authorized("task.command") {
            return Err(TaskError::Unauthorized(
                "actor lacks required authority".to_string(),
            ));
        }

        match command {
            TaskCommand::CreateTask { .. } => Err(TaskError::InvalidTransition(
                "CreateTask must be dispatched via Task::create(), not decide()".to_string(),
            )),

            TaskCommand::MarkReady { reason } => match self.status {
                TaskStatus::Draft => Ok(vec![TaskEvent::TaskMarkedReady {
                    marked_ready_at: context.trusted_now,
                    reason,
                }]),
                _ => Err(self.invalid("MarkReady")),
            },

            TaskCommand::AssignOwner { owner_id, reason } => {
                if !self.self_loop_commands_permitted() {
                    return Err(self.invalid("AssignOwner"));
                }
                Ok(vec![TaskEvent::TaskOwnerAssigned {
                    owner_id,
                    assigned_at: context.trusted_now,
                    reason,
                }])
            }

            TaskCommand::ChangePriority { priority, reason } => {
                if !self.self_loop_commands_permitted() {
                    return Err(self.invalid("ChangePriority"));
                }
                Ok(vec![TaskEvent::TaskPriorityChanged {
                    priority,
                    changed_at: context.trusted_now,
                    reason,
                }])
            }

            TaskCommand::AddDependency { dependency } => {
                if !self.self_loop_commands_permitted() {
                    return Err(self.invalid("AddDependency"));
                }
                // Ruling B: Increment 1 rejects only self-dependency and
                // exact duplicates. Full acyclicity is deferred (a single
                // aggregate cannot see the full dependency graph).
                if dependency.task_id == self.id {
                    return Err(TaskError::InvalidDependency(
                        "a task cannot depend on itself".to_string(),
                    ));
                }
                if self.dependencies.contains(&dependency) {
                    return Err(TaskError::InvalidDependency(
                        "duplicate dependency".to_string(),
                    ));
                }
                Ok(vec![TaskEvent::TaskDependencyAdded {
                    dependency,
                    added_at: context.trusted_now,
                }])
            }

            TaskCommand::StartTask { reason } => match self.status {
                // Reopened -> Active per ruling T1.
                TaskStatus::Ready | TaskStatus::Reopened => Ok(vec![TaskEvent::TaskStarted {
                    started_at: context.trusted_now,
                    reason,
                }]),
                _ => Err(self.invalid("StartTask")),
            },

            TaskCommand::PauseTask { reason } => match self.status {
                TaskStatus::Active => Ok(vec![TaskEvent::TaskPaused {
                    paused_at: context.trusted_now,
                    reason,
                }]),
                _ => Err(self.invalid("PauseTask")),
            },

            TaskCommand::BlockTask {
                reason,
                blocking_ref,
            } => match self.status {
                TaskStatus::Active => Ok(vec![TaskEvent::TaskBlocked {
                    blocked_at: context.trusted_now,
                    reason,
                    blocking_ref,
                }]),
                _ => Err(self.invalid("BlockTask")),
            },

            TaskCommand::ResumeTask { reason } => match self.status {
                TaskStatus::Paused => Ok(vec![TaskEvent::TaskResumed {
                    resumed_at: context.trusted_now,
                    reason,
                }]),
                _ => Err(self.invalid("ResumeTask")),
            },

            TaskCommand::UnblockTask { reason } => match self.status {
                TaskStatus::Blocked => Ok(vec![TaskEvent::TaskUnblocked {
                    unblocked_at: context.trusted_now,
                    reason,
                }]),
                _ => Err(self.invalid("UnblockTask")),
            },

            TaskCommand::SubmitCompletion { evidence } => match self.status {
                TaskStatus::Active => {
                    if evidence.is_empty() {
                        return Err(TaskError::MissingCompletionEvidence(
                            "evidence must not be empty".to_string(),
                        ));
                    }
                    Ok(vec![TaskEvent::TaskCompletionSubmitted {
                        submitted_at: context.trusted_now,
                        evidence,
                    }])
                }
                _ => Err(self.invalid("SubmitCompletion")),
            },

            TaskCommand::RejectTask { reason } => match self.status {
                TaskStatus::Submitted => Ok(vec![TaskEvent::TaskRejected {
                    rejected_at: context.trusted_now,
                    reason,
                }]),
                _ => Err(self.invalid("RejectTask")),
            },

            TaskCommand::ApproveTask { reason } => match self.status {
                TaskStatus::Submitted => Ok(vec![TaskEvent::TaskApproved {
                    approved_at: context.trusted_now,
                    reason,
                }]),
                _ => Err(self.invalid("ApproveTask")),
            },

            TaskCommand::CloseTask { reason } => match self.status {
                TaskStatus::Approved => Ok(vec![TaskEvent::TaskClosed {
                    closed_at: context.trusted_now,
                    reason,
                }]),
                _ => Err(self.invalid("CloseTask")),
            },

            TaskCommand::ReopenTask {
                reason,
                authorized_by,
            } => match self.status {
                TaskStatus::Closed => Ok(vec![TaskEvent::TaskReopened {
                    reopened_at: context.trusted_now,
                    reason,
                    authorized_by,
                }]),
                _ => Err(self.invalid("ReopenTask")),
            },

            TaskCommand::CancelTask { reason } => match self.status {
                TaskStatus::Draft
                | TaskStatus::Ready
                | TaskStatus::Active
                | TaskStatus::Paused
                | TaskStatus::Blocked
                | TaskStatus::Reopened => Ok(vec![TaskEvent::TaskCancelled {
                    cancelled_at: context.trusted_now,
                    reason,
                }]),
                _ => Err(self.invalid("CancelTask")),
            },
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            TaskEvent::TaskCreated { .. } => {
                // Handled by `from_created_event`; no-op on replay.
            }
            TaskEvent::TaskMarkedReady { .. } => {
                self.status = TaskStatus::Ready;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            TaskEvent::TaskOwnerAssigned { owner_id, .. } => {
                self.owner_id = *owner_id;
                self.authority_epoch = self.authority_epoch.advance();
            }
            TaskEvent::TaskPriorityChanged { priority, .. } => {
                self.priority = *priority;
            }
            TaskEvent::TaskDependencyAdded { dependency, .. } => {
                self.dependencies.push(*dependency);
            }
            TaskEvent::TaskStarted { .. } => {
                self.status = TaskStatus::Active;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            TaskEvent::TaskPaused { .. } => {
                self.status = TaskStatus::Paused;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            TaskEvent::TaskBlocked { .. } => {
                self.status = TaskStatus::Blocked;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            TaskEvent::TaskResumed { .. } => {
                self.status = TaskStatus::Active;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            TaskEvent::TaskUnblocked { .. } => {
                self.status = TaskStatus::Active;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            TaskEvent::TaskCompletionSubmitted { .. } => {
                self.status = TaskStatus::Submitted;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            TaskEvent::TaskRejected { .. } => {
                self.status = TaskStatus::Active;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            TaskEvent::TaskApproved { .. } => {
                self.status = TaskStatus::Approved;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            TaskEvent::TaskReopened { .. } => {
                self.status = TaskStatus::Reopened;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            TaskEvent::TaskClosed { .. } => {
                self.status = TaskStatus::Closed;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            TaskEvent::TaskCancelled { .. } => {
                self.status = TaskStatus::Cancelled;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
        }
        self.version = self.version.next();
    }
}
