//! Legal holds for the Settings page: read one hold, apply a hold to a target, and
//! release an applied hold. Apply uses the dedicated REST route with an Idempotency-Key;
//! release is a command carrying the projection's concurrency values. Tokens are memory
//! only (Q-01).

use crate::policy::{self, Concurrency};
use crate::users::UsersError;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone)]
pub struct HoldRow {
    pub status: String,
    pub concurrency: Concurrency,
}

#[derive(Deserialize)]
struct CreatedHold {
    legal_hold_id: String,
}

/// Turns a query row into a hold. `None` if a concurrency value is missing, so no
/// release can be sent with a guessed epoch.
fn to_hold(row: &Value) -> Option<HoldRow> {
    let id = row.get("id")?.as_str()?.to_string();
    let state = row.get("state").cloned().unwrap_or(Value::Null);
    Some(HoldRow {
        status: state.get("status").and_then(Value::as_str).unwrap_or("").to_string(),
        concurrency: Concurrency {
            id,
            version: policy::int(row, "version")?,
            lifecycle_epoch: policy::int(row, "lifecycle_epoch")?,
            authority_epoch: policy::int(row, "authority_epoch")?,
        },
    })
}

/// Reads one hold. `None` if it does not exist in this organization.
pub async fn hold_detail(token: &str, organization_id: &str, id: &str) -> Result<Option<HoldRow>, UsersError> {
    let rows = policy::query(token, organization_id, "legal_hold.detail", json!({ "target_id": id })).await?;
    Ok(rows
        .iter()
        .find(|r| r.get("id").and_then(Value::as_str) == Some(id))
        .and_then(to_hold))
}

/// Applies a hold and returns the new hold id. One Idempotency-Key per user action.
pub async fn apply(
    token: &str,
    target_id: &str,
    target_type: &str,
    reason: &str,
    idempotency_key: &str,
) -> Result<String, UsersError> {
    let response = policy::client()?
        .post(format!("{}/api/admin/legal-holds", policy::API_BASE))
        .bearer_auth(token)
        .header("Idempotency-Key", idempotency_key)
        .json(&json!({ "target_id": target_id, "target_type": target_type, "reason": reason }))
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    policy::check(response.status())?;
    let created: CreatedHold = response.json().await.map_err(|_| UsersError::Failed)?;
    if created.legal_hold_id.is_empty() {
        return Err(UsersError::Failed);
    }
    Ok(created.legal_hold_id)
}

/// Releases an applied hold. The Worker's command name is `ReleaseLegalHold`.
pub async fn release(token: &str, organization_id: &str, hold: &HoldRow, reason: &str) -> Result<(), UsersError> {
    policy::send_command(
        token,
        organization_id,
        "legal_hold",
        "ReleaseLegalHold",
        &hold.concurrency,
        json!({ "reason": reason }),
    )
    .await
    .map(|_| ())
}
