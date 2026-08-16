use async_trait::async_trait;
use platform_kernel::{ObjectId, Timestamp};
use sqlx::{Row, SqlitePool};
use worker_application::{ClaimedJob, JobFailureOutcome, JobId, JobQueue, JobQueueError, NewJob};

#[derive(Clone)]
pub struct SqliteJobQueue {
    pool: SqlitePool,
}

impl SqliteJobQueue {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn timestamp_ms(timestamp: Timestamp) -> i64 {
    (timestamp.0 / 1_000_000) as i64
}

fn from_ms(value: i64) -> Timestamp {
    Timestamp::from_nanos(value.max(0) as u64 * 1_000_000)
}

#[async_trait]
impl JobQueue for SqliteJobQueue {
    async fn enqueue(&self, job: NewJob) -> Result<JobId, JobQueueError> {
        let result = sqlx::query(
            "INSERT OR IGNORE INTO jobs (organization_id, job_type, payload, status, attempts, max_retries, next_attempt_at_ms, deduplication_key, created_at_ms, updated_at_ms) VALUES (?, ?, ?, 'pending', 0, ?, ?, ?, ?, ?)",
        )
        .bind(job.organization_id.0.to_vec())
        .bind(&job.job_type)
        .bind(job.payload.to_string())
        .bind(job.max_retries as i64)
        .bind(timestamp_ms(job.next_attempt_at))
        .bind(&job.deduplication_key)
        .bind(timestamp_ms(Timestamp::now()))
        .bind(timestamp_ms(Timestamp::now()))
        .execute(&self.pool)
        .await
        .map_err(|error| JobQueueError::Persistence(error.to_string()))?;
        if result.rows_affected() == 1 {
            return Ok(JobId(result.last_insert_rowid()));
        }
        let Some(key) = job.deduplication_key else {
            return Err(JobQueueError::Persistence(
                "job insertion was ignored without a deduplication key".to_string(),
            ));
        };
        let id: i64 = sqlx::query_scalar(
            "SELECT id FROM jobs WHERE deduplication_key = ? ORDER BY id DESC LIMIT 1",
        )
        .bind(key)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| JobQueueError::Persistence(error.to_string()))?;
        Ok(JobId(id))
    }

