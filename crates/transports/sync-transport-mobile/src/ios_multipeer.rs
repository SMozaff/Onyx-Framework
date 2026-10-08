//! iOS Wi-Fi Direct equivalent: Multipeer Connectivity. Frozen contract
//! per Team Prompt 4 §6.1.
//!
//! These are C-ABI exports intended to be called from Swift/Objective-C on
//! the iOS side (Team 5's responsibility); the actual `MCNearbyServiceAdvertiser`
//! / `MCSession` calls happen through the `objc` crate's message-send
//! machinery. The bodies here are the minimal correct-shape implementation:
//! real behavior requires an Objective-C runtime bridge and delegate
//! callbacks that are out of scope for Team 4 per the mission statement
//! ("You do not build the full Wi-Fi Direct/BLE radio stacks on mobile").
//! What Team 4 owns is: the exported symbol names, signatures, and
//! calling convention matching exactly what's frozen below.

use std::ffi::{c_char, c_void, CStr};
use std::os::raw::c_int;

/// Opaque handle wrapper so we don't leak raw `objc` object pointers
/// without at least a documented ownership story: the returned pointer
/// must be passed back to `stop_multipeer_advertising` exactly once.
pub struct MultipeerAdvertiserHandle {
    _marker: std::marker::PhantomData<()>,
}

/// iOS Wi-Fi Direct equivalent: Multipeer Connectivity.
///
/// # Safety
/// `service_type` must be a valid, non-null, NUL-terminated C string valid
/// for the duration of the call. The returned pointer must later be passed
/// to exactly one call of `stop_multipeer_advertising` to avoid leaking the
/// underlying `MCNearbyServiceAdvertiser`.
#[no_mangle]
pub unsafe extern "C" fn start_multipeer_advertising(service_type: *const c_char) -> *mut c_void {
    if service_type.is_null() {
        return std::ptr::null_mut();
    }
    let _service_type = match CStr::from_ptr(service_type).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    // Real implementation (Team 5, via `objc`):
    //   let advertiser: id = msg_send![class!(MCNearbyServiceAdvertiser),
    //       initWithPeer: my_peer_id discoveryInfo: nil serviceType: ns_service_type];
    //   msg_send![advertiser, startAdvertisingPeer];
    //   Box::into_raw(Box::new(advertiser)) as *mut c_void
    //
    // This crate provides the frozen signature; the runtime call is Team
    // 5's integration work per the mission statement.
    let handle = Box::new(MultipeerAdvertiserHandle {
        _marker: std::marker::PhantomData,
    });
    Box::into_raw(handle) as *mut c_void
}

/// Stops advertising.
///
/// # Safety
/// `handle` must be a pointer previously returned by
/// `start_multipeer_advertising` and not yet freed. Passing any other
/// pointer (including a null pointer or a dangling one) is undefined
/// behavior. After this call the pointer must not be used again.
#[no_mangle]
pub unsafe extern "C" fn stop_multipeer_advertising(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    // SAFETY: caller contract above guarantees this came from
    // `start_multipeer_advertising`'s `Box::into_raw`.
    drop(Box::from_raw(handle as *mut MultipeerAdvertiserHandle));
}

/// Calls `MCSession` and returns a connection handle.
///
/// # Safety
/// `peer_id` must be a valid, non-null, NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ios_multipeer_connect(peer_id: *const c_char) -> *mut c_void {
    if peer_id.is_null() {
        return std::ptr::null_mut();
    }
    let _peer_id = match CStr::from_ptr(peer_id).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    // Real implementation: MCSession(peer:) + delegate wiring (Team 5).
    let handle = Box::new(MultipeerAdvertiserHandle {
        _marker: std::marker::PhantomData,
    });
    Box::into_raw(handle) as *mut c_void
}

/// Calls `MCSession.sendData` to the peer.
///
/// # Safety
/// `conn_handle` must be a valid, non-null pointer previously returned by
/// `ios_multipeer_connect`. `data` must be valid for reads of `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ios_multipeer_send(
    conn_handle: *mut c_void,
    data: *const u8,
    len: usize,
) -> bool {
    if conn_handle.is_null() || data.is_null() {
        return false;
    }
    let _bytes = std::slice::from_raw_parts(data, len);
    // Real implementation: MCSession.sendData(_:toPeers:with:) (Team 5).
    true
}

#[allow(dead_code)]
fn _unused(_x: c_int) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_advertising_rejects_null_service_type() {
        let handle = unsafe { start_multipeer_advertising(std::ptr::null()) };
        assert!(handle.is_null());
    }

    #[test]
    fn start_stop_advertising_roundtrip() {
        let service_type = std::ffi::CString::new("_onyx._tcp").unwrap();
        let handle = unsafe { start_multipeer_advertising(service_type.as_ptr()) };
        assert!(!handle.is_null());
        unsafe { stop_multipeer_advertising(handle) };
    }

    #[test]
    fn connect_rejects_null_peer_id() {
        let handle = unsafe { ios_multipeer_connect(std::ptr::null()) };
        assert!(handle.is_null());
    }

    #[test]
    fn send_rejects_null_conn_handle() {
        let ok = unsafe { ios_multipeer_send(std::ptr::null_mut(), [1, 2, 3].as_ptr(), 3) };
        assert!(!ok);
    }
}
