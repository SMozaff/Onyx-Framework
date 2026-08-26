//! End-to-end proof that mobile's owner-authority gate actually works,
//! through the real `extern "C"` FFI surface (same standard as
//! `ffi_integration.rs` and `client-composition`'s own
//! `task_owner_authority_gate.rs`): the FFI boundary, not an in-process
//! shortcut, is what a real Dart caller would go through.
//!
//! Confirms:
//! - Before `mobile_core_set_hierarchy` is ever called, `ApproveTask` is
//!   denied for everyone (the safe, fail-closed default `HierarchyCache`
//!   provides when it has no cached entries at all).
//! - After `mobile_core_set_hierarchy` loads a real hierarchy, an
//!   unrelated stranger is still denied `ApproveTask`.
//! - The task owner's real, cache-resolved direct manager can approve.
//! - `SubmitCompletion` (the owner acting on their own work) succeeds
//!   ungated throughout, unaffected by any of the above.
//! - State is reloaded via a fresh `GetTask` query afterward to confirm
//!   persistence, not just an in-memory result.

use mobile_core::{
    mobile_core_execute_command, mobile_core_execute_query, mobile_core_free,
    mobile_core_free_string, mobile_core_new, mobile_core_set_hierarchy,
};
use platform_kernel::{ObjectId, OrganizationId};
use std::ffi::{CStr, CString};

fn test_db_path() -> String {
    std::env::temp_dir()
        .join(format!("mobile-core-hierarchy-test-{}.sqlite", uuid::Uuid::new_v4()))
        .to_string_lossy()
        .into_owned()
}

fn config_json(organization_id: OrganizationId) -> String {
    serde_json::json!({
        "organization_id": organization_id,
        "cloud_relay_endpoint": "wss://relay.test.invalid/v1",
        "sync_interval_secs": 3600,
    })
    .to_string()
}

