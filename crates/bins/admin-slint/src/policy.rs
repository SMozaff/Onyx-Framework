//! Policy administration for the Settings page. Reads use /api/query, writes use
//! /api/command (or POST /api/admin/policies for creation). Every write carries the
//! concurrency values read from the projection, so stale actions are rejected by the
//! Worker. Tokens are memory only (Q-01).

use crate::i18n::{text, Lang};
use crate::users::{ConcurrencyCode, UsersError};
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

/// One policy lookup. A missing row and a row without concurrency values are
/// different failures: the second must never become a command with guessed epochs.
pub enum PolicyDetail {
    Found(PolicyRow),
    NotFound,
    MissingConcurrency,
}

pub(crate) fn client() -> Result<reqwest::Client, UsersError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| UsersError::Failed)
}

/// Maps the Worker's stable code. Unknown codes stay `None` and are reported as a
/// generic failure; they are never guessed from the message text.
pub(crate) fn concurrency_code(code: &str) -> Option<ConcurrencyCode> {
    match code {
        "LIFECYCLE_EPOCH_REQUIRED" => Some(ConcurrencyCode::LifecycleEpochRequired),
        "AUTHORITY_EPOCH_REQUIRED" => Some(ConcurrencyCode::AuthorityEpochRequired),
        "EPOCH_MISMATCH" => Some(ConcurrencyCode::EpochMismatch),
        "VERSION_CONFLICT" => Some(ConcurrencyCode::VersionConflict),
        _ => None,
    }
}

/// Reads the stable code from a Worker error body. The command route wraps it as
/// `{"error":{"code":"…"}}`; other routes use `{"error":"…"}`. Both are accepted.
pub(crate) fn error_code(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    match value.get("error")? {
        Value::String(code) => Some(code.clone()),
        Value::Object(object) => object
            .get("code")
            .and_then(Value::as_str)
            .map(str::to_owned),
        _ => None,
    }
}

