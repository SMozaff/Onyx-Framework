use platform_contracts::AggregateRoot;
use platform_kernel::ObjectId;
use work_domain::test_support::{test_context, test_mission_id, test_user_id};
use work_domain::{Task, TaskCommand, TaskEvent, TaskStatus};

use super::test_harness::PostgresHarness;

fn apply(task: &mut Task, command: TaskCommand) -> Vec<TaskEvent> {
    let events = task.decide(command, &test_context()).expect("task command");
    for event in &events {
        task.apply(event);
    }
    events
}

#[tokio::test(flavor = "current_thread")]
async fn journey_2_task_workflow() -> anyhow::Result<()> {
    let postgres = PostgresHarness::start().await?;

    let created = Task::create(
        TaskCommand::CreateTask {
            mission_id: test_mission_id(),
            title: "Validate production release".to_string(),
            description: Some("Run mandatory backend gates".to_string()),
            owner_id: test_user_id(),
        },
        &test_context(),
    )?;
    let mut task = Task::from_created_event(&created[0]);
    apply(
        &mut task,
        TaskCommand::MarkReady {
            reason: "defined".to_string(),
        },
    );
    apply(
        &mut task,
        TaskCommand::StartTask {
            reason: "execution started".to_string(),
        },
    );
    apply(
        &mut task,
        TaskCommand::SubmitCompletion {
            evidence: vec![ObjectId::new_random()],
        },
    );
    apply(
        &mut task,
        TaskCommand::ApproveTask {
            reason: "evidence verified".to_string(),
        },
    );
    apply(
        &mut task,
        TaskCommand::CloseTask {
            reason: "done".to_string(),
        },
    );

    assert_eq!(task.status(), TaskStatus::Closed);
    postgres.record("journey-2-task-workflow", "passed").await?;
    Ok(())
}
