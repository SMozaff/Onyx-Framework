//! `mobile_core_android_do_work` — split out of the original flat
//! `lib.rs`. See `ffi_commands.rs`'s doc comment for the split's
//! provenance.

use std::ffi::{c_int, c_void};

use crate::MobileApp;

/// Background sync entry point (Android). Called from `WorkManager`.
/// Returns 1 if work was done, 0 if idle. Team Prompt 5 §3.3/§4.3.
///
/// # Safety
/// Same handle-parameter deviation as `mobile_core_ios_background_sync`
/// (`ios_background.rs`) — the frozen contract's literal signature
/// (`mobile_core_android_do_work(void* env, void* thiz)`, JNI
/// convention) also has no way to reach a specific `MobileApp` instance.
/// `env`/`thiz` are accepted and ignored (real JNI plumbing — resolving
/// `thiz` to a stored native handle field — is Android-toolchain-specific
/// code this sandbox cannot build or verify, per the same "no NDK"
/// constraint as the rest of this crate's mobile-specific pieces).
#[no_mangle]
pub unsafe extern "C" fn mobile_core_android_do_work(
    handle: *mut MobileApp,
    _env: *mut c_void,
    _thiz: *mut c_void,
) -> c_int {
    if handle.is_null() {
        return 0;
    }
    let app = &*handle;
    match app
        .runtime
        .block_on(async { app.state.sync_agent.trigger_sync_now().await })
    {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

/// JNI wrapper used by `com.onyx.WorkManagerService`. It delegates to the
/// process-registered mobile core handle so WorkManager does not need to
/// marshal a raw Rust pointer through Kotlin.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_onyx_WorkManagerService_nativeAndroidDoWork(
    _env: *mut c_void,
    _class: *mut c_void,
) -> c_int {
    crate::mobile_core_background_sync_registered(0)
}
