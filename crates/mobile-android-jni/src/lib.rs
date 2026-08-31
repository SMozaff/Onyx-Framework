//! Thin JNI adapter over `mobile-core`'s existing C ABI, for Android
//! Work Package A1 (ONYX-MOB-00 §11 / ONYX-MOB-01 §12).
//!
//! # Architecture decision — wrap the C ABI directly, not a JNI-native
//! # `mobile-core`
//! `mobile-core` already exports a plain C ABI (`mobile-core.h`,
//! auto-generated via `cbindgen`) built and consumed today by Dart's
//! `dart:ffi` on both Android and iOS. Two real options existed for
//! Kotlin:
//!
//! 1. This crate: a separate, thin Rust crate depending on `mobile-core`
//!    as a normal path dependency, exposing `Java_com_onyx_...` JNI
//!    entry points that marshal JNI types into the exact same `*mut
//!    MobileApp`/`*const c_char` types `mobile-core`'s C functions
//!    already take, then call those functions directly (an ordinary
//!    Rust function call across the crate boundary -- no
//!    `dlopen`/`dlsym`, since `mobile_core_new` etc. are `pub` Rust
//!    items re-exported from `mobile_core`'s crate root, not only C
//!    symbols).
//! 2. Skip a dedicated Rust JNI crate entirely: have Kotlin's `external
//!    fun` declarations bind straight to `mobile-core`'s own `#[no_mangle]
//!    extern "C"` functions, the way `mobile/android/app/src/main/kotlin/
//!    com/onyx/WorkManagerService.kt`'s `nativeAndroidDoWork(): Int`
//!    already does for `mobile_core_android_do_work`.
//!
//! **Option 2 does not generalize**, confirmed by reading `mobile-core`'s
//! own real exported signatures rather than assuming either option
//! works uniformly: `mobile_core_android_do_work(handle: *mut MobileApp,
//! _env: *mut c_void, _thiz: *mut c_void) -> c_int` takes no string or
//! JNI-object arguments at all -- a JVM `external fun` returning `Int`
//! with no parameters happens to line up with a JNI-callable native
//! method signature by coincidence of having nothing to marshal. Every
//! other real function that matters for the rewrite --
//! `mobile_core_execute_command`/`_execute_query` (JSON string in,
//! JSON string out), `mobile_core_new` (two strings in, opaque pointer
//! out), `mobile_core_upload_file`/`_download_file` -- takes `*const
//! c_char`/`*mut c_char`, which is not a JNI-compatible parameter type.
//! JNI requires every native method's real parameters to be JNI object
//! types (`jstring`, `jobject`, ...) or primitives, never a raw
//! `char*` -- Java-side strings arrive as opaque `jstring` references
//! that must be explicitly converted, so calling
//! `mobile_core_execute_command` directly as a JNI native-method target
//! is not possible without exactly the marshalling layer this crate
//! provides. Option 2 is real only for the one coincidentally
//! all-primitive function already using it; it was not silently
//! generalized to the rest of the surface.
//!
//! This crate therefore wraps the functions that actually need
//! marshalling. It contains **no business logic** (per the manifesto's
//! explicit prohibition) -- every wrapper's body is: convert JNI
//! arguments to the C ABI's native types, call straight into
//! `mobile_core::*`, convert the result back, done.
//!
//! # Scope
//! Per A1's own "prove the connection, don't build every wrapper"
//! framing, and A2's note that real JNI adapter test coverage may still
//! be a gap A3+ needs to close: this crate wraps handle lifecycle
//! (`mobile_core_new`/`mobile_core_free`), one representative
//! string-round-trip function (`mobile_core_execute_command`) -- the
//! harder marshalling case (JSON string in, JSON string out, not just
//! an opaque pointer), proving the pattern generalizes rather than
//! proving only the trivial handle-only case -- and, added in A3,
//! `mobile_core_set_hierarchy` (needed for real login/session startup).
//! The remaining ~14 functions follow this exact same pattern (see
//! `execute_command`
//! below as the template) and are deliberately left for the task that
//! actually needs them, per A1's "do not build beyond the minimal
//! skeleton" instruction.
//!
//! # `jni` 0.22's `Env`/`EnvUnowned` split
//! Confirmed the hard way: this module was first drafted against the
//! pre-0.22 single-`JNIEnv` API (Context7 has no indexed docs for this
//! crate under any of "jni"/"jni-rs"/"jni crate rust" -- checked, not
//! assumed -- so the version was confirmed against crates.io directly:
//! 0.22.4, current stable), and failed to compile with a hard error
//! naming the real, current split: a native method receives an
//! FFI-safe `EnvUnowned`, then calls `with_env(|env| ...)` to get a
//! real `Env` for the duration of one closure. Every wrapper below
//! follows that real, current pattern (read directly from
//! `jni-0.22.4`'s own source and doc examples, not a remembered older
//! shape).
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use jni::errors::{Error as JniError, LogErrorAndDefault};
use jni::objects::{JClass, JString};
use jni::sys::{jlong, jstring};
use jni::EnvUnowned;

