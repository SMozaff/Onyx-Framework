//! Commands accepted by the Task aggregate.
//!
//! # Increment 1 Rulings
//! - Ruling B added `MarkReady`, `ResumeTask`, `UnblockTask`, `RejectTask`.
//! - Ruling T1 reinstated `ReopenTask` per the Handover Document.
//! - `CreateTask` is dispatched via `Task::create()`, not `decide()`
//!   (mirrors ruling C for Mission).

use crate::value::{Dependency, TaskMissionRef, TaskPriority};
use platform_kernel::{ObjectId, UserId};
use serde::{Deserialize, Serialize};

/// A command targeting the Task aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TaskCommand {
    /// Creates a new task. Dispatched via `Task::create()`, not `decide()`.
    CreateTask {
        /// The mission this task belongs to.
        mission_id: TaskMissionRef,
        /// The task's title.
        title: String,
        /// An optional longer description.
        description: Option<String>,
        /// The task's initial owner.
        owner_id: UserId,
    },
    /// Marks the task ready for assignment/execution (`Draft → Ready`).
    ///
    /// # Increment 1 Ruling (B)
    MarkReady {
        /// The reason the task is ready.
        reason: String,
    },
    /// Assigns a new owner to the task.
    AssignOwner {
        /// The new owner.
        owner_id: UserId,
        /// The reason for the reassignment.
        reason: String,
    },
    /// Changes the task's priority.
    ChangePriority {
        /// The new priority.
        priority: TaskPriority,
        /// The reason for the change.
        reason: String,
    },
    /// Adds a dependency on another task.
    AddDependency {
        /// The dependency to add.
        dependency: Dependency,
    },
    /// Starts the task (`Ready`/`Reopened → Active`).
    StartTask {
        /// The reason for starting.
        reason: String,
    },
    /// Pauses the task (`Active → Paused`).
    PauseTask {
        /// The reason for pausing.
        reason: String,
    },
    /// Marks the task blocked (`Active → Blocked`).
    BlockTask {
        /// The reason the task is blocked.
        reason: String,
        /// A reference to the blocking object, if any.
        blocking_ref: Option<ObjectId>,
    },
    /// Resumes a paused task (`Paused → Active`).
    ///
    /// # Increment 1 Ruling (B)
    ResumeTask {
        /// The reason for resuming.
        reason: String,
    },
    /// Unblocks a blocked task (`Blocked → Active`).
    ///
    /// # Increment 1 Ruling (B)
    UnblockTask {
        /// The reason the task is unblocked.
        reason: String,
    },
    /// Submits the task's completion for review (`Active → Submitted`).
    SubmitCompletion {
        /// Evidence supporting completion.
        evidence: Vec<ObjectId>,
    },
    /// Rejects a submitted completion, returning the task to `Active`.
    ///
    /// # Increment 1 Ruling (B)
    RejectTask {
        /// The reason for rejection.
        reason: String,
    },
    /// Approves a submitted completion (`Submitted → Approved`).
    ApproveTask {
        /// The reason for approval.
        reason: String,
    },
    /// Closes an approved task (`Approved → Closed`).
    CloseTask {
        /// The reason for closure.
        reason: String,
    },
    /// Reopens a closed task (`Closed → Reopened`).
    ///
    /// # Increment 1 Ruling (T1)
    /// Reinstated per the Handover Document. Requires exceptional
    /// authority (`authorized_by`).
    ReopenTask {
        /// The reason for reopening.
        reason: String,
        /// The user who authorized the reopening.
        authorized_by: UserId,
    },
    /// Cancels the task.
    CancelTask {
        /// The reason for cancellation.
        reason: String,
    },
}
