//! Cloud upload of pending outbox rows (offline-outbox-cloud-relay, step 3).
//!
//! A row is only "done" once the Worker has stored it. This module defines
//! the upload boundary used by `SyncAgent`:
//!
//! * `CloudOutboxUploader` — the port. The pump depends on this trait only.
//! * `HttpCloudOutboxUploader` — posts a batch to the Worker endpoint
//!   `POST {base}/api/sync/outbox` with a bearer token.
//!
//! Wire contract (Worker side is implemented in step 5):
//!
//! Request body:
//! ```json
//! { "items": [ { "outbox_id": 1, "event_id": "<uuid>", "event_type": "...",
//!                "aggregate_id": "...", "organization_id": "<uuid>",
//!                "occurred_at_millis": 0, "payload": { } } ] }
//! ```
//! Response body (200):
//! ```json
//! { "acks": [ { "outbox_id": 1, "status": "stored" } ,
//!             { "outbox_id": 2, "status": "rejected", "retryable": false,
//!               "reason": "..." } ] }
//! ```
//! Storing is keyed by `outbox_id` per device, so a retry after a lost
//! response re-stores nothing new (idempotent).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UploadItem {
    pub outbox_id: u64,
    pub event_id: String,
    pub event_type: String,
    pub aggregate_id: String,
    pub organization_id: String,
    pub occurred_at_millis: i64,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UploadAck {
    /// The Worker stored the row. Safe to mark done.
    Stored { outbox_id: u64 },
    /// The Worker refused the row. `retryable` decides between retry and
    /// dead-letter.
    Rejected {
        outbox_id: u64,
        retryable: bool,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum UploadError {
    /// No response: offline, DNS, TLS, timeout. Whole batch stays pending.
    #[error("cloud unreachable: {0}")]
    Unreachable(String),
    /// 401/403: credentials missing or expired. Whole batch stays pending.
    #[error("cloud rejected credentials (HTTP {0})")]
    Unauthorized(u16),
    /// Any other non-success status, or an unreadable body.
    #[error("cloud upload failed: {0}")]
    Server(String),
}

#[async_trait]
pub trait CloudOutboxUploader: Send + Sync {
    /// Upload a batch. Returns one ack per item that the server answered for.
    /// Items with no ack stay pending.
    async fn upload(&self, items: &[UploadItem]) -> Result<Vec<UploadAck>, UploadError>;
}

#[derive(Serialize)]
struct RequestBody<'a> {
    items: &'a [UploadItem],
}

#[derive(Deserialize)]
struct ResponseBody {
    acks: Vec<AckWire>,
}

#[derive(Deserialize)]
struct AckWire {
    outbox_id: u64,
    status: String,
    #[serde(default)]
    retryable: Option<bool>,
    #[serde(default)]
    reason: Option<String>,
}

/// Map the wire acks to `UploadAck`. Unknown statuses are treated as
/// retryable rejections so they never count as stored.
pub fn parse_acks(body: &[u8]) -> Result<Vec<UploadAck>, UploadError> {
    let parsed: ResponseBody =
        serde_json::from_slice(body).map_err(|e| UploadError::Server(e.to_string()))?;
    Ok(parsed
        .acks
        .into_iter()
        .map(|a| match a.status.as_str() {
            "stored" => UploadAck::Stored {
                outbox_id: a.outbox_id,
            },
            "rejected" => UploadAck::Rejected {
                outbox_id: a.outbox_id,
                retryable: a.retryable.unwrap_or(true),
                reason: a.reason.unwrap_or_default(),
            },
            other => UploadAck::Rejected {
                outbox_id: a.outbox_id,
                retryable: true,
                reason: format!("unknown ack status: {other}"),
            },
        })
        .collect())
}

/// HTTP uploader. `endpoint` is the full URL, e.g.
/// `https://<worker>/api/sync/outbox`. `bearer_token` is fetched per batch
/// so a refreshed token is used.
pub struct HttpCloudOutboxUploader {
    client: reqwest::Client,
    endpoint: String,
    auth: std::sync::Arc<dyn sync_transport::placeholder_types::AuthorityProvider>,
}

impl HttpCloudOutboxUploader {
    pub fn new(
        endpoint: String,
        auth: std::sync::Arc<dyn sync_transport::placeholder_types::AuthorityProvider>,
        timeout: std::time::Duration,
    ) -> Result<Self, UploadError> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| UploadError::Unreachable(e.to_string()))?;
        Ok(Self {
            client,
            endpoint,
            auth,
        })
    }
}

#[async_trait]
impl CloudOutboxUploader for HttpCloudOutboxUploader {
    async fn upload(&self, items: &[UploadItem]) -> Result<Vec<UploadAck>, UploadError> {
        let token = self
            .auth
            .bearer_token()
            .await
            .map_err(|_| UploadError::Unauthorized(401))?;
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(token)
            .json(&RequestBody { items })
            .send()
            .await
            .map_err(|e| UploadError::Unreachable(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(UploadError::Unauthorized(status.as_u16()));
        }
        if !status.is_success() {
            return Err(UploadError::Server(format!("HTTP {}", status.as_u16())));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| UploadError::Unreachable(e.to_string()))?;
        parse_acks(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stored_and_rejected_acks() {
        let body = br#"{"acks":[
            {"outbox_id":1,"status":"stored"},
            {"outbox_id":2,"status":"rejected","retryable":false,"reason":"bad schema"},
            {"outbox_id":3,"status":"rejected"}
        ]}"#;
        let acks = parse_acks(body).unwrap();
        assert_eq!(acks[0], UploadAck::Stored { outbox_id: 1 });
        assert_eq!(
            acks[1],
            UploadAck::Rejected {
                outbox_id: 2,
                retryable: false,
                reason: "bad schema".into()
            }
        );
        assert_eq!(
            acks[2],
            UploadAck::Rejected {
                outbox_id: 3,
                retryable: true,
                reason: String::new()
            }
        );
    }

    #[test]
    fn unknown_status_is_never_stored() {
        let acks = parse_acks(br#"{"acks":[{"outbox_id":9,"status":"maybe"}]}"#).unwrap();
        assert!(matches!(
            acks[0],
            UploadAck::Rejected {
                outbox_id: 9,
                retryable: true,
                ..
            }
        ));
    }

    #[test]
    fn unreadable_body_is_a_server_error() {
        assert!(matches!(
            parse_acks(b"not json"),
            Err(UploadError::Server(_))
        ));
    }
}
