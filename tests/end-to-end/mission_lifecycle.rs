use mission_domain::test_support::{test_context, test_user_id};
use mission_domain::{Mission, MissionCommand, MissionEvent, MissionStatus};
use platform_contracts::AggregateRoot;
use platform_kernel::ObjectId;

use super::test_harness::PostgresHarness;

fn apply(mission: &mut Mission, command: MissionCommand) -> Vec<MissionEvent> {
    let events = mission
        .decide(command, &test_context())
        .expect("mission command");
    for event in &events {
        mission.apply(event);
    }
    events
}

#[tokio::test(flavor = "current_thread")]
async fn journey_1_mission_lifecycle() -> anyhow::Result<()> {
    let postgres = PostgresHarness::start().await?;

    let created = Mission::create(
        MissionCommand::CreateMission {
            name: "Team 8 release mission".to_string(),
            description: Some("Production release validation".to_string()),
            owner_id: test_user_id(),
        },
        &test_context(),
    )?;
    let mut mission = Mission::from_created_event(&created[0]);

    let revision = apply(
        &mut mission,
        MissionCommand::CreateBlueprintRevision {
            blueprint_data: "release blueprint".to_string(),
            version: "1.0".to_string(),
        },
    );
    let revision_id = match revision.first() {
        Some(MissionEvent::MissionBlueprintRevisionCreated { revision_id, .. }) => *revision_id,
        other => panic!("unexpected revision event: {other:?}"),
    };
    apply(
        &mut mission,
        MissionCommand::SubmitBlueprint { revision_id },
    );
    apply(
        &mut mission,
        MissionCommand::RequestApproval {
            reason: "release readiness".to_string(),
            evidence: vec![ObjectId::new_random()],
        },
    );
    apply(
        &mut mission,
        MissionCommand::ActivateMission {
            reason: "approved".to_string(),
        },
    );
    apply(
        &mut mission,
        MissionCommand::TriggerReview {
            reason: "go-live review".to_string(),
        },
    );
    apply(
        &mut mission,
        MissionCommand::CloseMission {
            reason: "release complete".to_string(),
            closure_evidence: vec![ObjectId::new_random()],
        },
    );
    apply(
        &mut mission,
        MissionCommand::ArchiveMission {
            reason: "retained for audit".to_string(),
        },
    );

    assert_eq!(mission.status(), MissionStatus::Archived);
    postgres
        .record("journey-1-mission-lifecycle", "passed")
        .await?;
    Ok(())
}
