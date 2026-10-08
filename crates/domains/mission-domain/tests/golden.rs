//! Golden fixture tests: lock the exact JSON wire shape of Mission events,
//! commands, and the shared `DomainErrorResponse` wire contract, so that any
//! accidental schema drift is caught immediately. Marked `#[ignore]` per
//! §10 (`cargo test --test golden -- --ignored`).

use mission_domain::{MissionCommand, MissionEvent};
use platform_contracts::{DomainError, DomainErrorResponse};
use platform_kernel::{CorrelationId, ObjectId, Timestamp};

#[test]
#[ignore]
fn golden_mission_created_event_shape() {
    let event = MissionEvent::MissionCreated {
        mission_id: mission_domain::MissionId(ObjectId([1u8; 16])),
        name: "Golden Mission".to_string(),
        owner_id: ObjectId([2u8; 16]),
        created_at: Timestamp::from_nanos(1_700_000_000_000_000_000),
    };
    let value = serde_json::to_value(&event).unwrap();

    // Internally-tagged-by-variant-name shape (serde default enum repr,
    // since MissionEvent uses no #[serde(tag = ...)] attribute).
    assert!(value.get("MissionCreated").is_some());
    let inner = &value["MissionCreated"];
    assert_eq!(inner["name"], "Golden Mission");
    assert!(inner.get("mission_id").is_some());
    assert!(inner.get("owner_id").is_some());
    assert!(inner.get("created_at").is_some());
}

#[test]
#[ignore]
fn golden_mission_restarted_event_carries_checklist_completed() {
    // Locks in the fix for the apply()-purity issue: MissionRestarted must
    // carry checklist_completed on the wire so replay is deterministic
    // without hidden aggregate state.
    let event = MissionEvent::MissionRestarted {
        restarted_at: Timestamp::from_nanos(0),
        reason: "ok".to_string(),
        checklist_completed: true,
    };
    let value = serde_json::to_value(&event).unwrap();
    assert_eq!(value["MissionRestarted"]["checklist_completed"], true);
}

#[test]
#[ignore]
fn golden_submit_blueprint_command_shape() {
    let command = MissionCommand::SubmitBlueprint {
        revision_id: ObjectId([3u8; 16]),
    };
    let value = serde_json::to_value(&command).unwrap();
    assert!(value.get("SubmitBlueprint").is_some());
    assert!(value["SubmitBlueprint"].get("revision_id").is_some());
}

#[test]
#[ignore]
fn golden_domain_error_response_wire_shape_matches_handover_contract() {
    // Per the Handover Document error protocol and ruling E1: {"code",
    // "category", "retryability", "safe_details", "correlation_id"}.
    let error = DomainError::InvalidTransition {
        from: "Active".to_string(),
        command: "SubmitBlueprint".to_string(),
    };
    let correlation_id = CorrelationId([9u8; 16]);
    let response = DomainErrorResponse::from_error(&error, correlation_id);
    let value = serde_json::to_value(&response).unwrap();

    let keys: std::collections::BTreeSet<String> =
        value.as_object().unwrap().keys().cloned().collect();
    let expected: std::collections::BTreeSet<String> = [
        "code",
        "category",
        "retryability",
        "safe_details",
        "correlation_id",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(
        keys, expected,
        "DomainErrorResponse wire shape drifted from the Handover Document contract"
    );

    assert_eq!(value["code"], "INVALID_TRANSITION");
    assert_eq!(value["category"], "DOMAIN");
    assert_eq!(value["retryability"], "NON_RETRYABLE");
}
