//! iOS CoreBluetooth integration — thin Rust-level wrappers around
//! `sync-transport-mobile`'s real, already-delivered `extern "C"`
//! exports (`ios_ble_start_advertising`, `ios_ble_stop_advertising`,
//! `ios_ble_connect`, `ios_ble_send`, in
//! `sync_transport_mobile::ios_ble`). See `ios_multipeer.rs`'s module
//! doc comment for why this file re-exports Team 4's real functions
//! rather than redeclaring new `#[no_mangle]` symbols (the same
//! symbol-collision reasoning applies identically here), and for the
//! `target_os = "ios"` gating this file also carries (discovered by
//! reading `sync-transport-mobile`'s own `lib.rs`, which gates
//! `ios_ble` behind exactly that cfg).

#![cfg(target_os = "ios")]

pub use sync_transport_mobile::ios_ble::{
    ios_ble_connect, ios_ble_send, ios_ble_start_advertising, ios_ble_stop_advertising,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    /// Same purpose as `ios_multipeer.rs`'s test: confirms this crate's
    /// re-export wiring works, not a re-test of Team 4's own logic
    /// (which has its own suite in `sync-transport-mobile`). iOS-only,
    /// unverifiable in this sandbox — see this file's module doc
    /// comment.
    #[test]
    fn ble_advertising_round_trip_through_mobile_core_reexport() {
        let service_uuid = CString::new("0000FFF0-0000-1000-8000-00805F9B34FB").unwrap();
        let handle = unsafe { ios_ble_start_advertising(service_uuid.as_ptr()) };
        assert!(!handle.is_null());
        unsafe { ios_ble_stop_advertising(handle) };
    }
}
