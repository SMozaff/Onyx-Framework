//! Tests for `Mission::create()` (ruling C: creation bypasses `decide()`).

use mission_domain::test_support::{test_context, test_user_id};
use mission_domain::{Mission, MissionCommand, MissionError, MissionEvent, MissionStatus};
use platform_contracts::AggregateRoot;

#[test]
fn create_mission_with_valid_name_succeeds() {
    let ctx = test_context();
    let owner = test_user_id();
    let events = Mission::create(
        MissionCommand::CreateMission {
            name: "Operation Nightfall".to_string(),
            description: Some("A test mission".to_string()),
            owner_id: owner,
        },
        &ctx,
    )
    .expect("creation should succeed");

    assert_eq!(events.len(), 1);
    match &events[0] {
        MissionEvent::MissionCreated { name, owner_id, .. } => {
            assert_eq!(name, "Operation Nightfall");
            assert_eq!(*owner_id, owner);
        }
        other => panic!("expected MissionCreated, got {:?}", other),
    }
}

#[test]
fn create_mission_with_empty_name_fails() {
    let ctx = test_context();
    let owner = test_user_id();
    let result = Mission::create(
        MissionCommand::CreateMission {
            name: "   ".to_string(),
            description: None,
            owner_id: owner,
        },
        &ctx,
    );
    assert_eq!(result, Err(MissionError::MissingField("name".to_string())));
}

#[test]
fn create_mission_rejects_non_create_command() {
    let ctx = test_context();
    let result = Mission::create(
        MissionCommand::CancelMission {
            reason: "oops".to_string(),
        },
        &ctx,
    );
    assert!(matches!(result, Err(MissionError::InvalidTransition(_))));
}

#[test]
fn rehydrated_mission_starts_in_draft() {
    let ctx = test_context();
    let owner = test_user_id();
    let events = Mission::create(
        MissionCommand::CreateMission {
            name: "Operation Nightfall".to_string(),
            description: None,
            owner_id: owner,
        },
        &ctx,
    )
    .unwrap();

    let mission = Mission::from_created_event(&events[0]);
    assert_eq!(mission.status(), MissionStatus::Draft);
    assert_eq!(mission.name(), "Operation Nightfall");
    assert_eq!(mission.owner_id(), owner);
    assert_eq!(mission.version().0, 0);
}

#[test]
#[should_panic(expected = "from_created_event called with a non-MissionCreated event")]
fn from_created_event_panics_on_wrong_event_type() {
    let event = MissionEvent::MissionCancelled {
        cancelled_at: platform_kernel::Timestamp::from_nanos(0),
        reason: "x".to_string(),
    };
    let _ = Mission::from_created_event(&event);
}

/// Creating a mission then applying its own creation event again must be a
/// safe no-op (replay idempotency for the `MissionCreated` branch of
/// `apply()`).
#[test]
fn applying_mission_created_again_is_a_noop() {
    let ctx = test_context();
    let owner = test_user_id();
    let events = Mission::create(
        MissionCommand::CreateMission {
            name: "Operation Nightfall".to_string(),
            description: None,
            owner_id: owner,
        },
        &ctx,
    )
    .unwrap();

    let mut mission = Mission::from_created_event(&events[0]);
    let before_status = mission.status();
    let before_name = mission.name().to_string();

    mission.apply(&events[0]);

    assert_eq!(mission.status(), before_status);
    assert_eq!(mission.name(), before_name);
    // version still increments even for a no-op branch, per apply()'s
    // uniform post-match `self.version = self.version.next()`.
    assert_eq!(mission.version().0, 1);
}
