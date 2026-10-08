//! `mobile_core_execute_command` — split out of the original flat
//! `lib.rs` per the requested file structure.
//!
//! # Error-surfacing change (mobile Approvals task)
//! A dispatch error (e.g. `CommandError::OwnerAuthorityDenied`, the real
//! gate `ApproveTask`/`RejectTask`/`RejectApproval`/`ActivateMission` are
//! checked against) used to collapse to a bare null pointer here, same
//! as a malformed/undecodable envelope — the Dart side could not tell
//! "you are not authorized" apart from "the request was garbage", and
//! surfaced both as the same generic `StateError('mobile-core returned
//! null')`. That was a real gap, not a style choice: the whole point of
//! wiring Approvals to the real backend gate is that most actors
//! legitimately cannot approve most tasks, so a denial is an expected,
//! common outcome that needs a specific, diagnosable message, not a
//! collapsed one indistinguishable from a client bug. A dispatch error
//! now serializes to `{"success": false, "error": "<Display message>"}`
//! instead — reusing the same `"success"` boolean key the real success
//! payload already uses (`command_handler::handle_command`'s
//! `json!({"success": true, ...})`), so there is one boolean callers
//! check either way, not two different shapes to distinguish.
//!
//! Still null, deliberately unchanged: a null/invalid `handle`, an
//! un-decodable `command_json` C string, or an envelope that fails to
//! deserialize into `CommandEnvelope<Value>` at all (missing required
//! fields) — these are caller-side malformed-input bugs the FFI
//! boundary itself rejects before any dispatch is attempted, genuinely
//! different from a real, well-formed command the domain layer decided
//! to reject; `tests/ffi_integration.rs`'s
//! `execute_command_returns_null_for_unknown_command_type` still covers
//! exactly this path and needed no change.

use std::ffi::c_char;

use platform_contracts::CommandEnvelope;

use crate::{cstr_to_string, string_to_cstr, MobileApp};

/// Executes a command. Returns a JSON string (the caller must free it via
/// `mobile_core_free_string`): either the real success payload (as
/// `command_handler::handle_command` produces it, `"success": true`
/// among other fields), or `{"success": false, "error": "<message>"}` if
/// dispatch itself rejected the command (see this module's doc comment).
/// Returns null only for a malformed FFI call itself — an invalid
/// `handle`, an undecodable `command_json` string, or an envelope that
/// doesn't even parse into `CommandEnvelope<Value>` — never for a
/// well-formed command the domain layer decided to reject. Team Prompt 5
/// §3.3.
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
        Err(error) => string_to_cstr(
            serde_json::json!({
                "success": false,
                "error": error.to_string(),
            })
            .to_string(),
        ),
    }
}
