//! Staff profiles for the Admin Profiles page: list, save, import and export.
//! The Worker validates everything. Tokens are memory only (Q-01). File reads and
//! writes live here, not in the token code.

use crate::users::UsersError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const API_BASE: &str = "https://onyx-framework.soheil-mozaffari.workers.dev";

#[derive(Deserialize, Clone)]
pub struct ProfileRow {
    pub owner_id: String,
    pub full_name: String,
    pub photo_blob_key: Option<String>,
    pub job_title: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub department: Option<String>,
    pub class_label: Option<String>,
    pub parent_display_name: Option<String>,
    #[serde(default)]
    pub team_memberships: Vec<String>,
}

#[derive(Serialize)]
pub struct ProfileForm {
    pub owner_id: String,
    pub full_name: String,
    pub job_title: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub department: Option<String>,
    pub photo_blob_key: Option<String>,
    pub class_label: Option<String>,
    pub parent_display_name: Option<String>,
    pub team_memberships: Vec<String>,
}

fn client() -> Result<reqwest::Client, UsersError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|_| UsersError::Failed)
}

fn check(status: reqwest::StatusCode) -> Result<(), UsersError> {
    if status == reqwest::StatusCode::FORBIDDEN {
        Err(UsersError::Forbidden)
    } else if status.is_success() {
        Ok(())
    } else {
        Err(UsersError::Failed)
    }
}

pub async fn list_profiles(token: &str) -> Result<Vec<ProfileRow>, UsersError> {
    let response = client()?
        .get(format!("{API_BASE}/api/profiles"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    check(response.status())?;
    response.json().await.map_err(|_| UsersError::Failed)
}

pub async fn save_profile(token: &str, form: &ProfileForm) -> Result<(), UsersError> {
    let response = client()?
        .put(format!("{API_BASE}/api/admin/profiles"))
        .bearer_auth(token)
        .json(form)
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    check(response.status())
}

/// Creation report returned by the Worker for one import batch. It is an .xlsx
/// file that contains the one-time passwords of the new accounts.
pub struct ImportReport {
    pub bytes: Vec<u8>,
    pub total: String,
    pub created: String,
    pub failed: String,
}

/// Sends the raw file as the request body. The Worker detects .xlsx by content,
/// and CSV or JSON by Content-Type. Every non-empty row creates an account.
pub async fn import_bytes(
    token: &str,
    bytes: Vec<u8>,
    content_type: &str,
) -> Result<ImportReport, UsersError> {
    let response = client()?
        .post(format!("{API_BASE}/api/admin/profiles/import"))
        .bearer_auth(token)
        .header("Content-Type", content_type)
        .body(bytes)
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    check(response.status())?;
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("0")
            .to_string()
    };
    let (total, created, failed) = (
        header("x-onyx-import-total"),
        header("x-onyx-import-created"),
        header("x-onyx-import-failed"),
    );
    let bytes = response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|_| UsersError::Failed)?;
    Ok(ImportReport {
        bytes,
        total,
        created,
        failed,
    })
}

pub async fn export_bytes(token: &str, format: &str) -> Result<Vec<u8>, UsersError> {
    let response = client()?
        .get(format!("{API_BASE}/api/admin/profiles/export"))
        .query(&[("format", format)])
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    check(response.status())?;
    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|_| UsersError::Failed)
}

/// Opens a file picker on the calling thread. `None` means the user cancelled.
pub fn pick_import_file() -> Option<(Vec<u8>, &'static str)> {
    let path = rfd::FileDialog::new()
        .add_filter("Excel, CSV or JSON", &["xlsx", "csv", "json"])
        .pick_file()?;
    let content_type = match path.extension().and_then(|e| e.to_str()) {
        Some("json") => "application/json",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "text/csv",
    };
    std::fs::read(path).ok().map(|bytes| (bytes, content_type))
}

/// Opens a save dialog and writes the bytes. `Ok(false)` means cancelled.
pub fn save_file(bytes: &[u8], file_name: &str) -> Result<bool, UsersError> {
    let Some(path) = rfd::FileDialog::new().set_file_name(file_name).save_file() else {
        return Ok(false);
    };
    std::fs::write(path, bytes).map_err(|_| UsersError::Failed)?;
    Ok(true)
}
