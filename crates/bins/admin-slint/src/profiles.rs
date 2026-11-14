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

/// Sends the raw file as the request body. The Worker parses CSV or JSON by Content-Type.
pub async fn import_bytes(token: &str, bytes: Vec<u8>, content_type: &str) -> Result<(), UsersError> {
    let response = client()?
        .post(format!("{API_BASE}/api/admin/profiles/import"))
        .bearer_auth(token)
        .header("Content-Type", content_type)
        .body(bytes)
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    check(response.status())
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
        .add_filter("Profiles", &["csv", "json"])
        .pick_file()?;
    let content_type = match path.extension().and_then(|e| e.to_str()) {
        Some("json") => "application/json",
        _ => "text/csv",
    };
    std::fs::read(path).ok().map(|bytes| (bytes, content_type))
}

/// Opens a save dialog and writes the exported bytes. `Ok(false)` means cancelled.
pub fn save_export_file(bytes: &[u8], format: &str) -> Result<bool, UsersError> {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(format!("profiles.{format}"))
        .save_file()
    else {
        return Ok(false);
    };
    std::fs::write(path, bytes).map_err(|_| UsersError::Failed)?;
    Ok(true)
}
