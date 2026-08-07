//! Golden fixture tests for Task events/commands and the shared
//! `DomainErrorResponse` wire contract. Marked `#[ignore]` per §10.

use platform_contracts::{DomainError, DomainErrorResponse};
use platform_kernel::{CorrelationId, ObjectId, Timestamp};
use work_domain::{TaskCommand, TaskEvent, TaskId};

#[test]
#[ignore]
fn golden_task_created_event_shape() {
    let event = TaskEvent::TaskCreated {
        task_id: TaskId(ObjectId([1u8; 16])),
        mission_id: mission_domain::MissionId(ObjectId([2u8; 16])),
        title: "Golden Task".to_string(),
        owner_id: ObjectId([3u8; 16]),
        created_at: Timestamp::from_nanos(1_700_000_000_000_000_000),
    };
    let value = serde_json::to_value(&event).unwrap();

    assert!(value.get("TaskCreated").is_some());
    let inner = &value["TaskCreated"];
    assert_eq!(inner["title"], "Golden Task");
    assert!(inner.get("task_id").is_some());
    assert!(inner.get("mission_id").is_some());
    assert!(inner.get("owner_id").is_some());
}

#[test]
#[ignore]
fn golden_task_reopened_event_shape() {
    // Locks in ruling T1's reinstated event, including authorized_by.
    let event = TaskEvent::TaskReopened {
        reopened_at: Timestamp::from_nanos(0),
        reason: "needs more work".to_string(),
        authorized_by: ObjectId([4u8; 16]),
    };
    let value = serde_json::to_value(&event).unwrap();
    assert!(value.get("TaskReopened").is_some());
    assert!(value["TaskReopened"].get("authorized_by").is_some());
}

#[test]
#[ignore]
fn golden_reopen_task_command_shape() {
    let command = TaskCommand::ReopenTask {
        reason: "needs more work".to_string(),
        authorized_by: ObjectId([5u8; 16]),
    };
    let value = serde_json::to_value(&command).unwrap();
    assert!(value.get("ReopenTask").is_some());
    assert!(value["ReopenTask"].get("authorized_by").is_some());
}

#[test]
#[ignore]
fn golden_task_domain_error_response_matches_handover_contract() {
    let error = DomainError::InvalidArgument {
        field: "evidence".to_string(),
        reason: "must not be empty".to_string(),
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
    assert_eq!(keys, expected);
    assert_eq!(value["code"], "INVALID_ARGUMENT");
}
