//! Android Bluetooth LE bridge. Frozen contract per Team Prompt 4 §6.4.
//! `BluetoothLeAdvertiser`/`BluetoothGatt` calls are Team 5's integration
//! work (via `jni`); this crate owns the exported symbol names and
//! signatures. Same C-ABI-vs-JNI-entry-point note as
//! `android_wifi_direct.rs` applies here — see DECISIONS.md.

use std::ffi::{c_char, c_void, CStr};

pub struct AndroidBLEHandle {
    _marker: std::marker::PhantomData<()>,
}

/// Starts BLE advertising with the given service UUID string.
///
/// # Safety
/// `service_uuid` must be a valid, non-null, NUL-terminated C string.
/// The returned pointer must later be passed to exactly one call of
/// `android_ble_stop_advertising`.
#[no_mangle]
pub unsafe extern "C" fn android_ble_start_advertising(service_uuid: *const c_char) -> *mut c_void {
    if service_uuid.is_null() {
        return std::ptr::null_mut();
    }
    let _uuid = match CStr::from_ptr(service_uuid).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    // Real implementation (Team 5, via `jni`):
    //   BluetoothLeAdvertiser.startAdvertising(settings, data, callback)
    let handle = Box::new(AndroidBLEHandle {
        _marker: std::marker::PhantomData,
    });
    Box::into_raw(handle) as *mut c_void
}

/// Stops BLE advertising.
///
/// # Safety
/// `handle` must be a pointer previously returned by
/// `android_ble_start_advertising` and not yet freed.
#[no_mangle]
pub unsafe extern "C" fn android_ble_stop_advertising(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    drop(Box::from_raw(handle as *mut AndroidBLEHandle));
}

/// Connects to a BLE device by MAC address.
///
/// # Safety
/// `device_address` must be a valid, non-null, NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn android_ble_connect(device_address: *const c_char) -> *mut c_void {
    if device_address.is_null() {
        return std::ptr::null_mut();
    }
    let _addr = match CStr::from_ptr(device_address).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    // Real implementation: BluetoothDevice.connectGatt(...) (Team 5).
    let handle = Box::new(AndroidBLEHandle {
        _marker: std::marker::PhantomData,
    });
    Box::into_raw(handle) as *mut c_void
}

/// Sends data over an established BLE GATT characteristic.
///
/// # Safety
/// `conn_handle` must be a valid, non-null pointer from
/// `android_ble_connect`. `data` must be valid for reads of `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn android_ble_send(
    conn_handle: *mut c_void,
    data: *const u8,
    len: usize,
) -> bool {
    if conn_handle.is_null() || data.is_null() {
        return false;
    }
    let _bytes = std::slice::from_raw_parts(data, len);
    // Real implementation: BluetoothGatt.writeCharacteristic(...) (Team 5).
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_advertising_rejects_null_uuid() {
        assert!(unsafe { android_ble_start_advertising(std::ptr::null()) }.is_null());
    }

    #[test]
    fn start_stop_advertising_roundtrip() {
        let uuid = std::ffi::CString::new("0000FEED-0000-1000-8000-00805F9B34FB").unwrap();
        let handle = unsafe { android_ble_start_advertising(uuid.as_ptr()) };
        assert!(!handle.is_null());
        unsafe { android_ble_stop_advertising(handle) };
    }

    #[test]
    fn connect_rejects_null_address() {
        assert!(unsafe { android_ble_connect(std::ptr::null()) }.is_null());
    }

    #[test]
    fn send_rejects_null_handle() {
        assert!(!unsafe { android_ble_send(std::ptr::null_mut(), [1].as_ptr(), 1) });
    }
}
