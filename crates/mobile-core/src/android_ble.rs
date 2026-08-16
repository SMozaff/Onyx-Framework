//! Android BluetoothLe integration — thin Rust-level wrappers around
//! `sync-transport-mobile`'s real, already-delivered `extern "C"`
//! exports (`android_ble_start_advertising`,
//! `android_ble_stop_advertising`, `android_ble_connect`,
//! `android_ble_send`, in `sync_transport_mobile::android_ble`). See
//! `ios_multipeer.rs`'s module doc comment (in this same crate) for why
//! this file re-exports Team 4's real functions rather than redeclaring
//! new `#[no_mangle]` symbols (the same symbol-collision reasoning
//! applies identically here), and for the `target_os = "android"`
//! gating this file also carries (discovered by reading
//! `sync-transport-mobile`'s own `lib.rs`, which gates `android_ble`
//! behind exactly that cfg).

#![cfg(target_os = "android")]

pub use sync_transport_mobile::android_ble::{
    android_ble_connect, android_ble_send, android_ble_start_advertising,
    android_ble_stop_advertising,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    /// Same purpose as `ios_multipeer.rs`'s test: confirms this crate's
    /// re-export wiring works, not a re-test of Team 4's own logic.
    /// Android-only, unverifiable in this sandbox (no Android NDK) — see
    /// this file's module doc comment.
    #[test]
    fn ble_advertising_round_trip_through_mobile_core_reexport() {
        let service_uuid = CString::new("0000FFF0-0000-1000-8000-00805F9B34FB").unwrap();
        let handle = unsafe { android_ble_start_advertising(service_uuid.as_ptr()) };
        assert!(!handle.is_null());
        unsafe { android_ble_stop_advertising(handle) };
    }
}
