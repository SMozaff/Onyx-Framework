//! Mobile-oriented FFI helpers required by the Flutter v1.1 client.
//! These functions preserve the delivered command/query boundary while
//! exposing list projections, sync state, and human conflict resolution in
//! JSON form suitable for Dart FFI.

use std::ffi::{c_char, c_int};

use platform_kernel::ConflictId;
use synchronization_domain::ConflictResolution;

use crate::{cstr_to_string, string_to_cstr, MobileApp};

/// Lists locally stored aggregate projections for one aggregate type.
/// Returns a JSON array ordered by most recently updated first.
///
/// # Safety
/// `handle` must be a valid pointer from `mobile_core_new`. `aggregate_type`
/// must be a valid, NUL-terminated C string pointer.
#[no_mangle]
pub unsafe extern "C" fn mobile_core_list_aggregates(
    handle: *mut MobileApp,
    aggregate_type: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    let Some(aggregate_type) = cstr_to_string(aggregate_type) else {
        return std::ptr::null_mut();
    };
    let app = &*handle;
    let result = app.runtime.block_on(async {
        sqlx::query_as::<_, (Vec<u8>, String, i64, i64, i64, i64)>(
            r#"SELECT id, state, version, lifecycle_epoch, authority_epoch, updated_at
               FROM aggregates
               WHERE aggregate_type = ?
               ORDER BY updated_at DESC"#,
        )
        .bind(aggregate_type)
        .fetch_all(&app.state.pool)
        .await
    });

    let Ok(rows) = result else {
        return std::ptr::null_mut();
    };
    let values: Vec<serde_json::Value> = rows
        .into_iter()
        .filter_map(
            |(id, state, version, lifecycle_epoch, authority_epoch, updated_at)| {
                let aggregate = serde_json::from_str::<serde_json::Value>(&state).ok()?;
                Some(serde_json::json!({
                    "id": id,
                    "aggregate": aggregate,
                    "version": version,
                    "lifecycle_epoch": lifecycle_epoch,
                    "authority_epoch": authority_epoch,
                    "updated_at": updated_at,
                }))
            },
        )
        .collect();
    string_to_cstr(serde_json::Value::Array(values).to_string())
}

/// Returns the current local-first synchronization state as JSON.
///
/// # Safety
/// `handle` must be a valid pointer from `mobile_core_new`.
#[no_mangle]
pub unsafe extern "C" fn mobile_core_get_sync_status(handle: *mut MobileApp) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    let app = &*handle;
    let status = app
        .runtime
        .block_on(async { app.state.sync_agent.status().await });
    match serde_json::to_string(&status) {
        Ok(json) => string_to_cstr(json),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Returns all currently open synchronization conflicts as JSON.
///
/// # Safety
/// `handle` must be a valid pointer from `mobile_core_new`.
#[no_mangle]
pub unsafe extern "C" fn mobile_core_list_conflicts(handle: *mut MobileApp) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    let app = &*handle;
    let conflicts = app
        .runtime
        .block_on(async { app.state.sync_agent.open_conflicts().await });
    match serde_json::to_string(&conflicts) {
        Ok(json) => string_to_cstr(json),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Resolves an open conflict using `local`, `remote`, or `escalate`.
/// `conflict_json` must contain the serialized `conflict_id` field returned
/// by `mobile_core_list_conflicts`.
///
/// # Safety
/// `handle` must be a valid pointer from `mobile_core_new`. `conflict_json`
/// and `resolution` must each be a valid, NUL-terminated C string pointer.
#[no_mangle]
pub unsafe extern "C" fn mobile_core_resolve_conflict(
    handle: *mut MobileApp,
    conflict_json: *const c_char,
    resolution: *const c_char,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    let Some(conflict_json) = cstr_to_string(conflict_json) else {
        return -1;
    };
    let Some(resolution) = cstr_to_string(resolution) else {
        return -1;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&conflict_json) else {
        return -1;
    };
    let Some(id_value) = value.get("conflict_id") else {
        return -1;
    };
    let Ok(conflict_id) = serde_json::from_value::<ConflictId>(id_value.clone()) else {
        return -1;
    };
    let resolution = match resolution.as_str() {
        "local" => ConflictResolution::LocalWins,
        "remote" => ConflictResolution::RemoteWins,
        "escalate" => ConflictResolution::Escalated,
        _ => return -1,
    };
    let app = &*handle;
    if app.runtime.block_on(async {
        app.state
            .sync_agent
            .resolve_conflict(conflict_id, resolution)
            .await
    }) {
        0
    } else {
        1
    }
}
