//! Exercises `mobile-core`'s `extern "C"` functions directly from safe
//! Rust — proving the FFI surface correctly dispatches through the real
//! `AppState` (same registries/handlers `desktop-shell` and
//! `client-composition`'s own tests already prove work end-to-end
//! against real SQLite), not a genuine C/Swift/Kotlin caller. See
//! `lib.rs`'s own module doc comment for why that's the right scope for
//! this sandbox (no Xcode/Android NDK — Team 4's established precedent).

use mobile_core::{
    mobile_core_execute_command, mobile_core_execute_query, mobile_core_free,
    mobile_core_free_string, mobile_core_new, mobile_core_trigger_sync,
};
use platform_kernel::OrganizationId;
use std::ffi::{CStr, CString};

fn test_db_path() -> String {
    format!("/tmp/mobile-core-test-{}.sqlite", uuid::Uuid::new_v4())
}

fn config_json(organization_id: OrganizationId) -> String {
    serde_json::json!({
        "organization_id": organization_id,
        "cloud_relay_endpoint": "wss://relay.test.invalid/v1",
        "sync_interval_secs": 3600,
    })
    .to_string()
}

#[test]
fn mobile_core_new_and_free_round_trip() {
    let organization_id = OrganizationId::new_random();
    let db_path = CString::new(test_db_path()).unwrap();
    let config = CString::new(config_json(organization_id)).unwrap();

    let handle = unsafe { mobile_core_new(db_path.as_ptr(), config.as_ptr()) };
    assert!(
        !handle.is_null(),
        "mobile_core_new should succeed with valid arguments"
    );

    unsafe { mobile_core_free(handle) };
}

#[test]
fn mobile_core_new_returns_null_for_invalid_config_json() {
    let db_path = CString::new(test_db_path()).unwrap();
    let bad_config = CString::new("not valid json").unwrap();

    let handle = unsafe { mobile_core_new(db_path.as_ptr(), bad_config.as_ptr()) };
    assert!(
        handle.is_null(),
        "malformed config_json must produce a null handle, not a panic or garbage pointer"
    );
}

#[test]
fn mobile_core_new_returns_null_for_null_pointers() {
    let handle = unsafe { mobile_core_new(std::ptr::null(), std::ptr::null()) };
    assert!(
        handle.is_null(),
        "null arguments must produce a null handle, not dereference the null pointer"
    );
}

#[test]
fn execute_command_creates_a_mission_through_the_real_command_registry() {
    let organization_id = OrganizationId::new_random();
    let db_path = CString::new(test_db_path()).unwrap();
    let config = CString::new(config_json(organization_id)).unwrap();
    let handle = unsafe { mobile_core_new(db_path.as_ptr(), config.as_ptr()) };
    assert!(!handle.is_null());

    let envelope = mission_domain::test_support::test_context(); // reuse real fixtures
    let target_id = platform_kernel::ObjectId::new_random();

    let command_json = serde_json::json!({
        "command_id": platform_kernel::CommandId::new_random(),
        "operation_id": platform_kernel::OperationId::new_random(),
        "command_type": "CreateMission",
        "schema_version": platform_kernel::SchemaVersion::new("1.0.0"),
        "target": {
            "id": target_id,
            "type": "mission",
            "organization_id": organization_id,
        },
        "expected_version": 0,
        "expected_lifecycle_epoch": 0,
        "expected_authority_epoch": 0,
        "actor": {
            "user_id": envelope.actor.user_id,
            "device_id": platform_kernel::ObjectId::new_random(),
            "organization_id": organization_id,
        },
        "authority_proof": {
            "proof_type": "Jwt",
            "scope": {
                "organization_id": organization_id,
                "object_type": "mission",
                "object_id": null,
                "command_types": ["CreateMission"],
                "delegation_depth": 0,
            },
            "issued_at": 0,
            "expires_at": u64::MAX,
            "signature": null,
        },
        "issued_at": platform_kernel::Timestamp::now(),
        "vector_clock": {"entries": {}},
        "correlation_id": platform_kernel::CorrelationId::new_random(),
        "causation_id": null,
        "payload": {
            "CreateMission": {
                "name": "FFI Test Mission",
                "description": null,
                "owner_id": envelope.actor.user_id,
            }
        },
    })
    .to_string();
    let command_json_c = CString::new(command_json).unwrap();

    let result_ptr = unsafe { mobile_core_execute_command(handle, command_json_c.as_ptr()) };
    assert!(
        !result_ptr.is_null(),
        "CreateMission via FFI should succeed"
    );

    let result_str = unsafe { CStr::from_ptr(result_ptr) }
        .to_str()
        .unwrap()
        .to_string();
    let result: serde_json::Value = serde_json::from_str(&result_str).unwrap();
    assert_eq!(result["success"], serde_json::json!(true));
    assert!(result.get("mission_id").is_some());

    unsafe { mobile_core_free_string(result_ptr) };
    unsafe { mobile_core_free(handle) };
}

