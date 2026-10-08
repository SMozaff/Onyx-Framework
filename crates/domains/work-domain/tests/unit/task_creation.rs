//! Tests for `Task::create()`.

use platform_contracts::AggregateRoot;
use work_domain::test_support::{test_context, test_mission_id, test_user_id};
use work_domain::{Task, TaskCommand, TaskError, TaskEvent, TaskStatus};

#[test]
fn create_task_with_valid_title_succeeds() {
    let ctx = test_context();
    let mission_id = test_mission_id();
    let owner = test_user_id();
    let events = Task::create(
        TaskCommand::CreateTask {
            mission_id,
            title: "Recon the perimeter".to_string(),
            description: Some("A test task".to_string()),
            owner_id: owner,
        },
        &ctx,
    )
    .expect("creation should succeed");

    assert_eq!(events.len(), 1);
    match &events[0] {
        TaskEvent::TaskCreated {
            title,
            owner_id,
            mission_id: mid,
            ..
        } => {
            assert_eq!(title, "Recon the perimeter");
            assert_eq!(*owner_id, owner);
            assert_eq!(*mid, mission_id);
        }
        other => panic!("expected TaskCreated, got {:?}", other),
    }
}

#[test]
fn create_task_with_empty_title_fails() {
    let ctx = test_context();
    let result = Task::create(
        TaskCommand::CreateTask {
            mission_id: test_mission_id(),
            title: "   ".to_string(),
            description: None,
            owner_id: test_user_id(),
        },
        &ctx,
    );
    assert_eq!(result, Err(TaskError::MissingField("title".to_string())));
}

#[test]
fn create_task_rejects_non_create_command() {
    let ctx = test_context();
    let result = Task::create(
        TaskCommand::CancelTask {
            reason: "oops".to_string(),
        },
        &ctx,
    );
    assert!(matches!(result, Err(TaskError::InvalidTransition(_))));
}

#[test]
fn rehydrated_task_starts_in_draft() {
    let ctx = test_context();
    let mission_id = test_mission_id();
    let owner = test_user_id();
    let events = Task::create(
        TaskCommand::CreateTask {
            mission_id,
            title: "Recon the perimeter".to_string(),
            description: None,
            owner_id: owner,
        },
        &ctx,
    )
    .unwrap();

    let task = Task::from_created_event(&events[0]);
    assert_eq!(task.status(), TaskStatus::Draft);
    assert_eq!(task.title(), "Recon the perimeter");
    assert_eq!(task.owner_id(), owner);
    assert_eq!(task.mission_id(), mission_id);
    assert_eq!(task.version().0, 0);
}

#[test]
#[should_panic(expected = "from_created_event called with a non-TaskCreated event")]
fn from_created_event_panics_on_wrong_event_type() {
    let event = TaskEvent::TaskCancelled {
        cancelled_at: platform_kernel::Timestamp::from_nanos(0),
        reason: "x".to_string(),
    };
    let _ = Task::from_created_event(&event);
}

#[test]
fn applying_task_created_again_is_a_noop() {
    let ctx = test_context();
    let events = Task::create(
        TaskCommand::CreateTask {
            mission_id: test_mission_id(),
            title: "Recon the perimeter".to_string(),
            description: None,
            owner_id: test_user_id(),
        },
        &ctx,
    )
    .unwrap();

    let mut task = Task::from_created_event(&events[0]);
    let before_status = task.status();
    task.apply(&events[0]);
    assert_eq!(task.status(), before_status);
    assert_eq!(task.version().0, 1);
}