use mobile_core::MobileApp;

/// `Java_com_onyx_bridge_MobileCoreBridge_nativeNew` --
/// `com.onyx.bridge.MobileCoreBridge.nativeNew(dbPath: String, configJson: String): Long`.
///
/// Returns the raw handle as a `jlong` (`0` on failure, matching
/// `mobile_core_new`'s null-on-failure convention -- `0` is not a valid
/// non-null pointer value on any platform this project targets). The
/// Kotlin side stores this `Long` and passes it back into every other
/// call as the session handle, exactly mirroring how `mobile-core`'s C
/// callers already treat `*mut MobileApp` as an opaque token.
#[no_mangle]
pub extern "system" fn Java_com_onyx_bridge_MobileCoreBridge_nativeNew<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    db_path: JString<'local>,
    config_json: JString<'local>,
) -> jlong {
    env.with_env(|env| -> Result<jlong, JniError> {
        let Some(db_path) = jstring_to_cstring(env, &db_path) else {
            return Ok(0);
        };
        let Some(config_json) = jstring_to_cstring(env, &config_json) else {
            return Ok(0);
        };
        // Safety: db_path/config_json are freshly built, valid,
        // NUL-terminated C strings, satisfying mobile_core_new's own
        // safety contract.
        let handle =
            unsafe { mobile_core::mobile_core_new(db_path.as_ptr(), config_json.as_ptr()) };
        Ok(handle as jlong)
    })
    .resolve::<LogErrorAndDefault>()
}

/// `Java_com_onyx_bridge_MobileCoreBridge_nativeFree` --
/// `com.onyx.bridge.MobileCoreBridge.nativeFree(handle: Long)`.
///
/// # Safety (of the underlying call this wraps)
/// `handle` must be a value previously returned by `nativeNew` on this
/// same process, not yet freed, and not used again after this call --
/// identical contract to `mobile_core_free` itself, which this function
/// is a direct pass-through to. The JNI entry point itself is safe
/// Rust; the `unsafe` is confined to the one call it makes.
#[no_mangle]
pub extern "system" fn Java_com_onyx_bridge_MobileCoreBridge_nativeFree<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
) {
    env.with_env(|_env| -> Result<(), JniError> {
        // Safety: contract described above -- carried from the Kotlin
        // caller, which owns the handle's lifecycle.
        unsafe { mobile_core::mobile_core_free(handle as *mut MobileApp) };
        Ok(())
    })
    .resolve::<LogErrorAndDefault>()
}

