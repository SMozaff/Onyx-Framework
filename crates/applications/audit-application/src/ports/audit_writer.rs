use async_trait::async_trait;
use platform_kernel::{CorrelationId, OperationId, OrganizationId, Timestamp, UserId};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{IntegrityDigest, IntegrityVerification};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditRecord {
    pub organization_id: OrganizationId,
    pub actor_id: Option<UserId>,
    pub operation_id: Option<OperationId>,
    pub correlation_id: Option<CorrelationId>,
    pub category: String,
    pub action: String,
    pub outcome: String,
    pub occurred_at: Timestamp,
    pub safe_details: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditWriteReceipt {
    pub sequence: i64,
    pub previous_hash: IntegrityDigest,
    pub current_hash: IntegrityDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuditError {
    #[error("audit serialization failed: {0}")]
    Serialization(String),
    #[error("audit persistence failed: {0}")]
    Persistence(String),
    #[error("audit hash chain is invalid at sequence {0}")]
    IntegrityViolation(i64),
}

#[async_trait]
pub trait AuditWriter: Send + Sync {
    async fn append(&self, record: &AuditRecord) -> Result<AuditWriteReceipt, AuditError>;
    async fn verify(
        &self,
        organization_id: OrganizationId,
    ) -> Result<IntegrityVerification, AuditError>;
}
