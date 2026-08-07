//! `mobile_core_execute_command` — split out of the original flat
//! `lib.rs` per the requested file structure. No behavior change;
//! verified by re-running `tests/ffi_integration.rs`'s command-execution
//! tests unmodified after this split (see `DECISIONS.md`).

use std::ffi::c_char;

use platform_contracts::CommandEnvelope;

use crate::{cstr_to_string, string_to_cstr, MobileApp};

/// Executes a command. Returns a JSON string with the result (the caller
/// must free it via `mobile_core_free_string`), or null on failure
/// (invalid arguments, or a dispatch error — the error itself is not
/// currently surfaced to the caller as anything richer than "null";
/// flagged, not silently treated as complete, since Team Prompt 5 §3.3
/// doesn't specify an error-reporting mechanism for this function beyond
/// "returns JSON string with the result"). Team Prompt 5 §3.3.
///
/// # Safety
/// `handle` must be a valid pointer from `mobile_core_new`. `command_json`
/// must be a valid, NUL-terminated C string pointer.
#[no_mangle]
pub unsafe extern "C" fn mobile_core_execute_command(
    handle: *mut MobileApp,
    command_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    let app = &*handle;
    let Some(command_json) = cstr_to_string(command_json) else {
        return std::ptr::null_mut();
    };
    let Ok(envelope) = serde_json::from_str::<CommandEnvelope<serde_json::Value>>(&command_json)
    else {
        return std::ptr::null_mut();
    };

    let result = app
        .runtime
        .block_on(async { app.state.command_registry.dispatch(envelope).await });

    match result {
        Ok(value) => string_to_cstr(value.to_string()),
        Err(_) => std::ptr::null_mut(),
    }
}
