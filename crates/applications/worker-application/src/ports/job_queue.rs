//! Durable background-job queue port used by the worker composition root.

use async_trait::async_trait;
use platform_kernel::{OrganizationId, Timestamp};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub i64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    DeadLetter,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewJob {
    pub organization_id: OrganizationId,
    pub job_type: String,
    pub payload: Value,
    pub next_attempt_at: Timestamp,
    pub max_retries: u32,
    pub deduplication_key: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClaimedJob {
    pub id: JobId,
    pub organization_id: OrganizationId,
    pub job_type: String,
    pub payload: Value,
    pub attempts: u32,
    pub max_retries: u32,
    pub lease_token: String,
    pub lease_expires_at: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobFailureOutcome {
    pub dead_lettered: bool,
    pub attempts: u32,
    pub next_attempt_at: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JobQueueError {
    #[error("job queue is empty")]
    Empty,
    #[error("job lease is invalid or expired")]
    LeaseInvalid,
    #[error("job queue persistence failed: {0}")]
    Persistence(String),
    #[error("job queue serialization failed: {0}")]
    Serialization(String),
}

#[async_trait]
pub trait JobQueue: Send + Sync {
    async fn enqueue(&self, job: NewJob) -> Result<JobId, JobQueueError>;
    async fn claim(
        &self,
        worker_id: &str,
        limit: usize,
        lease_seconds: u64,
    ) -> Result<Vec<ClaimedJob>, JobQueueError>;
    async fn complete(&self, job_id: JobId, lease_token: &str) -> Result<(), JobQueueError>;
    async fn fail(
        &self,
        job_id: JobId,
        lease_token: &str,
        error: &str,
        retry_at: Timestamp,
    ) -> Result<JobFailureOutcome, JobQueueError>;
    async fn depth(&self) -> Result<u64, JobQueueError>;
    async fn recover_expired_leases(&self, now: Timestamp) -> Result<u64, JobQueueError>;
}
