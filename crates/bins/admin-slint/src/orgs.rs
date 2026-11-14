//! Organizations and organization Admins for the All-Father page. The Worker
//! enforces All-Father authority on every route. Tokens are memory only (Q-01).

use crate::users::UsersError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const API_BASE: &str = "https://onyx-framework.soheil-mozaffari.workers.dev";

#[derive(Deserialize)]
pub struct OrgRow {
    pub id: String,
    pub name: String,
    pub is_active: bool,
}

#[derive(Deserialize)]
pub struct AdminRow {
    pub id: String,
    pub username: String,
    pub is_active: bool,
}

#[derive(Serialize)]
struct NameBody<'a> {
    name: &'a str,
}

#[derive(Serialize)]
struct CreateAdminBody<'a> {
    organization_id: &'a str,
    username: &'a str,
    email: &'a str,
    password: &'a str,
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

fn transport(_: reqwest::Error) -> UsersError {
    UsersError::Failed
}

pub async fn list_orgs(token: &str) -> Result<Vec<OrgRow>, UsersError> {
    let response = client()?
        .get(format!("{API_BASE}/api/allfather/organizations"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(transport)?;
    check(response.status())?;
    response.json().await.map_err(|_| UsersError::Failed)
}

pub async fn create_org(token: &str, name: &str) -> Result<(), UsersError> {
    let response = client()?
        .post(format!("{API_BASE}/api/allfather/organizations"))
        .bearer_auth(token)
        .json(&NameBody { name })
        .send()
        .await
        .map_err(transport)?;
    check(response.status())
}

pub async fn rename_org(token: &str, org_id: &str, name: &str) -> Result<(), UsersError> {
    let response = client()?
        .patch(format!("{API_BASE}/api/allfather/organizations/{org_id}"))
        .bearer_auth(token)
        .json(&NameBody { name })
        .send()
        .await
        .map_err(transport)?;
    check(response.status())
}

pub async fn set_org_active(token: &str, org_id: &str, active: bool) -> Result<(), UsersError> {
    let action = if active { "activate" } else { "deactivate" };
    let response = client()?
        .post(format!("{API_BASE}/api/allfather/organizations/{org_id}/{action}"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(transport)?;
    check(response.status())
}

pub async fn list_admins(token: &str, org_id: &str) -> Result<Vec<AdminRow>, UsersError> {
    let response = client()?
        .get(format!("{API_BASE}/api/allfather/admins"))
        .query(&[("organization_id", org_id)])
        .bearer_auth(token)
        .send()
        .await
        .map_err(transport)?;
    check(response.status())?;
    response.json().await.map_err(|_| UsersError::Failed)
}

pub async fn create_admin(
    token: &str,
    org_id: &str,
    username: &str,
    email: &str,
    password: &str,
) -> Result<(), UsersError> {
    let response = client()?
        .post(format!("{API_BASE}/api/allfather/admins"))
        .bearer_auth(token)
        .json(&CreateAdminBody { organization_id: org_id, username, email, password })
        .send()
        .await
        .map_err(transport)?;
    check(response.status())
}

/// Deactivates an org Admin (DELETE) or reactivates one (POST), as the Tauri Admin does.
pub async fn set_admin_active(token: &str, admin_id: &str, active: bool) -> Result<(), UsersError> {
    let request = if active {
        client()?.post(format!("{API_BASE}/api/allfather/admins/{admin_id}/reactivate"))
    } else {
        client()?.delete(format!("{API_BASE}/api/admin/users/{admin_id}"))
    };
    let response = request.bearer_auth(token).send().await.map_err(transport)?;
    check(response.status())
}
