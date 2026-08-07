//! `mobile_core_ios_background_sync` — split out of the original flat
//! `lib.rs`. See `ffi_commands.rs`'s doc comment for the split's
//! provenance.

use std::ffi::c_int;

use crate::MobileApp;

/// Background sync entry point (iOS). Called from `BGAppRefreshTask`.
/// Returns 1 if work was done, 0 if idle. Team Prompt 5 §3.3/§4.3.
///
/// # Session resumption (flagged, not silently assumed complete)
/// Team Prompt 5 §4.3 requires persisting `SyncCursor` across app
/// suspension and resuming a *new* `SynchronizationSession` from it on
/// the next background run. `SynchronizationSession` has no
/// cursor-argument constructor or `resume(cursor)` — `pause()`/`resume()`
/// operate on the same session instance's internal state (see R2/W1,
/// `DECISIONS.md`), which cannot survive process death. This function
/// therefore runs a fresh, full sync cycle (`trigger_sync_now`) rather
/// than genuine cursor-based resumption — real §4.3 compliance is
/// blocked on `SynchronizationSession` gaining a real resumption API, a
/// change to Increment 3's delivered type this crate has no mandate to
/// make unilaterally.
///
/// # Safety
/// `handle`, not `task_id`, is the actual pointer parameter here — Team
/// Prompt 5 §3.3's C signature is `mobile_core_ios_background_sync(uint64_t
/// task_id)` with **no handle parameter at all**, which cannot work: the
/// function has no way to reach a `MobileApp` instance without one. This
/// is a genuine gap in the frozen contract's own C header, not a
/// transcription error — flagged rather than silently worked around by
/// inventing global mutable state (a `static` handle) to paper over it,
/// since that would itself be a significant, silent architectural choice
/// (global state, single-instance assumption) the frozen contract never
/// asked for either. Implemented here taking an explicit handle,
/// deviating from the literal C signature; the real Swift caller
/// (`BackgroundService.swift` in Team Prompt 5 §3.5) would need to be
/// updated to pass one.
#[no_mangle]
pub unsafe extern "C" fn mobile_core_ios_background_sync(
    handle: *mut MobileApp,
    _task_id: u64,
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
