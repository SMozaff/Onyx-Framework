//! Coverage of INVALID Task transitions: every state paired with at least
//! one command that must be rejected.

use platform_contracts::AggregateRoot;
use platform_kernel::ObjectId;
use work_domain::test_support::{test_context, test_mission_id, test_user_id};
use work_domain::{Task, TaskCommand, TaskError, TaskStatus};

fn advance(task: &mut Task, command: TaskCommand) {
    let ctx = test_context();
    let events = task.decide(command, &ctx).expect("setup step must succeed");
    for event in &events {
        task.apply(event);
    }
}

fn assert_rejected(task: &Task, command: TaskCommand) {
    let ctx = test_context();
    let result = task.decide(command, &ctx);
    assert!(
        matches!(result, Err(TaskError::InvalidTransition(_))),
        "expected InvalidTransition from status {:?}, got {:?}",
        task.status(),
        result
    );
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
    task
}

fn task_in_cancelled() -> Task {
    let mut task = new_draft_task();
    advance(
        &mut task,
        TaskCommand::CancelTask {
            reason: "r".to_string(),
        },
    );
    task
}

fn task_in_reopened() -> Task {
    let mut task = task_in_closed();
    advance(
        &mut task,
        TaskCommand::ReopenTask {
            reason: "r".to_string(),
            authorized_by: test_user_id(),
        },
    );
    task
}

// Draft
#[test]
fn draft_rejects_start_task() {
    let task = new_draft_task();
    assert_rejected(
        &task,
        TaskCommand::StartTask {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn draft_rejects_mark_ready_twice() {
    let task = task_in_ready();
    assert_rejected(
        &task,
        TaskCommand::MarkReady {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn draft_rejects_close() {
    let task = new_draft_task();
    assert_rejected(
        &task,
        TaskCommand::CloseTask {
            reason: "x".to_string(),
        },
    );
}

// Ready
#[test]
fn ready_rejects_pause() {
    let task = task_in_ready();
    assert_rejected(
        &task,
        TaskCommand::PauseTask {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn ready_rejects_submit_completion() {
    let task = task_in_ready();
    assert_rejected(
        &task,
        TaskCommand::SubmitCompletion {
            evidence: vec![ObjectId::new_random()],
        },
    );
}

// Active
#[test]
fn active_rejects_start_task_again() {
    let task = task_in_active();
    assert_rejected(
        &task,
        TaskCommand::StartTask {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn active_rejects_resume() {
    let task = task_in_active();
    assert_rejected(
        &task,
        TaskCommand::ResumeTask {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn active_rejects_unblock() {
    let task = task_in_active();
    assert_rejected(
        &task,
        TaskCommand::UnblockTask {
            reason: "x".to_string(),
        },
    );
}

// Paused
#[test]
fn paused_rejects_block() {
    let task = task_in_paused();
    assert_rejected(
        &task,
        TaskCommand::BlockTask {
            reason: "x".to_string(),
            blocking_ref: None,
        },
    );
}

#[test]
fn paused_rejects_submit_completion() {
    let task = task_in_paused();
    assert_rejected(
        &task,
        TaskCommand::SubmitCompletion {
            evidence: vec![ObjectId::new_random()],
        },
    );
}

#[test]
fn paused_rejects_change_priority() {
    // Ruling B: self-loop commands only in Draft/Ready/Active/Reopened.
    let task = task_in_paused();
    assert_rejected(
        &task,
        TaskCommand::ChangePriority {
            priority: work_domain::TaskPriority::High,
            reason: "x".to_string(),
        },
    );
}

// Blocked
#[test]
fn blocked_rejects_pause() {
    let task = task_in_blocked();
    assert_rejected(
        &task,
        TaskCommand::PauseTask {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn blocked_rejects_assign_owner() {
    let task = task_in_blocked();
    assert_rejected(
        &task,
        TaskCommand::AssignOwner {
            owner_id: test_user_id(),
            reason: "x".to_string(),
        },
    );
}

// Submitted
#[test]
fn submitted_rejects_pause() {
    let task = task_in_submitted();
    assert_rejected(
        &task,
        TaskCommand::PauseTask {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn submitted_rejects_add_dependency() {
    let task = task_in_submitted();
    assert_rejected(
        &task,
        TaskCommand::AddDependency {
            dependency: work_domain::Dependency {
                task_id: work_domain::TaskId::new_random(),
                dependency_type: work_domain::DependencyType::FinishToStart,
            },
        },
    );
}

// Approved
#[test]
fn approved_rejects_reject_task() {
    let task = task_in_approved();
    assert_rejected(
        &task,
        TaskCommand::RejectTask {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn approved_rejects_approve_again() {
    let task = task_in_approved();
    assert_rejected(
        &task,
        TaskCommand::ApproveTask {
            reason: "x".to_string(),
        },
    );
}

// Closed
#[test]
fn closed_rejects_cancel() {
    let task = task_in_closed();
    assert_rejected(
        &task,
        TaskCommand::CancelTask {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn closed_rejects_close_again() {
    let task = task_in_closed();
    assert_rejected(
        &task,
        TaskCommand::CloseTask {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn closed_rejects_start_task() {
    let task = task_in_closed();
    assert_rejected(
        &task,
        TaskCommand::StartTask {
            reason: "x".to_string(),
        },
    );
}

// Cancelled: terminal
#[test]
fn cancelled_rejects_start_task() {
    let task = task_in_cancelled();
    assert_rejected(
        &task,
        TaskCommand::StartTask {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn cancelled_rejects_cancel_again() {
    let task = task_in_cancelled();
    assert_rejected(
        &task,
        TaskCommand::CancelTask {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn cancelled_rejects_reopen() {
    let task = task_in_cancelled();
    assert_rejected(
        &task,
        TaskCommand::ReopenTask {
            reason: "x".to_string(),
            authorized_by: test_user_id(),
        },
    );
}

// Reopened
#[test]
fn reopened_rejects_reopen_again() {
    let task = task_in_reopened();
    assert_rejected(
        &task,
        TaskCommand::ReopenTask {
            reason: "x".to_string(),
            authorized_by: test_user_id(),
        },
    );
}

#[test]
fn reopened_rejects_pause() {
    // Reopened has no PauseTask transition in the ruled table (only
    // StartTask -> Active and CancelTask -> Cancelled).
    let task = task_in_reopened();
    assert_rejected(
        &task,
        TaskCommand::PauseTask {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn reopened_rejects_close() {
    let task = task_in_reopened();
    assert_rejected(
        &task,
        TaskCommand::CloseTask {
            reason: "x".to_string(),
        },
    );
}

// CreateTask is never valid inside decide().
#[test]
fn decide_rejects_create_task_on_any_status() {
    let task = new_draft_task();
    assert_rejected(
        &task,
        TaskCommand::CreateTask {
            mission_id: test_mission_id(),
            title: "another".to_string(),
            description: None,
            owner_id: test_user_id(),
        },
    );
}

#[test]
fn task_status_display_is_debug_shaped() {
    assert_eq!(format!("{}", TaskStatus::Draft), "Draft");
}