    async fn claim(
        &self,
        worker_id: &str,
        limit: usize,
        lease_seconds: u64,
    ) -> Result<Vec<ClaimedJob>, JobQueueError> {
        let now = Timestamp::now();
        let now_ms = timestamp_ms(now);
        let lease_expires_ms = now_ms.saturating_add((lease_seconds.saturating_mul(1_000)) as i64);
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| JobQueueError::Persistence(e.to_string()))?;
        let rows = sqlx::query(
            "SELECT id, organization_id, job_type, payload, attempts, max_retries FROM jobs WHERE status = 'pending' AND next_attempt_at_ms <= ? ORDER BY next_attempt_at_ms, id LIMIT ?",
        )
        .bind(now_ms)
        .bind(limit as i64)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| JobQueueError::Persistence(e.to_string()))?;

        let mut claimed = Vec::with_capacity(rows.len());
        for row in rows {
            let id: i64 = row
                .try_get("id")
                .map_err(|e| JobQueueError::Persistence(e.to_string()))?;
            let org: Vec<u8> = row
                .try_get("organization_id")
                .map_err(|e| JobQueueError::Persistence(e.to_string()))?;
            let org: [u8; 16] = org.try_into().map_err(|_| {
                JobQueueError::Serialization("organization_id is not 16 bytes".to_string())
            })?;
            let job_type: String = row
                .try_get("job_type")
                .map_err(|e| JobQueueError::Persistence(e.to_string()))?;
            let payload_raw: String = row
                .try_get("payload")
                .map_err(|e| JobQueueError::Persistence(e.to_string()))?;
            let payload = serde_json::from_str(&payload_raw)
                .map_err(|e| JobQueueError::Serialization(e.to_string()))?;
            let attempts: i64 = row
                .try_get("attempts")
                .map_err(|e| JobQueueError::Persistence(e.to_string()))?;
            let max_retries: i64 = row
                .try_get("max_retries")
                .map_err(|e| JobQueueError::Persistence(e.to_string()))?;
            let lease_token = uuid::Uuid::new_v4().to_string();
            let result = sqlx::query(
                "UPDATE jobs SET status = 'running', claimed_by = ?, lease_token = ?, lease_expires_at_ms = ?, attempts = attempts + 1, updated_at_ms = ? WHERE id = ? AND status = 'pending'",
            )
            .bind(worker_id)
            .bind(&lease_token)
            .bind(lease_expires_ms)
            .bind(now_ms)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| JobQueueError::Persistence(e.to_string()))?;
            if result.rows_affected() == 1 {
                claimed.push(ClaimedJob {
                    id: JobId(id),
                    organization_id: ObjectId(org),
                    job_type,
                    payload,
                    attempts: attempts.max(0) as u32 + 1,
                    max_retries: max_retries.max(0) as u32,
                    lease_token,
                    lease_expires_at: from_ms(lease_expires_ms),
                });
            }
        }
        tx.commit()
            .await
            .map_err(|e| JobQueueError::Persistence(e.to_string()))?;
        Ok(claimed)
    }

    async fn complete(&self, job_id: JobId, lease_token: &str) -> Result<(), JobQueueError> {
        let result = sqlx::query(
            "UPDATE jobs SET status = 'completed', claimed_by = NULL, lease_token = NULL, lease_expires_at_ms = NULL, updated_at_ms = ? WHERE id = ? AND status = 'running' AND lease_token = ?",
        )
        .bind(timestamp_ms(Timestamp::now()))
        .bind(job_id.0)
        .bind(lease_token)
        .execute(&self.pool)
        .await
        .map_err(|e| JobQueueError::Persistence(e.to_string()))?;
        if result.rows_affected() != 1 {
            return Err(JobQueueError::LeaseInvalid);
        }
        Ok(())
    }

    async fn fail(
        &self,
        job_id: JobId,
        lease_token: &str,
        error: &str,
        retry_at: Timestamp,
    ) -> Result<JobFailureOutcome, JobQueueError> {
        let row: Option<(i64, i64)> = sqlx::query_as(
            "SELECT attempts, max_retries FROM jobs WHERE id = ? AND status = 'running' AND lease_token = ?",
        )
        .bind(job_id.0)
        .bind(lease_token)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| JobQueueError::Persistence(e.to_string()))?;
        let Some((attempts, max_retries)) = row else {
            return Err(JobQueueError::LeaseInvalid);
        };
        let dead_lettered = attempts >= max_retries;
        let status = if dead_lettered {
            "dead_letter"
        } else {
            "pending"
        };
        let result = sqlx::query(
            "UPDATE jobs SET status = ?, claimed_by = NULL, lease_token = NULL, lease_expires_at_ms = NULL, last_error = ?, next_attempt_at_ms = ?, updated_at_ms = ? WHERE id = ? AND lease_token = ?",
        )
        .bind(status)
        .bind(error)
        .bind(timestamp_ms(retry_at))
        .bind(timestamp_ms(Timestamp::now()))
        .bind(job_id.0)
        .bind(lease_token)
        .execute(&self.pool)
        .await
        .map_err(|e| JobQueueError::Persistence(e.to_string()))?;
        if result.rows_affected() != 1 {
            return Err(JobQueueError::LeaseInvalid);
        }
        Ok(JobFailureOutcome {
            dead_lettered,
            attempts: attempts.max(0) as u32,
            next_attempt_at: (!dead_lettered).then_some(retry_at),
        })
    }

    async fn depth(&self) -> Result<u64, JobQueueError> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE status IN ('pending', 'running')")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| JobQueueError::Persistence(e.to_string()))?;
        Ok(count.max(0) as u64)
    }

    async fn recover_expired_leases(&self, now: Timestamp) -> Result<u64, JobQueueError> {
        let result = sqlx::query(
            "UPDATE jobs SET status = 'pending', claimed_by = NULL, lease_token = NULL, lease_expires_at_ms = NULL, updated_at_ms = ? WHERE status = 'running' AND lease_expires_at_ms <= ?",
        )
        .bind(timestamp_ms(now))
        .bind(timestamp_ms(now))
        .execute(&self.pool)
        .await
        .map_err(|e| JobQueueError::Persistence(e.to_string()))?;
        Ok(result.rows_affected())
    }
}