/// `Java_com_onyx_bridge_MobileCoreBridge_nativeSetHierarchy` --
/// `com.onyx.bridge.MobileCoreBridge.nativeSetHierarchy(handle: Long, hierarchyJson: String): Int`.
///
/// Added for A3 (startup/auth): populates the local approval-authority
/// cache after a real login, mirroring Dart's `OnyxApi.setHierarchy`
/// call in `ffi_login_screen.dart`/`main.dart::refreshHierarchyBestEffort`.
/// Returns `mobile_core_set_hierarchy`'s own result unchanged (`0`
/// success, `-1` invalid arguments or unparseable `hierarchyJson`) --
/// `-1` is also this wrapper's own failure value for a JNI-level string
/// conversion failure, matching `mobile_core_set_hierarchy`'s existing
/// "invalid arguments" case rather than inventing a third status code.
#[no_mangle]
pub extern "system" fn Java_com_onyx_bridge_MobileCoreBridge_nativeSetHierarchy<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
    hierarchy_json: JString<'local>,
) -> i32 {
    env.with_env(|env| -> Result<i32, JniError> {
        let Some(hierarchy_json) = jstring_to_cstring(env, &hierarchy_json) else {
            return Ok(-1);
        };
        // Safety: `handle` is the Kotlin caller's responsibility (must
        // be a live value from nativeNew); hierarchy_json is a freshly
        // built, valid C string.
        let result = unsafe {
            mobile_core::mobile_core_set_hierarchy(
                handle as *mut MobileApp,
                hierarchy_json.as_ptr(),
            )
        };
        Ok(result)
    })
    .resolve::<LogErrorAndDefault>()
}

/// `Java_com_onyx_bridge_MobileCoreBridge_nativeExecuteCommand` --
/// `com.onyx.bridge.MobileCoreBridge.nativeExecuteCommand(handle: Long, commandJson: String): String?`.
///
/// The representative string-round-trip wrapper (see this module's own
/// doc comment for why this, not just handle lifecycle, is the real
/// proof this architecture generalizes). Returns `null` for exactly the
/// cases `mobile_core_execute_command` itself returns a null pointer
/// for (malformed FFI call, not a domain-level command rejection --
/// see that function's own doc comment); a domain rejection still comes
/// back as a real JSON string (`{"success": false, "error": ...}`), not
/// `null`, preserving `mobile-core`'s existing error-surfacing contract
/// exactly rather than collapsing both cases into one Kotlin-side
/// `null`.
#[no_mangle]
pub extern "system" fn Java_com_onyx_bridge_MobileCoreBridge_nativeExecuteCommand<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
    command_json: JString<'local>,
) -> jstring {
    env.with_env(|env| -> Result<jstring, JniError> {
        let Some(command_json) = jstring_to_cstring(env, &command_json) else {
            return Ok(std::ptr::null_mut());
        };

        // Safety: `handle` is the Kotlin caller's responsibility (must
        // be a live value from nativeNew, per this function's own doc
        // comment); command_json is a freshly built, valid C string.
        let result_ptr = unsafe {
            mobile_core::mobile_core_execute_command(
                handle as *mut MobileApp,
                command_json.as_ptr(),
            )
        };
        copy_and_free_c_string(env, result_ptr)
    })
    .resolve::<LogErrorAndDefault>()
}

/// `Java_com_onyx_bridge_MobileCoreBridge_nativeListAggregates` --
/// `com.onyx.bridge.MobileCoreBridge.nativeListAggregates(handle: Long, aggregateType: String): String?`.
///
/// Added for A4 (core screens): every screen's list data
/// (missions/tasks/notifications) comes from this one function, per
/// the shared-refresh architecture `OnyxController` (A4's own Kotlin
/// port of `ui/app.dart`'s `OnyxController`) fans out on `refresh()`.
#[no_mangle]
pub extern "system" fn Java_com_onyx_bridge_MobileCoreBridge_nativeListAggregates<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
    aggregate_type: JString<'local>,
) -> jstring {
    env.with_env(|env| -> Result<jstring, JniError> {
        let Some(aggregate_type) = jstring_to_cstring(env, &aggregate_type) else {
            return Ok(std::ptr::null_mut());
        };
        // Safety: same contract as nativeExecuteCommand above.
        let result_ptr = unsafe {
            mobile_core::mobile_core_list_aggregates(
                handle as *mut MobileApp,
                aggregate_type.as_ptr(),
            )
        };
        copy_and_free_c_string(env, result_ptr)
    })
    .resolve::<LogErrorAndDefault>()
}