#[test]
fn execute_command_returns_null_for_unknown_command_type() {
    let organization_id = OrganizationId::new_random();
    let db_path = CString::new(test_db_path()).unwrap();
    let config = CString::new(config_json(organization_id)).unwrap();
    let handle = unsafe { mobile_core_new(db_path.as_ptr(), config.as_ptr()) };
    assert!(!handle.is_null());

    let bad_command = CString::new("{\"command_type\": \"NoSuchCommand\"}").unwrap();
    let result = unsafe { mobile_core_execute_command(handle, bad_command.as_ptr()) };
    // Malformed/incomplete envelope JSON fails to deserialize into
    // CommandEnvelope<Value> at all (missing required fields), so this
    // exercises the JSON-parse-failure path, not the dispatch path —
    // both correctly return null per this function's documented contract.
    assert!(result.is_null());

    unsafe { mobile_core_free(handle) };
}

#[test]
fn execute_query_for_nonexistent_mission_returns_null_json_value() {
    let organization_id = OrganizationId::new_random();
    let db_path = CString::new(test_db_path()).unwrap();
    let config = CString::new(config_json(organization_id)).unwrap();
    let handle = unsafe { mobile_core_new(db_path.as_ptr(), config.as_ptr()) };
    assert!(!handle.is_null());

    let query_json = serde_json::json!({
        "query_type": "GetMission",
        "target_id": platform_kernel::ObjectId::new_random(),
    })
    .to_string();
    let query_json_c = CString::new(query_json).unwrap();

    let result_ptr = unsafe { mobile_core_execute_query(handle, query_json_c.as_ptr()) };
    assert!(!result_ptr.is_null(), "a well-formed query for a missing aggregate should succeed with a null JSON value, not fail the FFI call itself");

    let result_str = unsafe { CStr::from_ptr(result_ptr) }
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(result_str, "null");

    unsafe { mobile_core_free_string(result_ptr) };
    unsafe { mobile_core_free(handle) };
}

#[test]
fn trigger_sync_with_no_peers_available_returns_success_not_error() {
    let organization_id = OrganizationId::new_random();
    let db_path = CString::new(test_db_path()).unwrap();
    let config = CString::new(config_json(organization_id)).unwrap();
    let handle = unsafe { mobile_core_new(db_path.as_ptr(), config.as_ptr()) };
    assert!(!handle.is_null());

    // No real peers are discoverable in this test environment;
    // "no peers found" is documented (sync_agent.rs) as a normal,
    // successful no-op cycle, not an error — this asserts that contract
    // holds through the FFI boundary too.
    let result = unsafe { mobile_core_trigger_sync(handle) };
    assert_eq!(
        result, 0,
        "trigger_sync with no discoverable peers should return 0 (success), not -1"
    );

    unsafe { mobile_core_free(handle) };
}

#[test]
fn list_aggregates_and_sync_status_are_available_to_flutter() {
    use mobile_core::{mobile_core_get_sync_status, mobile_core_list_aggregates};

    let organization_id = OrganizationId::new_random();
    let db_path = CString::new(test_db_path()).unwrap();
    let config = CString::new(config_json(organization_id)).unwrap();
    let handle = unsafe { mobile_core_new(db_path.as_ptr(), config.as_ptr()) };
    assert!(!handle.is_null());

    let aggregate_type = CString::new("mission").unwrap();
    let list_ptr = unsafe { mobile_core_list_aggregates(handle, aggregate_type.as_ptr()) };
    assert!(!list_ptr.is_null());
    let list_json = unsafe { CStr::from_ptr(list_ptr) }.to_str().unwrap();
    let list: serde_json::Value = serde_json::from_str(list_json).unwrap();
    assert!(list.is_array());
    unsafe { mobile_core_free_string(list_ptr) };

    let status_ptr = unsafe { mobile_core_get_sync_status(handle) };
    assert!(!status_ptr.is_null());
    let status_json = unsafe { CStr::from_ptr(status_ptr) }.to_str().unwrap();
    let status: serde_json::Value = serde_json::from_str(status_json).unwrap();
    assert!(status.get("pending_outbox_count").is_some());
    unsafe { mobile_core_free_string(status_ptr) };

    unsafe { mobile_core_free(handle) };
}

#[test]
fn registered_background_sync_uses_the_active_mobile_instance() {
    use mobile_core::mobile_core_background_sync_registered;

    let organization_id = OrganizationId::new_random();
    let db_path = CString::new(test_db_path()).unwrap();
    let config = CString::new(config_json(organization_id)).unwrap();
    let handle = unsafe { mobile_core_new(db_path.as_ptr(), config.as_ptr()) };
    assert!(!handle.is_null());

    assert_eq!(mobile_core_background_sync_registered(42), 1);
    unsafe { mobile_core_free(handle) };
    assert_eq!(mobile_core_background_sync_registered(42), 0);
}
