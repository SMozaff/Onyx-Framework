//! Events produced by the Task aggregate.

use crate::value::{Dependency, TaskId, TaskMissionRef, TaskPriority};
use platform_kernel::{ObjectId, Timestamp, UserId};
use serde::{Deserialize, Serialize};

/// An event recording a fact that occurred to a Task aggregate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TaskEvent {
    /// A new task was created.
    TaskCreated {
        /// The new task's identifier.
        task_id: TaskId,
        /// The mission this task belongs to.
        mission_id: TaskMissionRef,
        /// The task's title.
        title: String,
        /// The task's initial owner.
        owner_id: UserId,
        /// When the task was created.
        created_at: Timestamp,
    },
    /// The task was marked ready.
    TaskMarkedReady {
        /// When the task was marked ready.
        marked_ready_at: Timestamp,
        /// The reason the task is ready.
        reason: String,
    },
    /// The task's owner was reassigned.
    TaskOwnerAssigned {
        /// The new owner.
        owner_id: UserId,
        /// When the reassignment occurred.
        assigned_at: Timestamp,
        /// The reason for the reassignment.
        reason: String,
    },
    /// The task's priority was changed.
    TaskPriorityChanged {
        /// The new priority.
        priority: TaskPriority,
        /// When the change occurred.
        changed_at: Timestamp,
        /// The reason for the change.
        reason: String,
    },
    /// A dependency was added to the task.
    TaskDependencyAdded {
        /// The dependency that was added.
        dependency: Dependency,
        /// When the dependency was added.
        added_at: Timestamp,
    },
    /// The task was started.
    TaskStarted {
        /// When the task was started.
        started_at: Timestamp,
        /// The reason for starting.
        reason: String,
    },
    /// The task was paused.
    TaskPaused {
        /// When the task was paused.
        paused_at: Timestamp,
        /// The reason for pausing.
        reason: String,
    },
    /// The task was marked blocked.
    TaskBlocked {
        /// When the task was blocked.
        blocked_at: Timestamp,
        /// The reason the task is blocked.
        reason: String,
        /// A reference to the blocking object, if any.
        blocking_ref: Option<ObjectId>,
    },
    /// The task was resumed from a paused state.
    ///
    /// # Increment 1 Ruling (B)
    TaskResumed {
        /// When the task was resumed.
        resumed_at: Timestamp,
        /// The reason for resuming.
        reason: String,
    },
    /// The task was unblocked.
    ///
    /// # Increment 1 Ruling (B)
    TaskUnblocked {
        /// When the task was unblocked.
        unblocked_at: Timestamp,
        /// The reason the task is unblocked.
        reason: String,
    },
    /// Completion was submitted for review.
    TaskCompletionSubmitted {
        /// When the completion was submitted.
        submitted_at: Timestamp,
        /// Evidence supporting completion.
        evidence: Vec<ObjectId>,
    },
    /// A submitted completion was rejected.
    ///
    /// # Increment 1 Ruling (B)
    TaskRejected {
        /// When the rejection occurred.
        rejected_at: Timestamp,
        /// The reason for rejection.
        reason: String,
    },
    /// A submitted completion was approved.
    TaskApproved {
        /// When the approval occurred.
        approved_at: Timestamp,
        /// The reason for approval.
        reason: String,
    },
    /// The task was reopened.
    ///
    /// # Increment 1 Ruling (T1)
    /// Reinstated per the Handover Document.
    TaskReopened {
        /// When the task was reopened.
        reopened_at: Timestamp,
        /// The reason for reopening.
        reason: String,
        /// The user who authorized the reopening.
        authorized_by: UserId,
    },
    /// The task was closed.
    TaskClosed {
        /// When the task was closed.
        closed_at: Timestamp,
        /// The reason for closure.
        reason: String,
    },
    /// The task was cancelled.
    TaskCancelled {
        /// When the task was cancelled.
        cancelled_at: Timestamp,
        /// The reason for cancellation.
        reason: String,
    },
}
