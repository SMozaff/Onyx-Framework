use async_trait::async_trait;
use platform_kernel::{OrganizationId, Timestamp};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceClass {
    Commands,
    SyncOperations,
    AutomationExecutions,
}

impl ResourceClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Commands => "commands",
            Self::SyncOperations => "sync_operations",
            Self::AutomationExecutions => "automation_executions",
        }
    }
}

#[derive(Clone, Debug)]
pub struct RateLimitRequest {
    pub organization_id: OrganizationId,
    pub resource_class: ResourceClass,
    pub request_id: String,
    pub occurred_at: Timestamp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RateLimitPolicy {
    pub limit: u32,
    pub window_seconds: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitDecision {
    pub allowed: bool,
    pub limit: u32,
    pub remaining: u32,
    pub reset_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RateLimitError {
    #[error("rate limit exceeded; reset at {reset_at:?}")]
    Exceeded { reset_at: Timestamp },
    #[error("rate limiter infrastructure failed: {0}")]
    Infrastructure(String),
}

#[async_trait]
pub trait RateLimiter: Send + Sync {
    async fn check_and_record(
        &self,
        request: &RateLimitRequest,
    ) -> Result<RateLimitDecision, RateLimitError>;
}
