//! `mobile_core_upload_file`/`mobile_core_download_file` — mobile's
//! counterpart to `desktop-shell`'s `upload_file`/`download_file` Tauri
//! commands (`crates/bins/desktop-shell/src/lib.rs`). Both clients share
//! the same `client_composition::file_upload::FileUploadCoordinator`
//! through their own `AppState`, so the split here mirrors that file's
//! reasoning exactly rather than reinventing it: a file **path** in,
//! not raw bytes, so the native side reads the file directly instead of
//! the caller shipping potentially 100 MB of file content through the
//! FFI boundary as a JSON/C-string argument.

use std::ffi::c_char;

use platform_kernel::{ActorContext, ObjectId, OrganizationId};

use crate::{cstr_to_string, string_to_cstr, MobileApp};

/// Uploads the file at `path` from the local filesystem, returning a
/// JSON string (caller must free via `mobile_core_free_string`) with the
/// same shape as `client_composition::file_upload::UploadOutcome`
/// (`file_asset_id`, `upload_session_id`, `content_hash`, `size_bytes`).
/// Returns null on any failure — invalid arguments, an unreadable file,
/// or a coordinator error.
///
/// MIME type is hardcoded to `"application/octet-stream"`, matching
/// `desktop-shell::upload_file`'s own documented choice: no MIME-sniffing
/// library is a workspace dependency, and guessing from the file
/// extension alone would be a half-measure that silently mislabels
/// files. A real content-type detector is a follow-up for both clients,
/// not something to invent differently here.
///
/// # Safety
/// `handle` must be a valid pointer from `mobile_core_new`. `path`,
/// `organization_id`, `user_id`, and `device_id` must each be valid,
/// NUL-terminated C string pointers.
#[no_mangle]
pub unsafe extern "C" fn mobile_core_upload_file(
    handle: *mut MobileApp,
    path: *const c_char,
    organization_id: *const c_char,
    user_id: *const c_char,
    device_id: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    let app = &*handle;
    let Some(path) = cstr_to_string(path) else {
        return std::ptr::null_mut();
    };
    let Some(organization_id) = cstr_to_string(organization_id) else {
        return std::ptr::null_mut();
    };
    let Some(user_id) = cstr_to_string(user_id) else {
        return std::ptr::null_mut();
    };
    let Some(device_id) = cstr_to_string(device_id) else {
        return std::ptr::null_mut();
    };
    let Ok(organization_id) = organization_id.parse::<OrganizationId>() else {
        return std::ptr::null_mut();
    };
    let Ok(user_id) = user_id.parse::<ObjectId>() else {
        return std::ptr::null_mut();
    };
    let Ok(device_id) = device_id.parse::<ObjectId>() else {
        return std::ptr::null_mut();
    };

    let result = app.runtime.block_on(async {
        let content = tokio::fs::read(&path)
            .await
            .map_err(|e| e.to_string())?;
        let file_name = std::path::Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("untitled")
            .to_string();
        let actor = ActorContext {
            user_id,
            device_id,
            organization_id,
        };
        app.state
            .file_upload_coordinator
            .upload_new_file(actor, file_name, "application/octet-stream".to_string(), &content)
            .await
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(outcome) => match serde_json::to_string(&outcome) {
            Ok(json) => string_to_cstr(json),
            Err(_) => std::ptr::null_mut(),
        },
        Err(_) => std::ptr::null_mut(),
    }
}

/// Downloads the file content stored under `content_hash`, writing it to
/// `destination_path` on the local filesystem. Returns the number of
/// bytes written, or `-1` on any failure (invalid arguments, no stored
/// content for that hash, or a write error) -- mirrors
/// `desktop-shell::download_file`'s `u64`-bytes-written /
/// `InvalidArgument` contract, adapted to a C-ABI-friendly `i64` sentinel
/// since FFI has no `Result` to return through.
///
/// # Safety
/// `handle` must be a valid pointer from `mobile_core_new`.
/// `content_hash` and `destination_path` must each be valid,
/// NUL-terminated C string pointers.
#[no_mangle]
pub unsafe extern "C" fn mobile_core_download_file(
    handle: *mut MobileApp,
    content_hash: *const c_char,
    destination_path: *const c_char,
) -> i64 {
    if handle.is_null() {
        return -1;
    }
    let app = &*handle;
    let Some(content_hash) = cstr_to_string(content_hash) else {
        return -1;
    };
    let Some(destination_path) = cstr_to_string(destination_path) else {
        return -1;
    };

    let result = app.runtime.block_on(async {
        let content = app
            .state
            .file_upload_coordinator
            .download(&content_hash)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("no stored content for hash {content_hash}"))?;
        tokio::fs::write(&destination_path, &content)
            .await
            .map_err(|e| e.to_string())?;
        Ok::<u64, String>(content.len() as u64)
    });

    match result {
        Ok(bytes) => bytes as i64,
        Err(e) => {
            tracing::warn!(error = %e, "mobile_core_download_file failed");
            -1
        }
    }
}

