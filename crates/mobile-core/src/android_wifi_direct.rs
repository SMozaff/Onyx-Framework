//! Android WifiP2pManager integration — thin Rust-level wrappers around
//! `sync-transport-mobile`'s real, already-delivered `extern "C"`
//! exports (`android_wifi_direct_start_discovery`,
//! `android_wifi_direct_stop_discovery`, `android_wifi_direct_connect`,
//! `android_wifi_direct_send`, in
//! `sync_transport_mobile::android_wifi_direct`). See
//! `ios_multipeer.rs`'s module doc comment (in this same crate) for why
//! this file re-exports Team 4's real functions rather than redeclaring
//! new `#[no_mangle]` symbols (the same symbol-collision reasoning
//! applies identically here), and for the `target_os = "android"`
//! gating this file also carries (discovered by reading
//! `sync-transport-mobile`'s own `lib.rs`, which gates
//! `android_wifi_direct` behind exactly that cfg).

#![cfg(target_os = "android")]

pub use sync_transport_mobile::android_wifi_direct::{
    android_wifi_direct_connect, android_wifi_direct_send, android_wifi_direct_start_discovery,
    android_wifi_direct_stop_discovery,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Same purpose as `ios_multipeer.rs`'s test: confirms this crate's
    /// re-export wiring works, not a re-test of Team 4's own logic.
    /// Android-only, unverifiable in this sandbox (no Android NDK) — see
    /// this file's module doc comment.
    #[test]
    fn wifi_direct_discovery_round_trip_through_mobile_core_reexport() {
        let handle = unsafe { android_wifi_direct_start_discovery() };
        assert!(!handle.is_null());
        unsafe { android_wifi_direct_stop_discovery(handle) };
    }
}
