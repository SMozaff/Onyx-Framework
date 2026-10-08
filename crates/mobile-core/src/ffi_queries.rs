//! `mobile_core_execute_query` — split out of the original flat
//! `lib.rs`. See `ffi_commands.rs`'s doc comment for the split's
//! provenance.

use std::ffi::c_char;

use client_composition::query_registry::QueryEnvelope;

use crate::{cstr_to_string, string_to_cstr, MobileApp};

/// Executes a query. Returns a JSON string with the result. Team Prompt
/// 5 §3.3.
///
/// # Safety
/// Same contract as `mobile_core_execute_command`.
#[no_mangle]
pub unsafe extern "C" fn mobile_core_execute_query(
    handle: *mut MobileApp,
    query_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    let app = &*handle;
    let Some(query_json) = cstr_to_string(query_json) else {
        return std::ptr::null_mut();
    };
    let Ok(query) = serde_json::from_str::<QueryEnvelope>(&query_json) else {
        return std::ptr::null_mut();
    };

    let result = app
        .runtime
        .block_on(async { app.state.query_registry.dispatch(query).await });

    match result {
        Ok(value) => string_to_cstr(value.to_string()),
        Err(_) => std::ptr::null_mut(),
    }
}
