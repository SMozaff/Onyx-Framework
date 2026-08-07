//! Exhaustive coverage of every VALID Mission transition per the ruled
//! transition table (see workspace-root `DECISIONS.md`).

use mission_domain::test_support::test_context;
use mission_domain::{Mission, MissionCommand, MissionEvent, MissionStatus};
use platform_contracts::AggregateRoot;
use platform_kernel::ObjectId;

/// Advances `mission` by deciding+applying `command`, returning the
/// produced events. Panics (failing the test) if `decide()` errors.
fn advance(mission: &mut Mission, command: MissionCommand) -> Vec<MissionEvent> {
    let ctx = test_context();
    let events = mission
        .decide(command, &ctx)
        .expect("expected transition to succeed");
    for event in &events {
        mission.apply(event);
    }
    events
}

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

/// Drives a fresh mission to `Planning` by creating and submitting a
/// blueprint revision.
fn mission_in_planning() -> (Mission, ObjectId) {
    let mut mission = new_draft_mission();
    let events = advance(
        &mut mission,
        MissionCommand::CreateBlueprintRevision {
            blueprint_data: "plan v1".to_string(),
            version: "1.0".to_string(),
        },
    );
    let revision_id = match &events[0] {
        MissionEvent::MissionBlueprintRevisionCreated { revision_id, .. } => *revision_id,
        _ => panic!("expected MissionBlueprintRevisionCreated"),
    };
    advance(
        &mut mission,
        MissionCommand::SubmitBlueprint { revision_id },
    );
    assert_eq!(mission.status(), MissionStatus::Planning);
    (mission, revision_id)
}

fn mission_in_awaiting_approval() -> Mission {
    let (mut mission, _revision_id) = mission_in_planning();
    advance(
        &mut mission,
        MissionCommand::RequestApproval {
            reason: "ready for review".to_string(),
            evidence: vec![],
        },
    );
    assert_eq!(mission.status(), MissionStatus::AwaitingApproval);
    mission
}

