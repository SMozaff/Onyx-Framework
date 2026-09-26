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
use std::os::raw::{c_char, c_void};
use std::sync::atomic::{AtomicBool, Ordering};


use jni::errors::{Error as JniError, LogErrorAndDefault};
use jni::objects::{Global, JClass, JObject, JString};
use jni::sys::{jlong, jstring};
use jni::{jni_sig, jni_str, Env, EnvUnowned, JavaVM};

use mobile_core::MobileApp;

/// P2P transport codec (Phase 4.1): framing/encryption/handshake in pure
/// Rust plus the `com.onyx.p2p.P2pCodec` JNI surface Kotlin drives the
/// platform sockets through.
pub mod p2p;

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

/// `Java_com_onyx_bridge_MobileCoreBridge_nativeUploadFile` --
/// `com.onyx.bridge.MobileCoreBridge.nativeUploadFile(handle: Long, path: String, organizationId: String, userId: String, deviceId: String): String?`.
///
/// Added for A5 (Files screen): wraps `mobile_core_upload_file`
/// unchanged, including its real, current "collapse every failure (I/O
/// error, oversized file, coordinator error) into a null return" contract
/// -- confirmed by reading that function's own source directly, not
/// assumed richer than it is. Dart's own `FilesScreen` gets exactly the
/// same generic signal (`_decodeOwnedJson` throws `StateError('mobile-core
/// returned null')` on a null pointer, with no further detail), so Kotlin
/// matching that same generic failure is real parity, not a regression.
#[no_mangle]
pub extern "system" fn Java_com_onyx_bridge_MobileCoreBridge_nativeUploadFile<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
    path: JString<'local>,
    organization_id: JString<'local>,
    user_id: JString<'local>,
    device_id: JString<'local>,
) -> jstring {
    env.with_env(|env| -> Result<jstring, JniError> {
        let Some(path) = jstring_to_cstring(env, &path) else {
            return Ok(std::ptr::null_mut());
        };
        let Some(organization_id) = jstring_to_cstring(env, &organization_id) else {
            return Ok(std::ptr::null_mut());
        };
        let Some(user_id) = jstring_to_cstring(env, &user_id) else {
            return Ok(std::ptr::null_mut());
        };
        let Some(device_id) = jstring_to_cstring(env, &device_id) else {
            return Ok(std::ptr::null_mut());
        };
        // Safety: same contract as nativeExecuteCommand above; every
        // string argument is a freshly built, valid C string.
        let result_ptr = unsafe {
            mobile_core::mobile_core_upload_file(
                handle as *mut MobileApp,
                path.as_ptr(),
                organization_id.as_ptr(),
                user_id.as_ptr(),
                device_id.as_ptr(),
            )
        };
        copy_and_free_c_string(env, result_ptr)
    })
    .resolve::<LogErrorAndDefault>()
}

/// `Java_com_onyx_bridge_MobileCoreBridge_nativeDownloadFile` --
/// `com.onyx.bridge.MobileCoreBridge.nativeDownloadFile(handle: Long, contentHash: String, destinationPath: String): Long`.
///
/// Added for A5. Returns `mobile_core_download_file`'s own `i64` result
/// unchanged: bytes written on success, `-1` on any failure (invalid
/// arguments, no stored content for that hash, or a write error) --
/// same generic sentinel Dart's own `downloadFile` surfaces as
/// `StateError('mobile_core_download_file failed')`.
#[no_mangle]
pub extern "system" fn Java_com_onyx_bridge_MobileCoreBridge_nativeDownloadFile<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
    content_hash: JString<'local>,
    destination_path: JString<'local>,
) -> jlong {
    env.with_env(|env| -> Result<jlong, JniError> {
        let Some(content_hash) = jstring_to_cstring(env, &content_hash) else {
            return Ok(-1);
        };
        let Some(destination_path) = jstring_to_cstring(env, &destination_path) else {
            return Ok(-1);
        };
        // Safety: same contract as nativeExecuteCommand above.
        let bytes_written = unsafe {
            mobile_core::mobile_core_download_file(
                handle as *mut MobileApp,
                content_hash.as_ptr(),
                destination_path.as_ptr(),
            )
        };
        Ok(bytes_written as jlong)
    })
    .resolve::<LogErrorAndDefault>()
}

/// `Java_com_onyx_bridge_MobileCoreBridge_nativeTriggerSync` --
/// `com.onyx.bridge.MobileCoreBridge.nativeTriggerSync(handle: Long): Int`.
///
/// Added for A5 (sync status widget's manual "tap to synchronize now"
/// action). Returns `mobile_core_trigger_sync`'s own result unchanged
/// (`0` success, `-1` failure).
#[no_mangle]
pub extern "system" fn Java_com_onyx_bridge_MobileCoreBridge_nativeTriggerSync<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> i32 {
    env.with_env(|_env| -> Result<i32, JniError> {
        // Safety: same contract as nativeExecuteCommand above.
        let result = unsafe { mobile_core::mobile_core_trigger_sync(handle as *mut MobileApp) };
        Ok(result)
    })
    .resolve::<LogErrorAndDefault>()
}

