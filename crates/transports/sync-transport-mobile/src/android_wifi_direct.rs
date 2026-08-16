//! Android Wi-Fi Direct bridge. Frozen contract per Team Prompt 4 §6.2.
//! `WifiP2pManager` calls are Team 5's integration work (via `jni`); this
//! crate owns the exported symbol names and signatures.
//!
//! Real Android FFI is normally invoked the other direction (Kotlin/Java
//! calls into Rust via `#[no_mangle] extern "system" fn Java_...`), but
//! Team Prompt 4 §6.2's C-ABI table lists plain `extern "C"` signatures
//! (mirroring the iOS section), so those are implemented literally here.
//! A `jni`-crate-based JNI entry point wrapping the same logic is a
//! reasonable next step but isn't specified by the prompt; noted as a gap
//! for Team 5's integration, not fabricated here. See DECISIONS.md.

use std::ffi::{c_char, c_void, CStr};

pub struct WifiDirectHandle {
    _marker: std::marker::PhantomData<()>,
}

/// Starts Wi-Fi Direct peer discovery.
///
/// # Safety
/// The returned pointer must later be passed to exactly one call of
/// `android_wifi_direct_stop_discovery`.
#[no_mangle]
pub unsafe extern "C" fn android_wifi_direct_start_discovery() -> *mut c_void {
    // Real implementation (Team 5, via `jni`):
    //   WifiP2pManager.discoverPeers(channel, actionListener)
    let handle = Box::new(WifiDirectHandle {
        _marker: std::marker::PhantomData,
    });
    Box::into_raw(handle) as *mut c_void
}

/// Stops Wi-Fi Direct peer discovery.
///
/// # Safety
/// `handle` must be a pointer previously returned by
/// `android_wifi_direct_start_discovery` and not yet freed.
#[no_mangle]
pub unsafe extern "C" fn android_wifi_direct_stop_discovery(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    drop(Box::from_raw(handle as *mut WifiDirectHandle));
}

/// Connects to a Wi-Fi Direct peer by device address.
///
/// # Safety
/// `device_address` must be a valid, non-null, NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn android_wifi_direct_connect(device_address: *const c_char) -> *mut c_void {
    if device_address.is_null() {
        return std::ptr::null_mut();
    }
    let _addr = match CStr::from_ptr(device_address).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    // Real implementation: WifiP2pManager.connect(channel, config, listener) (Team 5).
    let handle = Box::new(WifiDirectHandle {
        _marker: std::marker::PhantomData,
    });
    Box::into_raw(handle) as *mut c_void
}

/// Sends data over an established Wi-Fi Direct socket.
///
/// # Safety
/// `conn_handle` must be a valid, non-null pointer from
/// `android_wifi_direct_connect`. `data` must be valid for reads of `len`
/// bytes.
#[no_mangle]
pub unsafe extern "C" fn android_wifi_direct_send(
    conn_handle: *mut c_void,
    data: *const u8,
    len: usize,
) -> bool {
    if conn_handle.is_null() || data.is_null() {
        return false;
    }
    let _bytes = std::slice::from_raw_parts(data, len);
    // Real implementation: java.net.Socket over the Wi-Fi Direct group (Team 5).
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_stop_discovery_roundtrip() {
        let handle = unsafe { android_wifi_direct_start_discovery() };
        assert!(!handle.is_null());
        unsafe { android_wifi_direct_stop_discovery(handle) };
    }

    #[test]
    fn connect_rejects_null_address() {
        assert!(unsafe { android_wifi_direct_connect(std::ptr::null()) }.is_null());
    }

    #[test]
    fn send_rejects_null_handle() {
        assert!(!unsafe { android_wifi_direct_send(std::ptr::null_mut(), [1].as_ptr(), 1) });
    }
}
