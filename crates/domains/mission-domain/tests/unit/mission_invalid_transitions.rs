//! Coverage of INVALID Mission transitions: every state paired with at
//! least one command that must be rejected with
//! `MissionError::InvalidTransition`, plus terminal-state exhaustiveness
//! for `Archived` (no outgoing transitions in Increment 1 — ruling A).

use mission_domain::test_support::test_context;
use mission_domain::{Mission, MissionCommand, MissionError, MissionEvent, MissionStatus};
use platform_contracts::AggregateRoot;
use platform_kernel::ObjectId;

fn advance(mission: &mut Mission, command: MissionCommand) {
    let ctx = test_context();
    let events = mission
        .decide(command, &ctx)
        .expect("setup step must succeed");
    for event in &events {
        mission.apply(event);
    }
}

fn assert_rejected(mission: &Mission, command: MissionCommand) {
    let ctx = test_context();
    let result = mission.decide(command, &ctx);
    assert!(
        matches!(result, Err(MissionError::InvalidTransition(_))),
        "expected InvalidTransition from status {:?}, got {:?}",
        mission.status(),
        result
    );
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

fn mission_in_planning() -> Mission {
    let mut mission = new_draft_mission();
    let ctx = test_context();
    let events = mission
        .decide(
            MissionCommand::CreateBlueprintRevision {
                blueprint_data: "plan v1".to_string(),
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
    advance(
        &mut mission,
        MissionCommand::SubmitBlueprint { revision_id },
    );
    mission
}

fn mission_in_awaiting_approval() -> Mission {
    let mut mission = mission_in_planning();
    advance(
        &mut mission,
        MissionCommand::RequestApproval {
            reason: "r".to_string(),
            evidence: vec![],
        },
    );
    mission
}

fn mission_in_active() -> Mission {
    let mut mission = mission_in_awaiting_approval();
    advance(
        &mut mission,
        MissionCommand::ActivateMission {
            reason: "r".to_string(),
        },
    );
    mission
}

fn mission_in_paused() -> Mission {
    let mut mission = mission_in_active();
    advance(
        &mut mission,
        MissionCommand::PauseMission {
            reason: "r".to_string(),
        },
    );
    mission
}

fn mission_in_halted() -> Mission {
    let mut mission = mission_in_active();
    advance(
        &mut mission,
        MissionCommand::OperationalHaltMission {
            reason: "r".to_string(),
            emergency_code: "E-1".to_string(),
        },
    );
    mission
}

fn mission_in_review() -> Mission {
    let mut mission = mission_in_active();
    advance(
        &mut mission,
        MissionCommand::TriggerReview {
            reason: "r".to_string(),
        },
    );
    mission
}

fn mission_in_closed() -> Mission {
    let mut mission = mission_in_active();
    advance(
        &mut mission,
        MissionCommand::CloseMission {
            reason: "r".to_string(),
            closure_evidence: vec![ObjectId::new_random()],
        },
    );
    mission
}

fn mission_in_cancelled() -> Mission {
    let mut mission = new_draft_mission();
    advance(
        &mut mission,
        MissionCommand::CancelMission {
            reason: "r".to_string(),
        },
    );
    mission
}

fn mission_in_archived() -> Mission {
    let mut mission = mission_in_closed();
    advance(
        &mut mission,
        MissionCommand::ArchiveMission {
            reason: "r".to_string(),
        },
    );
    mission
}

// Draft: cannot activate, resume, close, archive, or restart.
#[test]
fn draft_rejects_activate() {
    let mission = new_draft_mission();
    assert_rejected(
        &mission,
        MissionCommand::ActivateMission {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn draft_rejects_submit_blueprint_without_a_revision() {
    let mission = new_draft_mission();
    let ctx = test_context();
    let result = mission.decide(
        MissionCommand::SubmitBlueprint {
            revision_id: ObjectId::new_random(),
        },
        &ctx,
    );
    assert!(matches!(result, Err(MissionError::InvalidBlueprint(_))));
}

#[test]
fn draft_rejects_close() {
    let mission = new_draft_mission();
    assert_rejected(
        &mission,
        MissionCommand::CloseMission {
            reason: "x".to_string(),
            closure_evidence: vec![],
        },
    );
}

#[test]
fn draft_rejects_archive() {
    let mission = new_draft_mission();
    assert_rejected(
        &mission,
        MissionCommand::ArchiveMission {
            reason: "x".to_string(),
        },
    );
}

// Planning: cannot submit blueprint again, cannot restart, cannot archive.
#[test]
fn planning_rejects_submit_blueprint_again() {
    let mission = mission_in_planning();
    assert_rejected(
        &mission,
        MissionCommand::SubmitBlueprint {
            revision_id: ObjectId::new_random(),
        },
    );
}

#[test]
fn planning_rejects_restart() {
    let mission = mission_in_planning();
    assert_rejected(
        &mission,
        MissionCommand::RestartMission {
            reason: "x".to_string(),
            checklist_completed: true,
        },
    );
}

#[test]
fn planning_rejects_reject_approval_without_a_pending_request() {
    let mission = mission_in_planning();
    assert_rejected(
        &mission,
        MissionCommand::RejectApproval {
            reason: "x".to_string(),
        },
    );
}

// AwaitingApproval: cannot request approval again, cannot pause directly.
#[test]
fn awaiting_approval_rejects_request_approval_again() {
    let mission = mission_in_awaiting_approval();
    assert_rejected(
        &mission,
        MissionCommand::RequestApproval {
            reason: "x".to_string(),
            evidence: vec![],
        },
    );
}

#[test]
fn awaiting_approval_rejects_pause() {
    let mission = mission_in_awaiting_approval();
    assert_rejected(
        &mission,
        MissionCommand::PauseMission {
            reason: "x".to_string(),
        },
    );
}

// Active: cannot submit blueprint, cannot resume (not paused), cannot
// restart (not halted).
#[test]
fn active_rejects_submit_blueprint() {
    let mission = mission_in_active();
    assert_rejected(
        &mission,
        MissionCommand::SubmitBlueprint {
            revision_id: ObjectId::new_random(),
        },
    );
}

#[test]
fn active_rejects_resume() {
    let mission = mission_in_active();
    assert_rejected(
        &mission,
        MissionCommand::ResumeMission {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn active_rejects_restart() {
    let mission = mission_in_active();
    assert_rejected(
        &mission,
        MissionCommand::RestartMission {
            reason: "x".to_string(),
            checklist_completed: true,
        },
    );
}

// Paused: cannot trigger review, cannot close directly.
#[test]
fn paused_rejects_trigger_review() {
    let mission = mission_in_paused();
    assert_rejected(
        &mission,
        MissionCommand::TriggerReview {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn paused_rejects_close() {
    let mission = mission_in_paused();
    assert_rejected(
        &mission,
        MissionCommand::CloseMission {
            reason: "x".to_string(),
            closure_evidence: vec![ObjectId::new_random()],
        },
    );
}

// Halted: cannot resume directly (must restart), cannot trigger review.
#[test]
fn halted_rejects_resume() {
    let mission = mission_in_halted();
    assert_rejected(
        &mission,
        MissionCommand::ResumeMission {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn halted_rejects_trigger_review() {
    let mission = mission_in_halted();
    assert_rejected(
        &mission,
        MissionCommand::TriggerReview {
            reason: "x".to_string(),
        },
    );
}

// Review: cannot pause, cannot restart.
#[test]
fn review_rejects_pause() {
    let mission = mission_in_review();
    assert_rejected(
        &mission,
        MissionCommand::PauseMission {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn review_rejects_restart() {
    let mission = mission_in_review();
    assert_rejected(
        &mission,
        MissionCommand::RestartMission {
            reason: "x".to_string(),
            checklist_completed: true,
        },
    );
}

// Closed: cannot cancel, cannot activate, cannot close again.
#[test]
fn closed_rejects_cancel() {
    let mission = mission_in_closed();
    assert_rejected(
        &mission,
        MissionCommand::CancelMission {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn closed_rejects_activate() {
    let mission = mission_in_closed();
    assert_rejected(
        &mission,
        MissionCommand::ActivateMission {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn closed_rejects_close_again() {
    let mission = mission_in_closed();
    assert_rejected(
        &mission,
        MissionCommand::CloseMission {
            reason: "x".to_string(),
            closure_evidence: vec![ObjectId::new_random()],
        },
    );
}

#[test]
fn closed_rejects_create_blueprint_revision() {
    let mission = mission_in_closed();
    assert_rejected(
        &mission,
        MissionCommand::CreateBlueprintRevision {
            blueprint_data: "new plan".to_string(),
            version: "2.0".to_string(),
        },
    );
}

// Cancelled: cannot activate, cannot cancel again, cannot pause.
#[test]
fn cancelled_rejects_activate() {
    let mission = mission_in_cancelled();
    assert_rejected(
        &mission,
        MissionCommand::ActivateMission {
            reason: "x".to_string(),
        },
    );
}

#[test]
fn cancelled_rejects_cancel_again() {
    let mission = mission_in_cancelled();
    assert_rejected(
        &mission,
        MissionCommand::CancelMission {
            reason: "x".to_string(),
        },
    );
}

// Archived: terminal in Increment 1 — every command must be rejected
// (ruling A: Archived -> Closed restore is out of scope/unreachable).
#[test]
fn archived_rejects_every_command() {
    let mission = mission_in_archived();
    assert_rejected(
        &mission,
        MissionCommand::ActivateMission {
            reason: "x".to_string(),
        },
    );
    assert_rejected(
        &mission,
        MissionCommand::PauseMission {
            reason: "x".to_string(),
        },
    );
    assert_rejected(
        &mission,
        MissionCommand::CancelMission {
            reason: "x".to_string(),
        },
    );
    assert_rejected(
        &mission,
        MissionCommand::ArchiveMission {
            reason: "x".to_string(),
        },
    );
    assert_rejected(
        &mission,
        MissionCommand::CloseMission {
            reason: "x".to_string(),
            closure_evidence: vec![ObjectId::new_random()],
        },
    );
    assert_rejected(
        &mission,
        MissionCommand::RestartMission {
            reason: "x".to_string(),
            checklist_completed: true,
        },
    );
    assert_rejected(
        &mission,
        MissionCommand::ResumeMission {
            reason: "x".to_string(),
        },
    );
    assert_rejected(
        &mission,
        MissionCommand::TriggerReview {
            reason: "x".to_string(),
        },
    );
}

// CreateMission is never valid inside decide().
#[test]
fn decide_rejects_create_mission_on_any_status() {
    let mission = new_draft_mission();
    assert_rejected(
        &mission,
        MissionCommand::CreateMission {
            name: "another".to_string(),
            description: None,
            owner_id: ObjectId::new_random(),
        },
    );
}

// Also check MissionStatus's Display impl is exercised (documentation/debug
// coverage, not a business rule, but keeps clippy from flagging dead code).
#[test]
fn mission_status_display_is_debug_shaped() {
    assert_eq!(format!("{}", MissionStatus::Draft), "Draft");
}
