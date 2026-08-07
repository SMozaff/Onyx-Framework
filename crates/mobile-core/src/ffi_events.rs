//! `mobile_core_subscribe_events`/`mobile_core_unsubscribe`/
//! `mobile_core_trigger_sync` — split out of the original flat `lib.rs`.
//! See `ffi_commands.rs`'s doc comment for the split's provenance.
//! `trigger_sync` is grouped here rather than in its own file (the
//! requested file list has no separate "sync" FFI file) since it is
//! small and is the manual-trigger counterpart to the same `SyncAgent`
//! this module's event subscription reads from.

use std::ffi::{c_char, c_int, CString};
use std::sync::Arc;

use client_composition::event_bus::EventFilter;

use crate::{cstr_to_string, EventSubscription, MobileApp};

/// Triggers a sync session manually. Returns 0 on success, -1 on error.
/// Team Prompt 5 §3.3. Uses `SyncAgent::trigger_sync_now` — added to
/// `client-composition` specifically because this FFI function needed it
/// and no prior increment/ruling had defined a manual-trigger entry
/// point (`SyncAgent`'s only previous entry point was its own internal
/// periodic loop).
///
/// # Safety
/// `handle` must be a valid pointer from `mobile_core_new`.
#[no_mangle]
pub unsafe extern "C" fn mobile_core_trigger_sync(handle: *mut MobileApp) -> c_int {
    if handle.is_null() {
        return -1;
    }
    let app = &*handle;
    match app
        .runtime
        .block_on(async { app.state.sync_agent.trigger_sync_now().await })
    {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Subscribes to events. Returns a subscription handle; the callback is
/// invoked for each matching event (JSON string). Team Prompt 5 §3.3.
///
/// # Safety
/// `handle` must be valid. `filter_json` must be a valid NUL-terminated
/// C string. `callback` must be a valid function pointer that remains
/// valid for as long as the returned subscription is alive (i.e. until
/// `mobile_core_unsubscribe` is called) — this is inherently unsafe
/// FFI-callback lifetime management the C/Swift/Kotlin caller is
/// responsible for upholding; Rust cannot enforce it.
#[no_mangle]
pub unsafe extern "C" fn mobile_core_subscribe_events(
    handle: *mut MobileApp,
    filter_json: *const c_char,
    callback: extern "C" fn(*const c_char),
) -> *mut EventSubscription {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    let app = &*handle;
    let Some(filter_json) = cstr_to_string(filter_json) else {
        return std::ptr::null_mut();
    };
    let Ok(filter) = serde_json::from_str::<EventFilter>(&filter_json) else {
        return std::ptr::null_mut();
    };

    let event_bus = Arc::clone(&app.state.event_bus);
    // SAFETY (documented, not silently assumed): `callback` is a raw
    // function pointer captured into the spawned task's closure.
    // Function pointers are `Send`/`Copy`, so this compiles without an
    // explicit unsafe block here — the actual safety obligation (the
    // pointer staying valid for the subscription's lifetime) is on the
    // caller, per this function's own `# Safety` doc comment above.
    let task = app.runtime.spawn(async move {
        let mut subscription = event_bus.subscribe(filter);
        while let Some(event) = subscription.recv().await {
            let json = event.to_string();
            if let Ok(cstring) = CString::new(json) {
                // Dart's cross-thread NativeCallable.listener schedules the
                // callback asynchronously. Transfer ownership so the buffer
                // remains valid until the Dart callback reads it and calls
                // `mobile_core_free_string`.
                callback(cstring.into_raw());
            }
        }
    });

    Box::into_raw(Box::new(EventSubscription { task }))
}

/// Unsubscribes and frees the subscription. Team Prompt 5 §3.3.
///
/// # Safety
/// `sub` must be a pointer previously returned by
/// `mobile_core_subscribe_events`, not yet freed.
#[no_mangle]
pub unsafe extern "C" fn mobile_core_unsubscribe(sub: *mut EventSubscription) {
    if !sub.is_null() {
        let boxed = Box::from_raw(sub);
        boxed.task.abort();
    }
}