fn command_envelope(
    command_type: &str,
    target_id: ObjectId,
    organization_id: OrganizationId,
    expected_version: u64,
    actor_user_id: ObjectId,
    payload: serde_json::Value,
) -> String {
    serde_json::json!({
        "command_id": platform_kernel::CommandId::new_random(),
        "operation_id": platform_kernel::OperationId::new_random(),
        "command_type": command_type,
        "schema_version": platform_kernel::SchemaVersion::new("1.0.0"),
        "target": {
            "id": target_id,
            "type": "task",
            "organization_id": organization_id,
        },
        "expected_version": expected_version,
        // Every decision command exercised here causes a status
        // transition, and lifecycle_epoch advances by one on every such
        // transition (same reasoning as client-composition's
        // task_owner_authority_gate.rs) -- both start at 0 after
        // CreateTask, so lifecycle_epoch tracks expected_version.
        "expected_lifecycle_epoch": expected_version,
        "expected_authority_epoch": 0,
        "actor": {
            "user_id": actor_user_id,
            "device_id": ObjectId::new_random(),
            "organization_id": organization_id,
        },
        "authority_proof": {
            "proof_type": "Jwt",
            "scope": {
                "organization_id": organization_id,
                "object_type": "task",
                "object_id": null,
                "command_types": [command_type],
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
        "payload": payload,
    })
    .to_string()
}

fn execute(handle: *mut mobile_core::MobileApp, envelope_json: String) -> Result<serde_json::Value, ()> {
    let c = CString::new(envelope_json).unwrap();
    let result_ptr = unsafe { mobile_core_execute_command(handle, c.as_ptr()) };
    if result_ptr.is_null() {
        return Err(());
    }
    let result_str = unsafe { CStr::from_ptr(result_ptr) }.to_str().unwrap().to_string();
    unsafe { mobile_core_free_string(result_ptr) };
    Ok(serde_json::from_str(&result_str).unwrap())
}

#[test]
fn owner_submits_manager_approves_stranger_denied_through_real_ffi() {
    let organization_id = OrganizationId::new_random();
    let db_path = CString::new(test_db_path()).unwrap();
    let config = CString::new(config_json(organization_id)).unwrap();
    let handle = unsafe { mobile_core_new(db_path.as_ptr(), config.as_ptr()) };
    assert!(!handle.is_null(), "mobile_core_new should succeed");

    let owner = ObjectId::new_random();
    let manager = ObjectId::new_random();
    let stranger = ObjectId::new_random();

    // 1. Create the task, owned by `owner`, before any hierarchy is
    // loaded at all.
    let create_result = execute(
        handle,
        command_envelope(
            "CreateTask",
            ObjectId::new_random(),
            organization_id,
            0,
            owner,
            serde_json::json!({
                "CreateTask": {
                    "mission_id": ObjectId::new_random(),
                    "title": "Mobile owner-authority gate test task",
                    "description": null,
                    "owner_id": owner,
                }
            }),
        ),
    )
    .expect("CreateTask should succeed");
    let task_id: ObjectId = serde_json::from_value(create_result["task_id"].clone()).unwrap();

    // 2. Draft -> Ready -> Active -> Submitted, all as the owner (none
    // of these are owner-gated).
    execute(
        handle,
        command_envelope("MarkReady", task_id, organization_id, 0, owner, serde_json::json!({"MarkReady": {"reason": "ready"}})),
    )
    .expect("MarkReady should succeed");
    execute(
        handle,
        command_envelope("StartTask", task_id, organization_id, 1, owner, serde_json::json!({"StartTask": {"reason": "starting"}})),
    )
    .expect("StartTask should succeed");
    execute(
        handle,
        command_envelope(
            "SubmitCompletion",
            task_id,
            organization_id,
            2,
            owner,
            serde_json::json!({"SubmitCompletion": {"evidence": [ObjectId::new_random()]}}),
        ),
    )
    .expect(
        "SubmitCompletion must succeed ungated for the task's own owner, \
         with or without a hierarchy loaded -- it is not an owner-gated command",
    );

    // 3. Before any hierarchy is loaded, ApproveTask must be denied for
    // everyone -- the safe, fail-closed default.
    let denied_before_hierarchy = execute(
        handle,
        command_envelope("ApproveTask", task_id, organization_id, 3, manager, serde_json::json!({"ApproveTask": {"reason": "approving"}})),
    );
    assert!(
        denied_before_hierarchy.is_err(),
        "ApproveTask must be denied before mobile_core_set_hierarchy has ever been called, \
         even for who would become the real manager -- an empty cache authorizes no one"
    );

    // 4. Load a real hierarchy via the FFI function under test:
    // `manager` is `owner`'s direct manager; `stranger` has no relation.
    let hierarchy_json = serde_json::json!([
        {"id": owner.to_string(), "parent_user_id": manager.to_string(), "is_admin": false},
        {"id": manager.to_string(), "parent_user_id": null, "is_admin": false},
        {"id": stranger.to_string(), "parent_user_id": null, "is_admin": false},
    ])
    .to_string();
    let hierarchy_c = CString::new(hierarchy_json).unwrap();
    let set_result = unsafe { mobile_core_set_hierarchy(handle, hierarchy_c.as_ptr()) };
    assert_eq!(set_result, 0, "mobile_core_set_hierarchy should succeed for well-formed input");

    // 5. A stranger with no authority relationship to `owner` is denied.
    let denied = execute(
        handle,
        command_envelope("ApproveTask", task_id, organization_id, 3, stranger, serde_json::json!({"ApproveTask": {"reason": "approving"}})),
    );
    assert!(
        denied.is_err(),
        "an unrelated, non-manager user must still be denied ApproveTask after the hierarchy is loaded"
    );

    // 6. The task's real, cache-resolved direct manager can approve.
    let approved = execute(
        handle,
        command_envelope("ApproveTask", task_id, organization_id, 3, manager, serde_json::json!({"ApproveTask": {"reason": "approving"}})),
    )
    .expect("the task owner's real direct manager must be authorized to approve");
    assert_eq!(approved["success"], serde_json::json!(true));

    // 7. Reload via a fresh GetTask query to confirm the approval
    // genuinely persisted, not just an in-memory result.
    let query_json = serde_json::json!({"query_type": "GetTask", "target_id": task_id}).to_string();
    let query_c = CString::new(query_json).unwrap();
    let query_result_ptr = unsafe { mobile_core_execute_query(handle, query_c.as_ptr()) };
    assert!(!query_result_ptr.is_null());
    let query_result_str = unsafe { CStr::from_ptr(query_result_ptr) }.to_str().unwrap().to_string();
    unsafe { mobile_core_free_string(query_result_ptr) };
    let query_result: serde_json::Value = serde_json::from_str(&query_result_str).unwrap();
    assert_eq!(
        query_result["aggregate"]["status"],
        serde_json::json!("Approved"),
        "status must have genuinely persisted as Approved after the manager's approval, \
         confirmed via a fresh query rather than trusting the command's own response"
    );

    unsafe { mobile_core_free(handle) };
}
