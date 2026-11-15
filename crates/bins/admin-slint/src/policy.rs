//! Policy administration for the Settings page. Reads use /api/query, writes use
//! /api/command (or POST /api/admin/policies for creation). Every write carries the
//! concurrency values read from the projection, so stale actions are rejected by the
//! Worker. Tokens are memory only (Q-01).

use crate::users::UsersError;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

pub(crate) const API_BASE: &str = "https://onyx-framework.soheil-mozaffari.workers.dev";

#[derive(Clone)]
pub struct PolicyRow {
    pub id: String,
    pub name: String,
    pub status: String,
    pub version: i64,
    pub lifecycle_epoch: i64,
    pub authority_epoch: i64,
}

impl PolicyRow {
    /// The concurrency values a command needs, taken from the projection.
    pub fn concurrency(&self) -> Concurrency {
        Concurrency {
            id: self.id.clone(),
            version: self.version,
            lifecycle_epoch: self.lifecycle_epoch,
            authority_epoch: self.authority_epoch,
        }
    }
}

/// Target id and the three concurrency values a lifecycle command must carry.
#[derive(Clone)]
pub struct Concurrency {
    pub id: String,
    pub version: i64,
    pub lifecycle_epoch: i64,
    pub authority_epoch: i64,
}

#[derive(Deserialize)]
struct QueryResponse {
    data: Vec<Value>,
}

#[derive(Deserialize)]
struct CreatedPolicy {
    policy_id: String,
}

pub(crate) fn client() -> Result<reqwest::Client, UsersError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| UsersError::Failed)
}

pub(crate) fn check(status: reqwest::StatusCode) -> Result<(), UsersError> {
    if status == reqwest::StatusCode::FORBIDDEN {
        Err(UsersError::Forbidden)
    } else if status.is_success() {
        Ok(())
    } else {
        Err(UsersError::Failed)
    }
}

pub(crate) fn int(row: &Value, key: &str) -> Option<i64> {
    row.get(key).and_then(Value::as_i64)
}

/// Turns a query row (`id`, `version`, `lifecycle_epoch`, `authority_epoch`, `state`)
/// into a policy row. Returns `None` if a concurrency value is missing, so no command
/// can be sent with a guessed epoch.
fn to_policy(row: &Value) -> Option<PolicyRow> {
    let state = row.get("state").cloned().unwrap_or(Value::Null);
    Some(PolicyRow {
        id: row.get("id")?.as_str()?.to_string(),
        name: state.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
        status: state.get("status").and_then(Value::as_str).unwrap_or("").to_string(),
        version: int(row, "version")?,
        lifecycle_epoch: int(row, "lifecycle_epoch")?,
        authority_epoch: int(row, "authority_epoch")?,
    })
}

fn envelope(query_type: &str, organization_id: &str, filters: Value) -> String {
    let body = json!({
        "query_id": uuid::Uuid::new_v4().to_string(),
        "query_type": query_type,
        "schema_version": "1.0",
        "organization_id": organization_id,
        "filters": filters,
        "limit": 100,
        "sort_order": "desc",
    });
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(body.to_string())
}

pub(crate) async fn query(token: &str, organization_id: &str, query_type: &str, filters: Value) -> Result<Vec<Value>, UsersError> {
    let response = client()?
        .get(format!("{API_BASE}/api/query"))
        .query(&[("envelope", envelope(query_type, organization_id, filters))])
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    check(response.status())?;
    let body: QueryResponse = response.json().await.map_err(|_| UsersError::Failed)?;
    Ok(body.data)
}

pub async fn list_policies(token: &str, organization_id: &str) -> Result<Vec<PolicyRow>, UsersError> {
    let rows = query(token, organization_id, "policy.list", json!({})).await?;
    Ok(rows.iter().filter_map(to_policy).collect())
}

/// Reads one policy's concurrency values for the detail panel. `None` if not found.
pub async fn policy_detail(token: &str, organization_id: &str, id: &str) -> Result<Option<PolicyRow>, UsersError> {
    let rows = query(token, organization_id, "policy.detail", json!({ "target_id": id })).await?;
    Ok(rows.iter().find(|r| r.get("id").and_then(Value::as_str) == Some(id)).and_then(to_policy))
}

/// Creates a policy. Sends one Idempotency-Key per user action, so a retry cannot
/// create a second policy.
pub async fn create_policy(token: &str, name: &str, idempotency_key: &str) -> Result<String, UsersError> {
    let response = client()?
        .post(format!("{API_BASE}/api/admin/policies"))
        .bearer_auth(token)
        .header("Idempotency-Key", idempotency_key)
        .json(&json!({ "name": name }))
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    check(response.status())?;
    let created: CreatedPolicy = response.json().await.map_err(|_| UsersError::Failed)?;
    Ok(created.policy_id)
}

/// Sends one lifecycle command. Names follow the Worker: `target_type` is "policy" or
/// "legal_hold"; `command_type` is e.g. "policy.PublishPolicyVersion" or "ReleaseLegalHold".
/// Returns the Worker's JSON result.
pub async fn send_command(
    token: &str,
    organization_id: &str,
    target_type: &str,
    command_type: &str,
    target: &Concurrency,
    payload: Value,
) -> Result<Value, UsersError> {
    let envelope = json!({
        "command_id": uuid::Uuid::new_v4().to_string(),
        "operation_id": uuid::Uuid::new_v4().to_string(),
        "command_type": command_type,
        "schema_version": "1.0",
        "target": { "id": target.id, "type": target_type, "organization_id": organization_id },
        "expected_version": target.version,
        "expected_lifecycle_epoch": target.lifecycle_epoch,
        "expected_authority_epoch": target.authority_epoch,
        "issued_at": chrono::Utc::now().to_rfc3339(),
        "vector_clock": { "entries": {} },
        "correlation_id": uuid::Uuid::new_v4().to_string(),
        "causation_id": null,
        "payload": payload,
    });
    let response = client()?
        .post(format!("{API_BASE}/api/command"))
        .bearer_auth(token)
        .json(&envelope)
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    check(response.status())?;
    response.json().await.map_err(|_| UsersError::Failed)
}
