//! Users list for the Admin Users page (feasibility wiring).
//!
//! `GET /api/admin/users` with the in-memory access token. The Worker is the
//! authorization boundary: it returns 403 for roles that may not list users.

use serde::Deserialize;

const API_BASE: &str = "https://onyx-framework.soheil-mozaffari.workers.dev";

#[derive(Deserialize)]
pub struct UserRow {
    pub username: String,
    pub class: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsersError {
    Forbidden,
    Failed,
}

pub async fn list_users(access_token: &str) -> Result<Vec<UserRow>, UsersError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|_| UsersError::Failed)?;
    let response = client
        .get(format!("{API_BASE}/api/admin/users"))
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    match response.status() {
        reqwest::StatusCode::FORBIDDEN => Err(UsersError::Forbidden),
        status if status.is_success() => response
            .json::<Vec<UserRow>>()
            .await
            .map_err(|_| UsersError::Failed),
        _ => Err(UsersError::Failed),
    }
}
