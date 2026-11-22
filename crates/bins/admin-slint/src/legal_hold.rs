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

impl HoldRow {
    pub fn id(&self) -> &str {
        &self.concurrency.id
    }
}

/// One row of the legal-hold list. Deliberately carries no concurrency values:
/// the list is for navigation, and the detail read decides whether a release is
/// safe.
pub struct HoldSummary {
    pub id: String,
    pub status: String,
}

/// One legal-hold lookup. A missing row and a row without concurrency values are
/// different failures: the second must never become a release command with
/// guessed epochs.
pub enum HoldDetail {
    Found(HoldRow),
    NotFound,
    MissingConcurrency,
}

/// The inputs of one apply action plus the Idempotency-Key that belongs to it.
/// A retry of the same action keeps the key, so an uncertain failure cannot
/// create a second hold. Changed inputs are a new action with a new key.
#[derive(Clone, PartialEq, Eq)]
pub struct ApplyRequest {
    pub target_id: String,
    pub target_type: String,
    pub reason: String,
    pub idempotency_key: String,
}

impl ApplyRequest {
    fn matches(&self, target_id: &str, target_type: &str, reason: &str) -> bool {
        self.target_id == target_id && self.target_type == target_type && self.reason == reason
    }
}

/// Reuses the previous key when the action is the same, otherwise calls
/// `new_key`. Pure, so retry behaviour is unit-tested without the UI.
pub fn prepare_apply(
    previous: Option<&ApplyRequest>,
    target_id: &str,
    target_type: &str,
    reason: &str,
    new_key: impl FnOnce() -> String,
) -> ApplyRequest {
    match previous.filter(|request| request.matches(target_id, target_type, reason)) {
        Some(request) => request.clone(),
        None => ApplyRequest {
            target_id: target_id.to_string(),
            target_type: target_type.to_string(),
            reason: reason.to_string(),
            idempotency_key: new_key(),
        },
    }
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
        status: state
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        concurrency: Concurrency {
            id,
            version: policy::int(row, "version")?,
            lifecycle_epoch: policy::int(row, "lifecycle_epoch")?,
            authority_epoch: policy::int(row, "authority_epoch")?,
        },
    })
}

/// Reads one hold.
pub async fn hold_detail(
    token: &str,
    organization_id: &str,
    id: &str,
) -> Result<HoldDetail, UsersError> {
    let rows = policy::query(
        token,
        organization_id,
        "legal_hold.detail",
        json!({ "target_id": id }),
    )
    .await?;
    let Some(row) = rows
        .iter()
        .find(|r| r.get("id").and_then(Value::as_str) == Some(id))
    else {
        return Ok(HoldDetail::NotFound);
    };
    Ok(match to_hold(row) {
        Some(hold) => HoldDetail::Found(hold),
        None => HoldDetail::MissingConcurrency,
    })
}

fn to_summary(row: &Value) -> Option<HoldSummary> {
    Some(HoldSummary {
        id: row.get("id")?.as_str()?.to_string(),
        status: row
            .get("state")
            .and_then(|state| state.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

/// Lists the holds of the signed-in admin's organization. The Worker caps the
/// page at its query limit; the UI shows what it receives.
pub async fn list_holds(
    token: &str,
    organization_id: &str,
) -> Result<Vec<HoldSummary>, UsersError> {
    let rows = policy::query(token, organization_id, "legal_hold.list", json!({})).await?;
    Ok(rows.iter().filter_map(to_summary).collect())
}

/// Applies a hold and returns the new hold id. The caller owns the
/// Idempotency-Key and must reuse it for retries of the same action.
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
    let response = policy::check(response).await?;
    let created: CreatedHold = response.json().await.map_err(|_| UsersError::Failed)?;
    if created.legal_hold_id.is_empty() {
        return Err(UsersError::Failed);
    }
    Ok(created.legal_hold_id)
}

/// Releases an applied hold. The Worker's command name is `ReleaseLegalHold`.
pub async fn release(
    token: &str,
    organization_id: &str,
    hold: &HoldRow,
    reason: &str,
) -> Result<(), UsersError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ApplyRequest {
        ApplyRequest {
            target_id: "target-1".into(),
            target_type: "file_asset".into(),
            reason: "Litigation".into(),
            idempotency_key: "key-1".into(),
        }
    }

    #[test]
    fn a_retry_of_the_same_action_reuses_the_key() {
        let first = request();
        let retried = prepare_apply(Some(&first), "target-1", "file_asset", "Litigation", || {
            "key-2".to_string()
        });
        assert_eq!(retried.idempotency_key, "key-1");
    }

    #[test]
    fn changed_inputs_are_a_new_action_with_a_new_key() {
        let first = request();
        for (target, kind, reason) in [
            ("target-2", "file_asset", "Litigation"),
            ("target-1", "message", "Litigation"),
            ("target-1", "file_asset", "Investigation"),
        ] {
            let next = prepare_apply(Some(&first), target, kind, reason, || "key-2".to_string());
            assert_eq!(next.idempotency_key, "key-2");
        }
    }

    #[test]
    fn no_previous_action_generates_a_key() {
        let next = prepare_apply(None, "target-1", "file_asset", "Litigation", || {
            "key-3".to_string()
        });
        assert_eq!(next.idempotency_key, "key-3");
    }

    #[test]
    fn a_hold_without_epochs_is_not_release_ready() {
        let row = json!({"id": "hold-1", "version": 2, "state": {"status": "Applied"}});
        assert!(to_hold(&row).is_none());
    }

    #[test]
    fn the_list_shows_holds_that_are_not_release_ready() {
        let row = json!({"id": "hold-1", "state": {"status": "Applied"}});
        let summary = to_summary(&row).expect("summary");
        assert_eq!(summary.id, "hold-1");
        assert_eq!(summary.status, "Applied");
        assert!(to_summary(&json!({"state": {"status": "Applied"}})).is_none());
    }

    #[test]
    fn a_hold_with_epochs_keeps_its_concurrency_values() {
        let row = json!({
            "id": "hold-1",
            "version": 2,
            "lifecycle_epoch": 5,
            "authority_epoch": 7,
            "state": {"status": "Applied"}
        });
        let hold = to_hold(&row).expect("hold");
        assert_eq!(hold.id(), "hold-1");
        assert_eq!(hold.status, "Applied");
        assert_eq!(hold.concurrency.version, 2);
        assert_eq!(hold.concurrency.lifecycle_epoch, 5);
        assert_eq!(hold.concurrency.authority_epoch, 7);
    }
}
