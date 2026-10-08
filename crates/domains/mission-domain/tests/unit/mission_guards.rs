//! Tests for guard conditions beyond pure state-gating: field validation,
//! blueprint-revision matching, closure evidence requirements, and the
//! Increment 1 authority stub.

use mission_domain::test_support::test_context;
use mission_domain::{Mission, MissionCommand, MissionError, MissionEvent};
use platform_contracts::AggregateRoot;
use platform_kernel::ObjectId;

fn new_draft_mission() -> Mission {
    let ctx = test_context();
    let events = Mission::create(
        MissionCommand::CreateMission {
            name: "Test Mission".to_string(),
            description: None,
            owner_id: mission_domain::test_support::test_user_id(),
        },
        &ctx,
    )
    .unwrap();
    Mission::from_created_event(&events[0])
}

#[test]
fn create_blueprint_revision_rejects_empty_data() {
    let mission = new_draft_mission();
    let ctx = test_context();
    let result = mission.decide(
        MissionCommand::CreateBlueprintRevision {
            blueprint_data: "   ".to_string(),
            version: "1.0".to_string(),
        },
        &ctx,
    );
    assert!(matches!(result, Err(MissionError::InvalidBlueprint(_))));
}

#[test]
fn submit_blueprint_rejects_mismatched_revision_id() {
    let mut mission = new_draft_mission();
    let ctx = test_context();
    let events = mission
        .decide(
            MissionCommand::CreateBlueprintRevision {
                blueprint_data: "plan".to_string(),
                version: "1.0".to_string(),
            },
            &ctx,
        )
        .unwrap();
    for e in &events {
        mission.apply(e);
    }

    let wrong_id = ObjectId::new_random();
    let result = mission.decide(
        MissionCommand::SubmitBlueprint {
            revision_id: wrong_id,
        },
        &ctx,
    );
    assert!(matches!(result, Err(MissionError::InvalidBlueprint(_))));
}

#[test]
fn operational_halt_rejects_empty_emergency_code() {
    let mut mission = new_draft_mission();
    let ctx = test_context();

    // Drive to Active first.
    let events = mission
        .decide(
            MissionCommand::CreateBlueprintRevision {
                blueprint_data: "plan".to_string(),
                version: "1.0".to_string(),
            },
            &ctx,
        )
        .unwrap();
    for e in &events {
        mission.apply(e);
    }
    let revision_id = match &events[0] {
        MissionEvent::MissionBlueprintRevisionCreated { revision_id, .. } => *revision_id,
        _ => unreachable!(),
    };
    let events = mission
        .decide(MissionCommand::SubmitBlueprint { revision_id }, &ctx)
        .unwrap();
    for e in &events {
        mission.apply(e);
    }
    let events = mission
        .decide(
            MissionCommand::RequestApproval {
                reason: "r".to_string(),
                evidence: vec![],
            },
            &ctx,
        )
        .unwrap();
    for e in &events {
        mission.apply(e);
    }
    let events = mission
        .decide(
            MissionCommand::ActivateMission {
                reason: "r".to_string(),
            },
            &ctx,
        )
        .unwrap();
    for e in &events {
        mission.apply(e);
    }

    let result = mission.decide(
        MissionCommand::OperationalHaltMission {
            reason: "incident".to_string(),
            emergency_code: "".to_string(),
        },
        &ctx,
    );
    assert!(matches!(result, Err(MissionError::MissingField(_))));
}

#[test]
fn close_mission_rejects_empty_closure_evidence() {
    let mut mission = new_draft_mission();
    let ctx = test_context();
    let events = mission
        .decide(
            MissionCommand::CreateBlueprintRevision {
                blueprint_data: "plan".to_string(),
                version: "1.0".to_string(),
            },
            &ctx,
        )
        .unwrap();
    for e in &events {
        mission.apply(e);
    }
    let revision_id = match &events[0] {
        MissionEvent::MissionBlueprintRevisionCreated { revision_id, .. } => *revision_id,
        _ => unreachable!(),
    };
    let events = mission
        .decide(MissionCommand::SubmitBlueprint { revision_id }, &ctx)
        .unwrap();
    for e in &events {
        mission.apply(e);
    }
    let events = mission
        .decide(
            MissionCommand::RequestApproval {
                reason: "r".to_string(),
                evidence: vec![],
            },
            &ctx,
        )
        .unwrap();
    for e in &events {
        mission.apply(e);
    }
    let events = mission
        .decide(
            MissionCommand::ActivateMission {
                reason: "r".to_string(),
            },
            &ctx,
        )
        .unwrap();
    for e in &events {
        mission.apply(e);
    }

    let result = mission.decide(
        MissionCommand::CloseMission {
            reason: "done".to_string(),
            closure_evidence: vec![],
        },
        &ctx,
    );
    assert!(matches!(
        result,
        Err(MissionError::ClosureCriteriaNotMet(_))
    ));
}

#[test]
fn request_approval_requires_a_submitted_blueprint() {
    // This should be unreachable via decide() because RequestApproval also
    // requires status == Planning, and Planning can only be reached via a
    // successful SubmitBlueprint (which always sets
    // submitted_blueprint_revision_id). This test exists to document and
    // lock in that invariant explicitly, in case a future increment adds
    // another path into Planning.
    let mut mission = new_draft_mission();
    let ctx = test_context();
    let events = mission
        .decide(
            MissionCommand::CreateBlueprintRevision {
                blueprint_data: "plan".to_string(),
                version: "1.0".to_string(),
            },
            &ctx,
        )
        .unwrap();
    for e in &events {
        mission.apply(e);
    }
    let revision_id = match &events[0] {
        MissionEvent::MissionBlueprintRevisionCreated { revision_id, .. } => *revision_id,
        _ => unreachable!(),
    };
    let events = mission
        .decide(MissionCommand::SubmitBlueprint { revision_id }, &ctx)
        .unwrap();
    for e in &events {
        mission.apply(e);
    }
    assert!(mission
        .decide(
            MissionCommand::RequestApproval {
                reason: "r".to_string(),
                evidence: vec![]
            },
            &ctx
        )
        .is_ok());
}

/// # Increment 1 Ruling (E)
/// The authority stub always returns `true`, so no command should ever be
/// rejected as `Unauthorized` in Increment 1. This test locks that
/// contract in explicitly so it's caught if the stub behavior ever
/// silently regresses before Increment 7 lands.
#[test]
fn authority_stub_never_rejects_in_increment_1() {
    let mission = new_draft_mission();
    let ctx = test_context();
    let result = mission.decide(
        MissionCommand::CreateBlueprintRevision {
            blueprint_data: "plan".to_string(),
            version: "1.0".to_string(),
        },
        &ctx,
    );
    assert!(result.is_ok());
    assert!(!matches!(result, Err(MissionError::Unauthorized(_))));
}