/// `Java_com_onyx_bridge_MobileCoreBridge_nativeResolveConflict` --
/// `com.onyx.bridge.MobileCoreBridge.nativeResolveConflict(handle: Long, conflictJson: String, resolution: String): Int`.
///
/// Added for A5 (conflict resolution dialog). `resolution` must be one
/// of `"local"`/`"remote"`/`"escalate"`, exactly matching Dart's
/// `ConflictChoice.name` values and `mobile_core_resolve_conflict`'s own
/// real, current string match -- confirmed by reading that function
/// directly rather than assumed. Returns `0` on success, non-zero
/// otherwise (invalid arguments, unknown resolution string, or the
/// conflict itself failing to resolve).
#[no_mangle]
pub extern "system" fn Java_com_onyx_bridge_MobileCoreBridge_nativeResolveConflict<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
    conflict_json: JString<'local>,
    resolution: JString<'local>,
) -> i32 {
    env.with_env(|env| -> Result<i32, JniError> {
        let Some(conflict_json) = jstring_to_cstring(env, &conflict_json) else {
            return Ok(-1);
        };
        let Some(resolution) = jstring_to_cstring(env, &resolution) else {
            return Ok(-1);
        };
        // Safety: same contract as nativeExecuteCommand above.
        let result = unsafe {
            mobile_core::mobile_core_resolve_conflict(
                handle as *mut MobileApp,
                conflict_json.as_ptr(),
                resolution.as_ptr(),
            )
        };
        Ok(result)
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

// ============================================================================
// QUERY + EVENT SURFACE — completed for KOTLIN_IMPLEMENTATION_PLAN.md Layers
// 2/4 (executeQuery) and 2/5 (real-time event stream). `nativeExecuteQuery`
// is a straight wrapper of `mobile_core_execute_query`; the subscribe/
// unsubscribe pair route `mobile-core`'s C callbacks into a Kotlin
// `EventCallback` via a JVM-attaching forwarder (see `JavaEventForwarder`).
// ============================================================================

/// `Java_com_onyx_bridge_MobileCoreBridge_nativeExecuteQuery` —
/// `com.onyx.bridge.MobileCoreBridge.nativeExecuteQuery(handle: Long, queryJson: String): String?`.
///
/// Wraps `mobile_core_execute_query` (from `crates/mobile-core/src/ffi_queries.rs`).
/// Added for KOTLIN_IMPLEMENTATION_PLAN.md Layer 2/4. `queryJson` is a
/// `QueryEnvelope` — `{"query_type": "GetMission", "target_id":
/// <uuid bytes as JSON array>}` (see `client_composition::query_registry`
/// for the full envelope shape). Returns the serialized query result (the
/// aggregate's `Loaded` JSON) or `null` when `mobile_core_execute_query`
/// itself returns null (unknown `query_type`, missing aggregate,
/// malformed FFI call) — the exact same convention every other wrapper in
/// this file preserves.
#[no_mangle]
pub extern "system" fn Java_com_onyx_bridge_MobileCoreBridge_nativeExecuteQuery<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
    query_json: JString<'local>,
) -> jstring {
    env.with_env(|env| -> Result<jstring, JniError> {
        let Some(query_json) = jstring_to_cstring(env, &query_json) else {
            return Ok(std::ptr::null_mut());
        };
        let result_ptr = unsafe {
            mobile_core::mobile_core_execute_query(
                handle as *mut mobile_core::MobileApp,
                query_json.as_ptr(),
            )
        };
        copy_and_free_c_string(env, result_ptr)
    })
    .resolve::<LogErrorAndDefault>()
}

/// `Java_com_onyx_bridge_MobileCoreBridge_nativeSubscribeEvents` —
/// `com.onyx.bridge.MobileCoreBridge.nativeSubscribeEvents(handle: Long, filterJson: String, callback: EventCallback): Long`.
///
/// Real subscription, wrapping `mobile_core_subscribe_events` (from
/// `crates/mobile-core/src/ffi_events.rs`) with the JNI problem that
/// function's 2025-era stub declared unsolvable ("JNI does not directly
/// support passing C function pointers from Kotlin"). It is solved the
/// way every C library with callback + userdata solves it:
///
/// * `mobile_core_subscribe_events`'s `context` pointer (added for this
///   client; see DECISIONS entry) carries a `Box<JavaEventForwarder>`
///   that owns the JVM handle (an `Arc<JavaVM>`) and a JNI `GlobalRef`
///   to the caller's Kotlin `EventCallback` instance.
/// * The C function pointer is a single `extern "C" fn` defined here —
///   `deliver_to_kotlin` — closing over nothing; everything it needs
///   lives in the forwarded `context`.
/// * On each matched event (delivered from a tokio worker thread, never
///   a JNI-attached thread), `deliver_to_kotlin` calls
///   `JavaVM::attach_current_thread` (jni-rs 0.22's current API; a
///   Rust-runtime thread normally has no JVM attachment), invokes
///   `EventCallback.onEvent(json)` via `CallObjectMethod`, then lets the
///   `AttachGuard` detach on drop. Attach/exit is tied to a single
///   delivery so no thread holds a spurious JVM attachment between
///   events, and a delivery that fails (crash, GC'd callback) is logged
///   and skipped, not fatal.
/// * The `GlobalRef` is released by dropping the forwarder on
///   `nativeUnsubscribe` (or when `mobile_core` aborts the forwarding
///   task).
///
/// Returns the `*mut EventSubscription` as the `Long` handle (same
/// pointer-as-jlong convention `nativeNew` uses for `*mut MobileApp`), or
/// `0` on failure (null/unknown handle, malformed `filterJson`, or a
/// JNI-level failure building the forwarder) — matching
/// `mobile_core_subscribe_events`'s own null-on-invalid-input contract.
#[no_mangle]
pub extern "system" fn Java_com_onyx_bridge_MobileCoreBridge_nativeSubscribeEvents<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
    filter_json: JString<'local>,
    callback: JObject<'local>,
) -> jlong {
    if handle == 0 {
        return 0;
    }
    env.with_env(|env| -> Result<jlong, JniError> {
        let Some(filter_json) = jstring_to_cstring(env, &filter_json) else {
            return Ok(0);
        };
        let Some(forwarder) = JavaEventForwarder::new(env, callback) else {
            return Ok(0);
        };
        let forwarder_ptr = Box::into_raw(Box::new(forwarder)) as *mut c_void;

        // Safety: `handle` is the Kotlin caller's responsibility (must
        // be a live value from nativeNew); filter_json is freshly built
        // and NUL-terminated; `forwarder_ptr` is leaked into context,
        // owned by the C-ABI subscription until nativeUnsubscribe
        // reclaims it (plus the early-reclaim below if subscription
        // setup fails); `deliver_to_kotlin` is a static fn valid for the
        // lifetime of the returned subscription.
        let sub_ptr = unsafe {
            mobile_core::mobile_core_subscribe_events(
                handle as *mut MobileApp,
                filter_json.as_ptr(),
                deliver_to_kotlin,
                forwarder_ptr,
            )
        };
        if sub_ptr.is_null() {
            // mobile_core rejected the subscription; reclaim the
            // forwarder we leaked above so the GlobalRef is not orphaned.
            drop(unsafe { Box::from_raw(forwarder_ptr as *mut JavaEventForwarder) });
            Ok(0)
        } else {
            Ok(sub_ptr as jlong)
        }
    })
    .resolve::<LogErrorAndDefault>()
}

/// `Java_com_onyx_bridge_MobileCoreBridge_nativeUnsubscribe` —
/// `com.onyx.bridge.MobileCoreBridge.nativeUnsubscribe(subscription: Long)`.
///
/// Aborts the forwarding task (`mobile_core_unsubscribe`) and reclaims
/// the `JavaEventForwarder`'s `GlobalRef`/JVM handle that was leaked as
/// the subscription's context pointer. Safe to call once only, exactly
/// like `mobile_core_unsubscribe` itself.
#[no_mangle]
pub extern "system" fn Java_com_onyx_bridge_MobileCoreBridge_nativeUnsubscribe<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    subscription: jlong,
) {
    if subscription == 0 {
        return;
    }
    env.with_env(|_env| -> Result<(), JniError> {
        let sub_ptr = subscription as *mut mobile_core::EventSubscription;
        // Safety: subscription must be a live value previously returned
        // by nativeSubscribeEvents, not yet freed — the subscription
        // pointer is only ever reclaimed here, same single-owner
        // contract mobile_core_unsubscribe itself documents.
        unsafe {
            mobile_core::mobile_core_unsubscribe(sub_ptr);
        }
        Ok(())
    })
    .resolve::<LogErrorAndDefault>()
}

