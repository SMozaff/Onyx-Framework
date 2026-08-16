//! Exhaustive coverage of every VALID Task transition per the ruled
//! transition table (see workspace-root `DECISIONS.md`, rulings B and T1).

use platform_contracts::AggregateRoot;
use platform_kernel::ObjectId;
use work_domain::test_support::{test_context, test_mission_id, test_user_id};
use work_domain::{Task, TaskCommand, TaskEvent, TaskStatus};

fn advance(task: &mut Task, command: TaskCommand) -> Vec<TaskEvent> {
    let ctx = test_context();
    let events = task
        .decide(command, &ctx)
        .expect("expected transition to succeed");
    for event in &events {
        task.apply(event);
    }
    events
}

fn new_draft_task() -> Task {
    let ctx = test_context();
    let events = Task::create(
        TaskCommand::CreateTask {
            mission_id: test_mission_id(),
            title: "Test Task".to_string(),
            description: None,
            owner_id: test_user_id(),
        },
        &ctx,
    )
    .unwrap();
    Task::from_created_event(&events[0])
}

fn task_in_ready() -> Task {
    let mut task = new_draft_task();
    advance(
        &mut task,
        TaskCommand::MarkReady {
            reason: "r".to_string(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Ready);
    task
}

fn task_in_active() -> Task {
    let mut task = task_in_ready();
    advance(
        &mut task,
        TaskCommand::StartTask {
            reason: "r".to_string(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Active);
    task
}

fn task_in_paused() -> Task {
    let mut task = task_in_active();
    advance(
        &mut task,
        TaskCommand::PauseTask {
            reason: "r".to_string(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Paused);
    task
}

fn task_in_blocked() -> Task {
    let mut task = task_in_active();
    advance(
        &mut task,
        TaskCommand::BlockTask {
            reason: "r".to_string(),
            blocking_ref: None,
        },
    );
    assert_eq!(task.status(), TaskStatus::Blocked);
    task
}

fn task_in_submitted() -> Task {
    let mut task = task_in_active();
    advance(
        &mut task,
        TaskCommand::SubmitCompletion {
            evidence: vec![ObjectId::new_random()],
        },
    );
    assert_eq!(task.status(), TaskStatus::Submitted);
    task
}

fn task_in_approved() -> Task {
    let mut task = task_in_submitted();
    advance(
        &mut task,
        TaskCommand::ApproveTask {
            reason: "r".to_string(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Approved);
    task
}

fn task_in_closed() -> Task {
    let mut task = task_in_approved();
    advance(
        &mut task,
        TaskCommand::CloseTask {
            reason: "r".to_string(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Closed);
    task
}

fn task_in_reopened() -> Task {
    let mut task = task_in_closed();
    advance(
        &mut task,
        TaskCommand::ReopenTask {
            reason: "needs more work".to_string(),
            authorized_by: test_user_id(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Reopened);
    task
}

// Draft -> Ready (MarkReady) [ruling B]
#[test]
fn draft_to_ready_via_mark_ready() {
    let task = task_in_ready();
    assert_eq!(task.status(), TaskStatus::Ready);
}

// Draft -> Cancelled (CancelTask)
#[test]
fn draft_to_cancelled_via_cancel_task() {
    let mut task = new_draft_task();
    advance(
        &mut task,
        TaskCommand::CancelTask {
            reason: "r".to_string(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Cancelled);
}

// Ready -> Active (StartTask)
#[test]
fn ready_to_active_via_start_task() {
    let task = task_in_active();
    assert_eq!(task.status(), TaskStatus::Active);
}

// Ready -> Cancelled (CancelTask)
#[test]
fn ready_to_cancelled_via_cancel_task() {
    let mut task = task_in_ready();
    advance(
        &mut task,
        TaskCommand::CancelTask {
            reason: "r".to_string(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Cancelled);
}

// Active -> Paused (PauseTask)
#[test]
fn active_to_paused_via_pause_task() {
    let task = task_in_paused();
    assert_eq!(task.status(), TaskStatus::Paused);
}

// Active -> Blocked (BlockTask)
#[test]
fn active_to_blocked_via_block_task() {
    let task = task_in_blocked();
    assert_eq!(task.status(), TaskStatus::Blocked);
}

// Active -> Submitted (SubmitCompletion)
#[test]
fn active_to_submitted_via_submit_completion() {
    let task = task_in_submitted();
    assert_eq!(task.status(), TaskStatus::Submitted);
}

// Active -> Cancelled (CancelTask)
#[test]
fn active_to_cancelled_via_cancel_task() {
    let mut task = task_in_active();
    advance(
        &mut task,
        TaskCommand::CancelTask {
            reason: "r".to_string(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Cancelled);
}

// Paused -> Active (ResumeTask) [ruling B]
#[test]
fn paused_to_active_via_resume_task() {
    let mut task = task_in_paused();
    advance(
        &mut task,
        TaskCommand::ResumeTask {
            reason: "r".to_string(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Active);
}

// Paused -> Cancelled (CancelTask)
#[test]
fn paused_to_cancelled_via_cancel_task() {
    let mut task = task_in_paused();
    advance(
        &mut task,
        TaskCommand::CancelTask {
            reason: "r".to_string(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Cancelled);
}

// Blocked -> Active (UnblockTask) [ruling B]
#[test]
fn blocked_to_active_via_unblock_task() {
    let mut task = task_in_blocked();
    advance(
        &mut task,
        TaskCommand::UnblockTask {
            reason: "r".to_string(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Active);
}

// Blocked -> Cancelled (CancelTask)
#[test]
fn blocked_to_cancelled_via_cancel_task() {
    let mut task = task_in_blocked();
    advance(
        &mut task,
        TaskCommand::CancelTask {
            reason: "r".to_string(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Cancelled);
}

// Submitted -> Active (RejectTask) [ruling B]
#[test]
fn submitted_to_active_via_reject_task() {
    let mut task = task_in_submitted();
    advance(
        &mut task,
        TaskCommand::RejectTask {
            reason: "needs rework".to_string(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Active);
}

// Submitted -> Approved (ApproveTask)
#[test]
fn submitted_to_approved_via_approve_task() {
    let task = task_in_approved();
    assert_eq!(task.status(), TaskStatus::Approved);
}

// Approved -> Closed (CloseTask)
#[test]
fn approved_to_closed_via_close_task() {
    let task = task_in_closed();
    assert_eq!(task.status(), TaskStatus::Closed);
}

// Closed -> Reopened (ReopenTask) [ruling T1]
#[test]
fn closed_to_reopened_via_reopen_task() {
    let task = task_in_reopened();
    assert_eq!(task.status(), TaskStatus::Reopened);
}

// Reopened -> Active (StartTask) [ruling T1]
#[test]
fn reopened_to_active_via_start_task() {
    let mut task = task_in_reopened();
    advance(
        &mut task,
        TaskCommand::StartTask {
            reason: "resuming".to_string(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Active);
}

// Reopened -> Cancelled (CancelTask) [ruling T1]
#[test]
fn reopened_to_cancelled_via_cancel_task() {
    let mut task = task_in_reopened();
    advance(
        &mut task,
        TaskCommand::CancelTask {
            reason: "abandoning".to_string(),
        },
    );
    assert_eq!(task.status(), TaskStatus::Cancelled);
}

// Self-loop commands: AssignOwner, ChangePriority, AddDependency
// permitted in Draft, Ready, Active, Reopened (ruling B, extended).
#[test]
fn assign_owner_permitted_in_draft() {
    let mut task = new_draft_task();
    let new_owner = test_user_id();
    advance(
        &mut task,
        TaskCommand::AssignOwner {
            owner_id: new_owner,
            reason: "reassignment".to_string(),
        },
    );
    assert_eq!(task.owner_id(), new_owner);
    assert_eq!(task.status(), TaskStatus::Draft);
}

#[test]
fn change_priority_permitted_in_ready() {
    let mut task = task_in_ready();
    advance(
        &mut task,
        TaskCommand::ChangePriority {
            priority: work_domain::TaskPriority::Urgent,
            reason: "escalated".to_string(),
        },
    );
    assert_eq!(task.priority(), work_domain::TaskPriority::Urgent);
}

#[test]
fn add_dependency_permitted_in_active() {
    let mut task = task_in_active();
    let dep = work_domain::Dependency {
        task_id: work_domain::TaskId::new_random(),
        dependency_type: work_domain::DependencyType::FinishToStart,
    };
    advance(&mut task, TaskCommand::AddDependency { dependency: dep });
    assert_eq!(task.dependencies().len(), 1);
    assert_eq!(task.dependencies()[0], dep);
}

#[test]
fn assign_owner_permitted_in_reopened() {
    let mut task = task_in_reopened();
    let new_owner = test_user_id();
    advance(
        &mut task,
        TaskCommand::AssignOwner {
            owner_id: new_owner,
            reason: "reassignment while reopened".to_string(),
        },
    );
    assert_eq!(task.owner_id(), new_owner);
}