/// `Java_com_onyx_bridge_MobileCoreBridge_nativeGetSyncStatus` --
/// `com.onyx.bridge.MobileCoreBridge.nativeGetSyncStatus(handle: Long): String?`.
/// Added for A4 -- one of the shared-refresh cycle's six calls
/// (Dashboard reads `pendingOutboxCount` from this).
#[no_mangle]
pub extern "system" fn Java_com_onyx_bridge_MobileCoreBridge_nativeGetSyncStatus<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> jstring {
    env.with_env(|env| -> Result<jstring, JniError> {
        // Safety: same contract as nativeExecuteCommand above.
        let result_ptr =
            unsafe { mobile_core::mobile_core_get_sync_status(handle as *mut MobileApp) };
        copy_and_free_c_string(env, result_ptr)
    })
    .resolve::<LogErrorAndDefault>()
}

/// `Java_com_onyx_bridge_MobileCoreBridge_nativeListConflicts` --
/// `com.onyx.bridge.MobileCoreBridge.nativeListConflicts(handle: Long): String?`.
/// Added for A4 -- one of the shared-refresh cycle's six calls
/// (Dashboard reads the conflict count from this).
#[no_mangle]
pub extern "system" fn Java_com_onyx_bridge_MobileCoreBridge_nativeListConflicts<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> jstring {
    env.with_env(|env| -> Result<jstring, JniError> {
        // Safety: same contract as nativeExecuteCommand above.
        let result_ptr =
            unsafe { mobile_core::mobile_core_list_conflicts(handle as *mut MobileApp) };
        copy_and_free_c_string(env, result_ptr)
    })
    .resolve::<LogErrorAndDefault>()
}

/// Shared tail end of every `*mut c_char`-returning wrapper above: copy
/// the C string into a JVM-owned string *before* freeing it via
/// `mobile_core_free_string` (`new_string` allocates its own copy, so
/// ownership of `result_ptr` never crosses into Kotlin), returning
/// `null` unchanged for a null `result_ptr` -- every wrapped function's
/// own "malformed FFI call, not a domain rejection" convention (see
/// `nativeExecuteCommand`'s doc comment) is preserved by construction,
/// not re-implemented per call site.
fn copy_and_free_c_string(
    env: &mut jni::Env<'_>,
    result_ptr: *mut c_char,
) -> Result<jstring, JniError> {
    if result_ptr.is_null() {
        return Ok(std::ptr::null_mut());
    }
    // Safety: result_ptr is non-null and was just returned by one of
    // this crate's wrapped mobile-core functions, each of which
    // documents it as a valid NUL-terminated string to be freed via
    // mobile_core_free_string exactly once -- done immediately after
    // this copy.
    let result_str = unsafe { CStr::from_ptr(result_ptr).to_string_lossy().into_owned() };
    unsafe { mobile_core::mobile_core_free_string(result_ptr) };
    match env.new_string(result_str) {
        Ok(s) => Ok(s.into_raw()),
        Err(e) => Err(e),
    }
}

/// Converts a JVM `String` to an owned, NUL-terminated `CString` for
/// `mobile-core`'s C ABI. Returns `None` on either a JNI-level failure
/// (mirrors `mobile_core_new`'s own "invalid arguments" null-return
/// convention) or a string that itself contains an interior NUL byte
/// (which cannot round-trip through a C string at all -- rejecting it
/// here, rather than truncating silently, matches this project's
/// general preference for a visible failure over silent data loss).
fn jstring_to_cstring(env: &jni::Env<'_>, value: &JString) -> Option<CString> {
    // `try_to_string` is `JString`'s real, current 0.22.4 accessor
    // (confirmed by reading jni-0.22.4's own source directly, not
    // assumed) -- the crate's `get_string`/`get_string_unchecked` are
    // now deprecated in favor of it.
    let s = value.try_to_string(env).ok()?;
    CString::new(s).ok()
}