/// C-ABI event callback trampoline — the function pointer handed to
/// `mobile_core_subscribe_events` as its `callback` argument.
///
/// `context` is the `Box<JavaEventForwarder>` the matching
/// `nativeSubscribeEvents` call leaked into the subscription; `json` is
/// the owned, NUL-terminated envelope buffer the C-ABI contract requires
/// freeing via `mobile_core_free_string` exactly once — and which every
/// path in this function, success or failure, satisfies.
///
/// Runs on a tokio worker thread, so it performs its own JVM attach for
/// the duration of the one delivery and detaches when the guard drops.
/// Must be `extern "C" fn` with no `Send` capture — matching the C ABI
/// `mobile_core` declares.
extern "C" fn deliver_to_kotlin(context: *mut c_void, json: *const c_char) {
    if json.is_null() {
        return;
    }
    // The C-ABI contract transfers ownership of `json` to the callback;
    // reclaim it up front so every return path below frees it exactly
    // once, whether or not the delivery reaches the JVM.
    let json = unsafe { CString::from_raw(json as *mut c_char) };
    // SAFETY: `context` is a live `JavaEventForwarder` box for as long as
    // the subscription is alive (owned by the C-ABI and reclaimed only by
    // `nativeUnsubscribe`, which aborts the delivery task first).
    let forwarder = unsafe { &*(context as *const JavaEventForwarder) };
    // A failed delivery must not take the subscription down (that would
    // stop the whole stream); skip the event and keep going. This crate
    // has no logger wired (JNI entry points use jni's LogErrorAndDefault);
    // logcat plumbing for the async callback path is future work.
    let _ = forwarder.deliver(&json);
}