fn mission_in_active() -> Mission {
    let mut mission = mission_in_awaiting_approval();
    advance(
        &mut mission,
        MissionCommand::ActivateMission {
            reason: "approved".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Active);
    mission
}

fn mission_in_review() -> Mission {
    let mut mission = mission_in_active();
    advance(
        &mut mission,
        MissionCommand::TriggerReview {
            reason: "milestone review".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Review);
    mission
}

fn mission_in_paused() -> Mission {
    let mut mission = mission_in_active();
    advance(
        &mut mission,
        MissionCommand::PauseMission {
            reason: "waiting on resources".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Paused);
    mission
}

fn mission_in_halted() -> Mission {
    let mut mission = mission_in_active();
    advance(
        &mut mission,
        MissionCommand::OperationalHaltMission {
            reason: "safety incident".to_string(),
            emergency_code: "E-911".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Halted);
    mission
}

fn mission_in_closed() -> Mission {
    let mut mission = mission_in_active();
    advance(
        &mut mission,
        MissionCommand::CloseMission {
            reason: "objectives met".to_string(),
            closure_evidence: vec![ObjectId::new_random()],
        },
    );
    assert_eq!(mission.status(), MissionStatus::Closed);
    mission
}

fn mission_in_cancelled() -> Mission {
    let mut mission = new_draft_mission();
    advance(
        &mut mission,
        MissionCommand::CancelMission {
            reason: "no longer needed".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Cancelled);
    mission
}

// ---------------------------------------------------------------------
// Draft -> Planning (SubmitBlueprint)
// ---------------------------------------------------------------------
#[test]
fn draft_to_planning_via_submit_blueprint() {
    let (mission, _rev) = mission_in_planning();
    assert_eq!(mission.status(), MissionStatus::Planning);
}

// ---------------------------------------------------------------------
// Draft -> Cancelled (CancelMission)
// ---------------------------------------------------------------------
#[test]
fn draft_to_cancelled_via_cancel_mission() {
    let mission = mission_in_cancelled();
    assert_eq!(mission.status(), MissionStatus::Cancelled);
}

// ---------------------------------------------------------------------
// Planning -> AwaitingApproval (RequestApproval) [ruling M1]
// ---------------------------------------------------------------------
#[test]
fn planning_to_awaiting_approval_via_request_approval() {
    let mission = mission_in_awaiting_approval();
    assert_eq!(mission.status(), MissionStatus::AwaitingApproval);
}

// ---------------------------------------------------------------------
// Planning -> Active (ActivateMission)
// ---------------------------------------------------------------------
#[test]
fn planning_to_active_via_activate_mission() {
    let (mut mission, _rev) = mission_in_planning();
    advance(
        &mut mission,
        MissionCommand::ActivateMission {
            reason: "approval stubbed true (Inc 1 ruling A)".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Active);
}

// ---------------------------------------------------------------------
// Planning -> Paused (PauseMission)
// ---------------------------------------------------------------------
#[test]
fn planning_to_paused_via_pause_mission() {
    let (mut mission, _rev) = mission_in_planning();
    advance(
        &mut mission,
        MissionCommand::PauseMission {
            reason: "manager requested pause".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Paused);
}

// ---------------------------------------------------------------------
// Planning -> Cancelled (CancelMission)
// ---------------------------------------------------------------------
#[test]
fn planning_to_cancelled_via_cancel_mission() {
    let (mut mission, _rev) = mission_in_planning();
    advance(
        &mut mission,
        MissionCommand::CancelMission {
            reason: "scope changed".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Cancelled);
}

// ---------------------------------------------------------------------
// AwaitingApproval -> Planning (RejectApproval) [ruling M1]
// ---------------------------------------------------------------------
#[test]
fn awaiting_approval_to_planning_via_reject_approval() {
    let mut mission = mission_in_awaiting_approval();
    advance(
        &mut mission,
        MissionCommand::RejectApproval {
            reason: "needs more detail".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Planning);
}

// ---------------------------------------------------------------------
// AwaitingApproval -> Active (ActivateMission)
// ---------------------------------------------------------------------
#[test]
fn awaiting_approval_to_active_via_activate_mission() {
    let mission = mission_in_active();
    assert_eq!(mission.status(), MissionStatus::Active);
}

// ---------------------------------------------------------------------
// AwaitingApproval -> Cancelled (CancelMission)
// ---------------------------------------------------------------------
#[test]
fn awaiting_approval_to_cancelled_via_cancel_mission() {
    let mut mission = mission_in_awaiting_approval();
    advance(
        &mut mission,
        MissionCommand::CancelMission {
            reason: "budget cut".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Cancelled);
}

// ---------------------------------------------------------------------
// Active -> Paused (PauseMission)
// ---------------------------------------------------------------------
#[test]
fn active_to_paused_via_pause_mission() {
    let mission = mission_in_paused();
    assert_eq!(mission.status(), MissionStatus::Paused);
}

// ---------------------------------------------------------------------
// Active -> Halted (OperationalHaltMission)
// ---------------------------------------------------------------------
#[test]
fn active_to_halted_via_operational_halt() {
    let mission = mission_in_halted();
    assert_eq!(mission.status(), MissionStatus::Halted);
}

// ---------------------------------------------------------------------
// Active -> Review (TriggerReview) [ruling A]
// ---------------------------------------------------------------------
#[test]
fn active_to_review_via_trigger_review() {
    let mission = mission_in_review();
    assert_eq!(mission.status(), MissionStatus::Review);
}

// ---------------------------------------------------------------------
// Active -> Closed (CloseMission)
// ---------------------------------------------------------------------
#[test]
fn active_to_closed_via_close_mission() {
    let mission = mission_in_closed();
    assert_eq!(mission.status(), MissionStatus::Closed);
}

// ---------------------------------------------------------------------
// Active -> Cancelled (CancelMission)
// ---------------------------------------------------------------------
#[test]
fn active_to_cancelled_via_cancel_mission() {
    let mut mission = mission_in_active();
    advance(
        &mut mission,
        MissionCommand::CancelMission {
            reason: "mission aborted".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Cancelled);
}

// ---------------------------------------------------------------------
// Paused -> Active (ResumeMission)
// ---------------------------------------------------------------------
#[test]
fn paused_to_active_via_resume_mission() {
    let mut mission = mission_in_paused();
    advance(
        &mut mission,
        MissionCommand::ResumeMission {
            reason: "resources available".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Active);
}

// ---------------------------------------------------------------------
// Paused -> Halted (OperationalHaltMission)
// ---------------------------------------------------------------------
#[test]
fn paused_to_halted_via_operational_halt() {
    let mut mission = mission_in_paused();
    advance(
        &mut mission,
        MissionCommand::OperationalHaltMission {
            reason: "incident while paused".to_string(),
            emergency_code: "E-911".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Halted);
}

// ---------------------------------------------------------------------
// Paused -> Cancelled (CancelMission)
// ---------------------------------------------------------------------
#[test]
fn paused_to_cancelled_via_cancel_mission() {
    let mut mission = mission_in_paused();
    advance(
        &mut mission,
        MissionCommand::CancelMission {
            reason: "cancelled while paused".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Cancelled);
}

// ---------------------------------------------------------------------
// Halted -> Active (RestartMission, checklist_completed=true) [ruling A]
// ---------------------------------------------------------------------
#[test]
fn halted_to_active_via_restart_with_completed_checklist() {
    let mut mission = mission_in_halted();
    advance(
        &mut mission,
        MissionCommand::RestartMission {
            reason: "checklist complete".to_string(),
            checklist_completed: true,
        },
    );
    assert_eq!(mission.status(), MissionStatus::Active);
}

// ---------------------------------------------------------------------
// Halted -> Paused (RestartMission, checklist_completed=false) [ruling A]
// ---------------------------------------------------------------------
#[test]
fn halted_to_paused_via_restart_with_incomplete_checklist() {
    let mut mission = mission_in_halted();
    advance(
        &mut mission,
        MissionCommand::RestartMission {
            reason: "checklist incomplete".to_string(),
            checklist_completed: false,
        },
    );
    assert_eq!(mission.status(), MissionStatus::Paused);
}

// ---------------------------------------------------------------------
// Halted -> Cancelled (CancelMission)
// ---------------------------------------------------------------------
#[test]
fn halted_to_cancelled_via_cancel_mission() {
    let mut mission = mission_in_halted();
    advance(
        &mut mission,
        MissionCommand::CancelMission {
            reason: "cancelled while halted".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Cancelled);
}

// ---------------------------------------------------------------------
// Review -> Active (ActivateMission)
// ---------------------------------------------------------------------
#[test]
fn review_to_active_via_activate_mission() {
    let mut mission = mission_in_review();
    advance(
        &mut mission,
        MissionCommand::ActivateMission {
            reason: "review passed".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Active);
}

// ---------------------------------------------------------------------
// Review -> Closed (CloseMission)
// ---------------------------------------------------------------------
#[test]
fn review_to_closed_via_close_mission() {
    let mut mission = mission_in_review();
    advance(
        &mut mission,
        MissionCommand::CloseMission {
            reason: "review concluded successfully".to_string(),
            closure_evidence: vec![ObjectId::new_random()],
        },
    );
    assert_eq!(mission.status(), MissionStatus::Closed);
}

// ---------------------------------------------------------------------
// Review -> Cancelled (CancelMission)
// ---------------------------------------------------------------------
#[test]
fn review_to_cancelled_via_cancel_mission() {
    let mut mission = mission_in_review();
    advance(
        &mut mission,
        MissionCommand::CancelMission {
            reason: "cancelled during review".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Cancelled);
}

// ---------------------------------------------------------------------
// Closed -> Archived (ArchiveMission)
// ---------------------------------------------------------------------
#[test]
fn closed_to_archived_via_archive_mission() {
    let mut mission = mission_in_closed();
    advance(
        &mut mission,
        MissionCommand::ArchiveMission {
            reason: "retention period reached".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Archived);
}

// ---------------------------------------------------------------------
// Cancelled -> Archived (ArchiveMission) [ruling: Handover authoritative]
// ---------------------------------------------------------------------
#[test]
fn cancelled_to_archived_via_archive_mission() {
    let mut mission = mission_in_cancelled();
    advance(
        &mut mission,
        MissionCommand::ArchiveMission {
            reason: "cancelled mission retired".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Archived);
}

// ---------------------------------------------------------------------
// CreateBlueprintRevision does not change status (ruling A)
// ---------------------------------------------------------------------
#[test]
fn create_blueprint_revision_does_not_change_status() {
    let mut mission = new_draft_mission();
    advance(
        &mut mission,
        MissionCommand::CreateBlueprintRevision {
            blueprint_data: "draft plan".to_string(),
            version: "0.1".to_string(),
        },
    );
    assert_eq!(mission.status(), MissionStatus::Draft);
    assert!(mission.blueprint_revision_id().is_some());
}
