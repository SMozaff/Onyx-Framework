//! Tests for Task guard conditions beyond pure state-gating.

use platform_contracts::AggregateRoot;
use work_domain::test_support::{test_context, test_mission_id, test_user_id};
use work_domain::{Dependency, DependencyType, Task, TaskCommand, TaskError, TaskId};

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

fn advance(task: &mut Task, command: TaskCommand) {
    let ctx = test_context();
    let events = task.decide(command, &ctx).unwrap();
    for e in &events {
        task.apply(e);
    }
}

fn task_in_active() -> Task {
    let mut task = new_draft_task();
    advance(
        &mut task,
        TaskCommand::MarkReady {
            reason: "r".to_string(),
        },
    );
    advance(
        &mut task,
        TaskCommand::StartTask {
            reason: "r".to_string(),
        },
    );
    task
}

#[test]
fn submit_completion_rejects_empty_evidence() {
    let task = task_in_active();
    let ctx = test_context();
    let result = task.decide(TaskCommand::SubmitCompletion { evidence: vec![] }, &ctx);
    assert!(matches!(
        result,
        Err(TaskError::MissingCompletionEvidence(_))
    ));
}

#[test]
fn add_dependency_rejects_self_dependency() {
    let task = task_in_active();
    let ctx = test_context();
    let self_id = *task.id();
    let result = task.decide(
        TaskCommand::AddDependency {
            dependency: Dependency {
                task_id: self_id,
                dependency_type: DependencyType::FinishToStart,
            },
        },
        &ctx,
    );
    assert!(matches!(result, Err(TaskError::InvalidDependency(_))));
}

#[test]
fn add_dependency_rejects_exact_duplicate() {
    let mut task = task_in_active();
    let dep = Dependency {
        task_id: TaskId::new_random(),
        dependency_type: DependencyType::FinishToStart,
    };
    advance(&mut task, TaskCommand::AddDependency { dependency: dep });

    let ctx = test_context();
    let result = task.decide(TaskCommand::AddDependency { dependency: dep }, &ctx);
    assert!(matches!(result, Err(TaskError::InvalidDependency(_))));
}

#[test]
fn add_dependency_allows_distinct_dependencies_on_same_task_with_different_type() {
    // Not a duplicate: same task_id but a different dependency_type is a
    // structurally distinct Dependency value (ruling B only rejects exact
    // duplicates and self-dependency).
    let mut task = task_in_active();
    let target = TaskId::new_random();
    advance(
        &mut task,
        TaskCommand::AddDependency {
            dependency: Dependency {
                task_id: target,
                dependency_type: DependencyType::FinishToStart,
            },
        },
    );
    advance(
        &mut task,
        TaskCommand::AddDependency {
            dependency: Dependency {
                task_id: target,
                dependency_type: DependencyType::StartToStart,
            },
        },
    );
    assert_eq!(task.dependencies().len(), 2);
}

/// # Increment 1 Ruling (E)
/// The authority stub always returns `true`, so no command should ever be
/// rejected as `Unauthorized` in Increment 1.
#[test]
fn authority_stub_never_rejects_in_increment_1() {
    let task = new_draft_task();
    let ctx = test_context();
    let result = task.decide(
        TaskCommand::MarkReady {
            reason: "r".to_string(),
        },
        &ctx,
    );
    assert!(result.is_ok());
    assert!(!matches!(result, Err(TaskError::Unauthorized(_))));
}