/// Checks a response and keeps a concurrency code instead of collapsing every
/// failure into `Failed`. The body is read only on failure.
pub(crate) async fn check(response: reqwest::Response) -> Result<reqwest::Response, UsersError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    if status == reqwest::StatusCode::FORBIDDEN {
        return Err(UsersError::Forbidden);
    }
    let body = response.text().await.unwrap_or_default();
    match error_code(&body).as_deref().and_then(concurrency_code) {
        Some(code) => Err(UsersError::Conflict(code)),
        None => Err(UsersError::Failed),
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
        name: state
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        status: state
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
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

pub(crate) async fn query(
    token: &str,
    organization_id: &str,
    query_type: &str,
    filters: Value,
) -> Result<Vec<Value>, UsersError> {
    let response = client()?
        .get(format!("{API_BASE}/api/query"))
        .query(&[("envelope", envelope(query_type, organization_id, filters))])
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    let response = check(response).await?;
    let body: QueryResponse = response.json().await.map_err(|_| UsersError::Failed)?;
    Ok(body.data)
}

pub async fn list_policies(
    token: &str,
    organization_id: &str,
) -> Result<Vec<PolicyRow>, UsersError> {
    let rows = query(token, organization_id, "policy.list", json!({})).await?;
    Ok(rows.iter().filter_map(to_policy).collect())
}

/// Reads one policy's concurrency values for the detail panel.
pub async fn policy_detail(
    token: &str,
    organization_id: &str,
    id: &str,
) -> Result<PolicyDetail, UsersError> {
    let rows = query(
        token,
        organization_id,
        "policy.detail",
        json!({ "target_id": id }),
    )
    .await?;
    let Some(row) = rows
        .iter()
        .find(|r| r.get("id").and_then(Value::as_str) == Some(id))
    else {
        return Ok(PolicyDetail::NotFound);
    };
    Ok(match to_policy(row) {
        Some(policy) => PolicyDetail::Found(policy),
        None => PolicyDetail::MissingConcurrency,
    })
}

/// Creates a policy. Sends one Idempotency-Key per user action, so a retry cannot
/// create a second policy.
pub async fn create_policy(
    token: &str,
    name: &str,
    idempotency_key: &str,
) -> Result<String, UsersError> {
    let response = client()?
        .post(format!("{API_BASE}/api/admin/policies"))
        .bearer_auth(token)
        .header("Idempotency-Key", idempotency_key)
        .json(&json!({ "name": name }))
        .send()
        .await
        .map_err(|_| UsersError::Failed)?;
    let response = check(response).await?;
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
    let response = check(response).await?;
    response.json().await.map_err(|_| UsersError::Failed)
}

/// Turns an entered rule value into JSON: true/false, a number, or text.
pub fn parse_rule_value(raw: &str) -> Value {
    match raw {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => raw
            .parse::<i64>()
            .map(Value::from)
            .or_else(|_| raw.parse::<f64>().map(Value::from))
            .unwrap_or_else(|_| Value::String(raw.to_string())),
    }
}

/// Short display form of a pending rule value. Text is shown as entered; other
/// JSON values keep their JSON form (`true`, `12`, `1.5`).
pub fn rule_value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// What the Settings page shows after an evaluation: a one-line summary and,
/// for object/array results, the pretty-printed value behind a detail toggle.
pub struct EvaluationView {
    pub summary: String,
    pub detail: String,
}

/// Formats an evaluation result. A missing rule and a rule without a value are
/// shown differently; a missing `rule_found` (older Worker) falls back to the
/// value alone. The actual value is shown as returned, never as false.
pub fn evaluation_view(lang: Lang, response: &Value) -> Option<EvaluationView> {
    let evaluation = response.pointer("/result/command_result")?;
    let version = evaluation
        .get("evaluated_version")
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let rule_found = evaluation.get("rule_found").and_then(Value::as_bool);
    let (shown, detail) = if rule_found == Some(false) {
        (
            text(lang, "settings.ruleNotFound").to_string(),
            String::new(),
        )
    } else {
        match evaluation.get("result") {
            None | Some(Value::Null) => (
                text(lang, "settings.noValueDefined").to_string(),
                String::new(),
            ),
            Some(Value::String(value)) => (value.clone(), String::new()),
            Some(value @ (Value::Object(_) | Value::Array(_))) => {
                let compact = value.to_string();
                let pretty =
                    serde_json::to_string_pretty(value).unwrap_or_else(|_| compact.clone());
                (compact, pretty)
            }
            Some(other) => (other.to_string(), String::new()),
        }
    };
    let summary = if version.is_empty() {
        format!("{}: {}", text(lang, "settings.evaluationResult"), shown)
    } else {
        format!(
            "{}: {} · {}: {}",
            text(lang, "settings.evaluationResult"),
            shown,
            text(lang, "settings.policyVersion"),
            version
        )
    };
    Some(EvaluationView { summary, detail })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_values_parse_as_json_types() {
        assert_eq!(parse_rule_value("true"), Value::Bool(true));
        assert_eq!(parse_rule_value("false"), Value::Bool(false));
        assert_eq!(parse_rule_value("42"), json!(42));
        assert_eq!(parse_rule_value("-7"), json!(-7));
        assert_eq!(parse_rule_value("1.5"), json!(1.5));
        assert_eq!(parse_rule_value("staff"), json!("staff"));
        assert_eq!(parse_rule_value(""), json!(""));
    }

    #[test]
    fn rule_value_text_keeps_text_and_json_forms() {
        assert_eq!(rule_value_text(&json!("staff")), "staff");
        assert_eq!(rule_value_text(&json!(true)), "true");
        assert_eq!(rule_value_text(&json!(12)), "12");
        assert_eq!(rule_value_text(&Value::Null), "null");
    }

    #[test]
    fn concurrency_codes_are_mapped_by_identifier_only() {
        assert_eq!(
            concurrency_code("LIFECYCLE_EPOCH_REQUIRED"),
            Some(ConcurrencyCode::LifecycleEpochRequired)
        );
        assert_eq!(
            concurrency_code("AUTHORITY_EPOCH_REQUIRED"),
            Some(ConcurrencyCode::AuthorityEpochRequired)
        );
        assert_eq!(
            concurrency_code("EPOCH_MISMATCH"),
            Some(ConcurrencyCode::EpochMismatch)
        );
        assert_eq!(
            concurrency_code("VERSION_CONFLICT"),
            Some(ConcurrencyCode::VersionConflict)
        );
        // Text that merely describes the condition must not be interpreted as a code.
        assert_eq!(concurrency_code("version conflict"), None);
        assert_eq!(concurrency_code("This record has changed."), None);
    }

    #[test]
    fn error_codes_read_both_worker_shapes() {
        let command =
            r#"{"success":false,"error":{"code":"EPOCH_MISMATCH","category":"CONCURRENCY"}}"#;
        let route = r#"{"error":"VERSION_CONFLICT","category":"CONCURRENCY"}"#;
        assert_eq!(error_code(command).as_deref(), Some("EPOCH_MISMATCH"));
        assert_eq!(error_code(route).as_deref(), Some("VERSION_CONFLICT"));
        assert_eq!(error_code("not json"), None);
        assert_eq!(error_code(r#"{"error":{}}"#), None);
        assert_eq!(error_code(r#"{"error":42}"#), None);
    }

    #[test]
    fn evaluation_never_reports_a_missing_rule_as_false() {
        let response = json!({
            "result": {
                "command_result": {
                    "rule_key": "messaging.enabled",
                    "rule_found": false,
                    "result": false,
                    "evaluated_version": 3
                }
            }
        });
        let view = evaluation_view(Lang::En, &response).expect("evaluation");
        assert!(view
            .summary
            .contains(text(Lang::En, "settings.ruleNotFound")));
        assert!(!view.summary.contains("false"));
        assert!(view.detail.is_empty());
    }

    #[test]
    fn evaluation_shows_value_and_version() {
        let response = json!({
            "result": {
                "command_result": {
                    "rule_key": "messaging.enabled",
                    "rule_found": true,
                    "result": true,
                    "evaluated_version": 4
                }
            }
        });
        let view = evaluation_view(Lang::En, &response).expect("evaluation");
        assert!(view.summary.contains("true"));
        assert!(view.summary.contains("4"));
        assert!(view.detail.is_empty());
    }

    #[test]
    fn structured_results_get_a_pretty_detail() {
        let response = json!({
            "result": {
                "command_result": {
                    "rule_found": true,
                    "result": {"limit": 5, "classes": ["staff"]},
                    "evaluated_version": 1
                }
            }
        });
        let view = evaluation_view(Lang::En, &response).expect("evaluation");
        assert!(view.summary.contains("\"limit\":5"));
        assert!(view.detail.contains("\n"));
        assert!(view.detail.contains("\"limit\": 5"));
    }

    #[test]
    fn a_missing_command_result_is_not_an_evaluation() {
        assert!(evaluation_view(Lang::En, &json!({"result": {}})).is_none());
    }
}
