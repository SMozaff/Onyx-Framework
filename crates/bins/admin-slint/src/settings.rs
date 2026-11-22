//! Mobile-access policy for the Admin Settings page. The Worker validates the
//! classes and the expected version, and rejects stale saves. Memory-only tokens (Q-01).

use crate::users::UsersError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const API_BASE: &str = "https://onyx-framework.soheil-mozaffari.workers.dev";

#[derive(Deserialize, Clone)]
pub struct MobileAccess {
    pub allowed_classes: Vec<String>,
    pub version: i64,
    pub enforcement_status: String,
}

#[derive(Serialize)]
struct SaveBody<'a> {
    allowed_classes: &'a [String],
    expected_version: i64,
}

fn client() -> Result<reqwest::Client, UsersError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
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

pub async fn get_mobile_access(token: &str) -> Result<MobileAccess, UsersError> {
    let response = client()?
        .get(format!("{API_BASE}/api/admin/mobile-access"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    check(response.status())?;
    response.json().await.map_err(|_| UsersError::Failed)
}

pub async fn put_mobile_access(
    token: &str,
    allowed_classes: &[String],
    expected_version: i64,
) -> Result<MobileAccess, UsersError> {
    let response = client()?
        .put(format!("{API_BASE}/api/admin/mobile-access"))
        .bearer_auth(token)
        .json(&SaveBody { allowed_classes, expected_version })
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    check(response.status())?;
    response.json().await.map_err(|_| UsersError::Failed)
}
