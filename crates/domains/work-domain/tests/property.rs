//! Property-based tests for the Task aggregate. Marked `#[ignore]` per §10.

use platform_contracts::AggregateRoot;
use platform_kernel::ObjectId;
use proptest::prelude::*;
use work_domain::test_support::{test_context, test_mission_id, test_user_id};
use work_domain::{Dependency, DependencyType, Task, TaskCommand, TaskError, TaskId, TaskStatus};

fn new_draft_task() -> Task {
    let ctx = test_context();
    let events = Task::create(
        TaskCommand::CreateTask {
            mission_id: test_mission_id(),
            title: "Property Task".to_string(),
            description: None,
            owner_id: test_user_id(),
        },
        &ctx,
    )
    .unwrap();
    Task::from_created_event(&events[0])
}

fn arbitrary_command() -> impl Strategy<Value = TaskCommand> {
    prop_oneof![
        Just(TaskCommand::MarkReady {
            reason: "r".to_string(),
        }),
        Just(TaskCommand::AssignOwner {
            owner_id: ObjectId::new_random(),
            reason: "r".to_string(),
        }),
        Just(TaskCommand::ChangePriority {
            priority: work_domain::TaskPriority::High,
            reason: "r".to_string(),
        }),
        Just(TaskCommand::AddDependency {
            dependency: Dependency {
                task_id: TaskId::new_random(),
                dependency_type: DependencyType::FinishToStart,
            },
        }),
        Just(TaskCommand::StartTask {
            reason: "r".to_string(),
        }),
        Just(TaskCommand::PauseTask {
            reason: "r".to_string(),
        }),
        Just(TaskCommand::BlockTask {
            reason: "r".to_string(),
            blocking_ref: None,
        }),
        Just(TaskCommand::ResumeTask {
            reason: "r".to_string(),
        }),
        Just(TaskCommand::UnblockTask {
            reason: "r".to_string(),
        }),
        Just(TaskCommand::SubmitCompletion {
            evidence: vec![ObjectId::new_random()],
        }),
        Just(TaskCommand::RejectTask {
            reason: "r".to_string(),
        }),
        Just(TaskCommand::ApproveTask {
            reason: "r".to_string(),
        }),
        Just(TaskCommand::CloseTask {
            reason: "r".to_string(),
        }),
        Just(TaskCommand::ReopenTask {
            reason: "r".to_string(),
            authorized_by: ObjectId::new_random(),
        }),
        Just(TaskCommand::CancelTask {
            reason: "r".to_string(),
        }),
    ]
}

proptest! {
    /// INVARIANT: `decide()` is total -- never panics for any command from
    /// any reachable status.
    #[test]
    #[ignore]
    fn decide_never_panics_for_any_command_sequence(
        commands in proptest::collection::vec(arbitrary_command(), 0..20)
    ) {
        let mut task = new_draft_task();
        let ctx = test_context();
        for command in commands {
            if let Ok(events) = task.decide(command, &ctx) {
                for event in &events {
                    task.apply(event);
                }
            }
        }
    }

    /// INVARIANT: version increments by exactly one per successfully
    /// applied event batch (every Task command in Increment 1 produces
    /// exactly one event on success).
    #[test]
    #[ignore]
    fn version_increments_monotonically_with_successful_commands(
        commands in proptest::collection::vec(arbitrary_command(), 0..20)
    ) {
        let mut task = new_draft_task();
        let ctx = test_context();
        let mut expected_version = task.version().0;
        for command in commands {
            if let Ok(events) = task.decide(command, &ctx) {
                for event in &events {
                    task.apply(event);
                    expected_version += 1;
                }
            }
        }
        prop_assert_eq!(task.version().0, expected_version);
    }

    /// INVARIANT: a rejected command never mutates the aggregate.
    #[test]
    #[ignore]
    fn rejected_command_leaves_aggregate_unchanged(
        commands in proptest::collection::vec(arbitrary_command(), 1..20)
    ) {
        let mut task = new_draft_task();
        let ctx = test_context();
        for command in commands {
            let status_before = task.status();
            let version_before = task.version();
            let owner_before = task.owner_id();

            match task.decide(command, &ctx) {
                Ok(events) => {
                    for event in &events {
                        task.apply(event);
                    }
                }
                Err(_) => {
                    prop_assert_eq!(task.status(), status_before);
                    prop_assert_eq!(task.version(), version_before);
                    prop_assert_eq!(task.owner_id(), owner_before);
                }
            }
        }
    }

    /// INVARIANT: `Cancelled` is terminal -- once reached, no command
    /// sequence can move the task out of `Cancelled`.
    #[test]
    #[ignore]
    fn cancelled_is_terminal_under_any_further_commands(
        commands in proptest::collection::vec(arbitrary_command(), 0..10)
    ) {
        let mut task = new_draft_task();
        let ctx = test_context();
        let events = task
            .decide(TaskCommand::CancelTask { reason: "r".to_string() }, &ctx)
            .unwrap();
        for e in &events { task.apply(e); }
        prop_assert_eq!(task.status(), TaskStatus::Cancelled);

        for command in commands {
            let result = task.decide(command, &ctx);
            prop_assert!(matches!(result, Err(TaskError::InvalidTransition(_))));
            prop_assert_eq!(task.status(), TaskStatus::Cancelled);
        }
    }
}