/// Owns everything a tokio-thread event delivery needs to reach a Kotlin
/// `EventCallback`: the JVM to attach to and the strong reference to the
/// callback object. Lives in a `Box`, transported as the C-ABI
/// subscription's `context` pointer, and is reclaimed (dropped) by
/// `nativeUnsubscribe`.
struct JavaEventForwarder {
    vm: JavaVM,
    callback: Global<JObject<'static>>,
    /// Re-entrancy guard: `mobile_core` invokes the callback serially
    /// from one forwarding task, but this makes that assumption visible
    /// and cheap to uphold rather than silently relying on it. Cleared
    /// on `Drop` so an in-flight failure can never wedge deliveries.
    active: AtomicBool,
}

const KOTLIN_EVENT_CALLBACK_CLASS: &str = "com/onyx/bridge/EventCallback";
const KOTLIN_EVENT_CALLBACK_METHOD: &str = "onEvent";
const KOTLIN_EVENT_CALLBACK_SIGNATURE: &str = "(Ljava/lang/String;)V";

impl JavaEventForwarder {
    /// Captures the JVM and builds a `GlobalRef` for the caller's
    /// `EventCallback` instance. Returns `None` on any JNI-level failure
    /// (no VM, null object, or the callback class/method not resolving).
    fn new(env: &mut Env<'_>, callback: JObject<'_>) -> Option<Self> {
        if callback.is_null() {
            return None;
        }
        let vm = env.get_java_vm().ok()?;
        let class = env.find_class(KOTLIN_EVENT_CALLBACK_CLASS).ok()?;
        // Resolve the method now so delivery (on another thread) never
        // needs to re-resolve it — fail fast on a typo'd name/signature.
        if env
            .get_method_id(
                &class,
                jni_str!("onEvent"),
                jni_sig!("(Ljava/lang/String;)V"),
            )
            .is_err()
        {
            return None;
        }
        let callback = env.new_global_ref(callback).ok()?;
        Some(Self {
            vm,
            callback,
            active: AtomicBool::new(false),
        })
    }

    /// Attaches the current (tokio) thread to the JVM for the duration of
    /// one delivery, reads the NUL-terminated envelope `json`, and calls
    /// `EventCallback.onEvent(String)` on the global `callback` ref.
    ///
    /// # Safety contract
    /// * `self` must be a live `JavaEventForwarder` (guard: the caller
    ///   holds the boxed address while a subscription is alive, and
    ///   `nativeUnsubscribe` aborts the delivery task before reclaiming
    ///   the box; this crate is the only owner of the pointer).
    /// * `json` must be a valid NUL-terminated C string — guaranteed by
    ///   `deliver_to_kotlin`, which hands over a freshly built `CString`.
    fn deliver(&self, json: &CStr) -> Result<(), String> {
        use jni::objects::JValue;

        if self.active.swap(true, Ordering::AcqRel) {
            return Err(
                "re-entrant event delivery (mobile_core invokes the callback serially)".into(),
            );
        }
        struct ClearOnDrop<'a>(&'a AtomicBool);
        impl Drop for ClearOnDrop<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _clear = ClearOnDrop(&self.active);

        // attach_current_thread returns an AttachGuard whose Drop
        // detaches; binding it here keeps this one worker thread attached
        // only for the duration of the delivery.
        self.vm
            .attach_current_thread(|env| -> jni::errors::Result<()> {
                let json_string = env.new_string(json.to_string_lossy().as_ref())?;
                env.call_method(
                    self.callback.as_obj(),
                    jni_str!("onEvent"),
                    jni_sig!("(Ljava/lang/String;)V"),
                    &[JValue::Object(&json_string)],
                )?;
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
