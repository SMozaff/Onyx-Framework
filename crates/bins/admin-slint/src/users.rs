//! Users for the Admin Users page. Every call goes through the Worker, which is
//! the authorization boundary. Tokens come from memory only (Q-01).

use serde::{Deserialize, Serialize};
use std::time::Duration;

const API_BASE: &str = "https://onyx-framework.soheil-mozaffari.workers.dev";

#[derive(Deserialize)]
pub struct UserRow {
    pub id: String,
    pub username: String,
    pub class: Option<String>,
    pub parent_user_id: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsersError {
    Forbidden,
    Failed,
}

#[derive(Serialize)]
struct CreateUserBody<'a> {
    username: &'a str,
    email: &'a str,
    password: &'a str,
    class: Option<&'a str>,
}

#[derive(Serialize)]
struct ClassBody<'a> {
    class: Option<&'a str>,
}

#[derive(Serialize)]
struct ParentBody<'a> {
    parent_user_id: Option<&'a str>,
}

fn client() -> Result<reqwest::Client, UsersError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| UsersError::Failed)
}

fn map_status(status: reqwest::StatusCode) -> Result<(), UsersError> {
    if status == reqwest::StatusCode::FORBIDDEN {
        Err(UsersError::Forbidden)
    } else if status.is_success() {
        Ok(())
    } else {
        Err(UsersError::Failed)
    }
}

pub async fn list_users(access_token: &str) -> Result<Vec<UserRow>, UsersError> {
    let response = client()?
        .get(format!("{API_BASE}/api/admin/users"))
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    map_status(response.status())?;
    response
        .json::<Vec<UserRow>>()
        .await
        .map_err(|_| UsersError::Failed)
}

pub async fn create_user(
    access_token: &str,
    username: &str,
    email: &str,
    password: &str,
    class: Option<&str>,
) -> Result<(), UsersError> {
    let response = client()?
        .post(format!("{API_BASE}/api/admin/users"))
        .bearer_auth(access_token)
        .json(&CreateUserBody { username, email, password, class })
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    map_status(response.status())
}

pub async fn set_class(access_token: &str, user_id: &str, class: Option<&str>) -> Result<(), UsersError> {
    let response = client()?
        .post(format!("{API_BASE}/api/admin/users/{user_id}/class"))
        .bearer_auth(access_token)
        .json(&ClassBody { class })
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    map_status(response.status())
}

pub async fn set_parent(access_token: &str, user_id: &str, parent_id: Option<&str>) -> Result<(), UsersError> {
    let response = client()?
        .post(format!("{API_BASE}/api/admin/users/{user_id}/parent"))
        .bearer_auth(access_token)
        .json(&ParentBody { parent_user_id: parent_id })
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    map_status(response.status())
}

pub async fn set_active(access_token: &str, user_id: &str, active: bool) -> Result<(), UsersError> {
    let action = if active { "activate" } else { "deactivate" };
    let response = client()?
        .post(format!("{API_BASE}/api/admin/users/{user_id}/{action}"))
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    map_status(response.status())
}
