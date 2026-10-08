//! iOS Bluetooth LE bridge. Frozen contract per Team Prompt 4 §6.3.
//! CoreBluetooth calls are Team 5's integration work (via `objc`); this
//! crate owns the exported C-ABI symbol names and signatures.

use std::ffi::{c_char, c_void, CStr};

pub struct BLEHandle {
    _marker: std::marker::PhantomData<()>,
}

/// Starts BLE advertising with the given service UUID string.
///
/// # Safety
/// `service_uuid` must be a valid, non-null, NUL-terminated C string.
/// The returned pointer must later be passed to exactly one call of
/// `ios_ble_stop_advertising`.
#[no_mangle]
pub unsafe extern "C" fn ios_ble_start_advertising(service_uuid: *const c_char) -> *mut c_void {
    if service_uuid.is_null() {
        return std::ptr::null_mut();
    }
    let _uuid = match CStr::from_ptr(service_uuid).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    // Real implementation (Team 5, via `objc`):
    //   CBPeripheralManager.startAdvertising([CBAdvertisementDataServiceUUIDsKey: [uuid]])
    let handle = Box::new(BLEHandle {
        _marker: std::marker::PhantomData,
    });
    Box::into_raw(handle) as *mut c_void
}

/// Stops BLE advertising.
///
/// # Safety
/// `handle` must be a pointer previously returned by
/// `ios_ble_start_advertising` and not yet freed.
#[no_mangle]
pub unsafe extern "C" fn ios_ble_stop_advertising(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    drop(Box::from_raw(handle as *mut BLEHandle));
}

/// Connects to a BLE peripheral by peer ID.
///
/// # Safety
/// `peer_id` must be a valid, non-null, NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ios_ble_connect(peer_id: *const c_char) -> *mut c_void {
    if peer_id.is_null() {
        return std::ptr::null_mut();
    }
    let _peer_id = match CStr::from_ptr(peer_id).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    // Real implementation: CBCentralManager.connect(peripheral) (Team 5).
    let handle = Box::new(BLEHandle {
        _marker: std::marker::PhantomData,
    });
    Box::into_raw(handle) as *mut c_void
}

/// Sends data over an established BLE GATT characteristic.
///
/// # Safety
/// `conn_handle` must be a valid, non-null pointer from `ios_ble_connect`.
/// `data` must be valid for reads of `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ios_ble_send(
    conn_handle: *mut c_void,
    data: *const u8,
    len: usize,
) -> bool {
    if conn_handle.is_null() || data.is_null() {
        return false;
    }
    let _bytes = std::slice::from_raw_parts(data, len);
    // Real implementation: CBPeripheral.writeValue(_:for:type:) (Team 5).
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_advertising_rejects_null_uuid() {
        assert!(unsafe { ios_ble_start_advertising(std::ptr::null()) }.is_null());
    }

    #[test]
    fn start_stop_advertising_roundtrip() {
        let uuid = std::ffi::CString::new("0000FEED-0000-1000-8000-00805F9B34FB").unwrap();
        let handle = unsafe { ios_ble_start_advertising(uuid.as_ptr()) };
        assert!(!handle.is_null());
        unsafe { ios_ble_stop_advertising(handle) };
    }

    #[test]
    fn connect_rejects_null_peer_id() {
        assert!(unsafe { ios_ble_connect(std::ptr::null()) }.is_null());
    }

    #[test]
    fn send_rejects_null_handle() {
        assert!(!unsafe { ios_ble_send(std::ptr::null_mut(), [1].as_ptr(), 1) });
    }
}
