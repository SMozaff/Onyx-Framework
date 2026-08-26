//! Real, end-to-end proof that mobile file sharing works through the
//! actual `extern "C"` FFI surface, mirroring `desktop-shell`'s own
//! `upload_file`/`download_file` Tauri commands byte-for-byte since both
//! sit on the same shared `FileUploadCoordinator`.
//!
//! Confirms:
//! - A real file on disk can be uploaded via `mobile_core_upload_file`
//!   and the returned JSON has the expected `UploadOutcome` shape.
//! - The uploaded content can be downloaded back via
//!   `mobile_core_download_file` and matches byte-for-byte.
//! - A file exceeding `file_domain::value::MAX_FILE_SIZE_BYTES` is
//!   rejected with a clear failure (`-1`), not silently truncated or
//!   accepted.

use mobile_core::{mobile_core_download_file, mobile_core_free, mobile_core_new, mobile_core_upload_file};
use platform_kernel::{ObjectId, OrganizationId};
use std::ffi::{CStr, CString};

fn test_db_path() -> String {
    std::env::temp_dir()
        .join(format!("mobile-core-file-sharing-test-{}.sqlite", uuid::Uuid::new_v4()))
        .to_string_lossy()
        .into_owned()
}

fn config_json(organization_id: OrganizationId) -> String {
    serde_json::json!({
        "organization_id": organization_id,
        "cloud_relay_endpoint": "wss://relay.test.invalid/v1",
        "sync_interval_secs": 3600,
    })
    .to_string()
}

#[test]
fn upload_then_download_round_trips_byte_for_byte_through_real_ffi() {
    let organization_id = OrganizationId::new_random();
    let db_path = CString::new(test_db_path()).unwrap();
    let config = CString::new(config_json(organization_id)).unwrap();
    let handle = unsafe { mobile_core_new(db_path.as_ptr(), config.as_ptr()) };
    assert!(!handle.is_null(), "mobile_core_new should succeed");

    let user_id = ObjectId::new_random();
    let device_id = ObjectId::new_random();

    let source_path = std::env::temp_dir().join(format!("mobile-core-upload-source-{}.bin", uuid::Uuid::new_v4()));
    let content: Vec<u8> = (0..10_000u32).map(|i| (i % 256) as u8).collect();
    std::fs::write(&source_path, &content).unwrap();

    let source_path_c = CString::new(source_path.to_string_lossy().into_owned()).unwrap();
    let organization_id_c = CString::new(organization_id.to_string()).unwrap();
    let user_id_c = CString::new(user_id.to_string()).unwrap();
    let device_id_c = CString::new(device_id.to_string()).unwrap();

    let upload_result_ptr = unsafe {
        mobile_core_upload_file(
            handle,
            source_path_c.as_ptr(),
            organization_id_c.as_ptr(),
            user_id_c.as_ptr(),
            device_id_c.as_ptr(),
        )
    };
    assert!(!upload_result_ptr.is_null(), "upload should succeed for a well-formed request");
    let upload_result_str = unsafe { CStr::from_ptr(upload_result_ptr) }.to_str().unwrap().to_string();
    let upload_result: serde_json::Value = serde_json::from_str(&upload_result_str).unwrap();
    assert_eq!(upload_result["size_bytes"], serde_json::json!(content.len() as u64));
    let content_hash = upload_result["content_hash"].as_str().unwrap().to_string();

    let destination_path = std::env::temp_dir().join(format!("mobile-core-download-dest-{}.bin", uuid::Uuid::new_v4()));
    let content_hash_c = CString::new(content_hash).unwrap();
    let destination_path_c = CString::new(destination_path.to_string_lossy().into_owned()).unwrap();
    let bytes_written = unsafe { mobile_core_download_file(handle, content_hash_c.as_ptr(), destination_path_c.as_ptr()) };
    assert_eq!(bytes_written, content.len() as i64);

    let downloaded = std::fs::read(&destination_path).unwrap();
    assert_eq!(downloaded, content, "downloaded content must match the uploaded content byte-for-byte");

    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&destination_path);
    unsafe { mobile_core_free(handle) };
}

#[test]
fn upload_rejects_a_file_exceeding_the_max_size_with_a_clear_failure() {
    let organization_id = OrganizationId::new_random();
    let db_path = CString::new(test_db_path()).unwrap();
    let config = CString::new(config_json(organization_id)).unwrap();
    let handle = unsafe { mobile_core_new(db_path.as_ptr(), config.as_ptr()) };
    assert!(!handle.is_null(), "mobile_core_new should succeed");

    let user_id = ObjectId::new_random();
    let device_id = ObjectId::new_random();

    // One byte over file_domain::value::MAX_FILE_SIZE_BYTES (100 MiB).
    let oversized_path = std::env::temp_dir().join(format!("mobile-core-oversized-{}.bin", uuid::Uuid::new_v4()));
    let oversized_size: u64 = 100 * 1024 * 1024 + 1;
    {
        let file = std::fs::File::create(&oversized_path).unwrap();
        file.set_len(oversized_size).unwrap();
    }

    let source_path_c = CString::new(oversized_path.to_string_lossy().into_owned()).unwrap();
    let organization_id_c = CString::new(organization_id.to_string()).unwrap();
    let user_id_c = CString::new(user_id.to_string()).unwrap();
    let device_id_c = CString::new(device_id.to_string()).unwrap();

    let upload_result_ptr = unsafe {
        mobile_core_upload_file(
            handle,
            source_path_c.as_ptr(),
            organization_id_c.as_ptr(),
            user_id_c.as_ptr(),
            device_id_c.as_ptr(),
        )
    };
    assert!(upload_result_ptr.is_null(), "an oversized file must be rejected, not silently accepted");

    let _ = std::fs::remove_file(&oversized_path);
    unsafe { mobile_core_free(handle) };
}
