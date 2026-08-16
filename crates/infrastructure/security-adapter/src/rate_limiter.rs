use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use async_trait::async_trait;
use platform_kernel::Timestamp;
use security_application::{
    RateLimitDecision, RateLimitError, RateLimitPolicy, RateLimitRequest, RateLimiter,
    ResourceClass,
};
use sqlx::PgPool;
use tokio::sync::Mutex;

fn default_policies() -> HashMap<ResourceClass, RateLimitPolicy> {
    fn configured(name: &str, fallback: u32) -> u32 {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(fallback)
    }
    HashMap::from([
        (
            ResourceClass::Commands,
            RateLimitPolicy {
                limit: configured("ONYX_COMMAND_RATE_LIMIT", 120),
                window_seconds: 60,
            },
        ),
        (
            ResourceClass::SyncOperations,
            RateLimitPolicy {
                limit: configured("ONYX_SYNC_RATE_LIMIT", 600),
                window_seconds: 60,
            },
        ),
        (
            ResourceClass::AutomationExecutions,
            RateLimitPolicy {
                limit: configured("ONYX_AUTOMATION_RATE_LIMIT", 300),
                window_seconds: 60,
            },
        ),
    ])
}

fn organization_string(request: &RateLimitRequest) -> String {
    uuid::Uuid::from_bytes(request.organization_id.0).to_string()
}

#[derive(Clone)]
pub struct PostgresSlidingWindowRateLimiter {
    pool: PgPool,
    policies: Arc<HashMap<ResourceClass, RateLimitPolicy>>,
}

impl PostgresSlidingWindowRateLimiter {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            policies: Arc::new(default_policies()),
        }
    }

    pub fn with_policies(pool: PgPool, policies: HashMap<ResourceClass, RateLimitPolicy>) -> Self {
        Self {
            pool,
            policies: Arc::new(policies),
        }
    }

    fn policy(&self, resource_class: ResourceClass) -> RateLimitPolicy {
        self.policies
            .get(&resource_class)
            .copied()
            .unwrap_or(RateLimitPolicy {
                limit: 100,
                window_seconds: 60,
            })
    }
}

#[async_trait]
impl RateLimiter for PostgresSlidingWindowRateLimiter {
    async fn check_and_record(
        &self,
        request: &RateLimitRequest,
    ) -> Result<RateLimitDecision, RateLimitError> {
        let policy = self.policy(request.resource_class);
        let now_ms = (request.occurred_at.0 / 1_000_000) as i64;
        let window_ms = (policy.window_seconds.saturating_mul(1_000)) as i64;
        let window_start = now_ms.saturating_sub(window_ms);
        let organization_id = organization_string(request);
        let resource = request.resource_class.as_str();
        let lock_key = format!("{organization_id}:{resource}");

        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| RateLimitError::Infrastructure(error.to_string()))?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(&lock_key)
            .execute(&mut *transaction)
            .await
            .map_err(|error| RateLimitError::Infrastructure(error.to_string()))?;
        sqlx::query(
            "DELETE FROM rate_limit_events WHERE organization_id = $1::uuid AND resource_class = $2 AND occurred_at_ms < $3",
        )
        .bind(&organization_id)
        .bind(resource)
        .bind(window_start)
        .execute(&mut *transaction)
        .await
        .map_err(|error| RateLimitError::Infrastructure(error.to_string()))?;

        let row: (i64, Option<i64>) = sqlx::query_as(
            "SELECT COUNT(*)::bigint, MIN(occurred_at_ms) FROM rate_limit_events WHERE organization_id = $1::uuid AND resource_class = $2 AND occurred_at_ms >= $3",
        )
        .bind(&organization_id)
        .bind(resource)
        .bind(window_start)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| RateLimitError::Infrastructure(error.to_string()))?;

        let count = row.0.max(0) as u32;
        let reset_ms = row.1.unwrap_or(now_ms).saturating_add(window_ms);
        let reset_at = Timestamp::from_nanos((reset_ms.max(0) as u64).saturating_mul(1_000_000));
        if count >= policy.limit {
            transaction
                .commit()
                .await
                .map_err(|error| RateLimitError::Infrastructure(error.to_string()))?;
            return Ok(RateLimitDecision {
                allowed: false,
                limit: policy.limit,
                remaining: 0,
                reset_at,
            });
        }

        sqlx::query(
            "INSERT INTO rate_limit_events (request_id, organization_id, resource_class, occurred_at_ms) VALUES ($1, $2::uuid, $3, $4) ON CONFLICT (request_id) DO NOTHING",
        )
        .bind(&request.request_id)
        .bind(&organization_id)
        .bind(resource)
        .bind(now_ms)
        .execute(&mut *transaction)
        .await
        .map_err(|error| RateLimitError::Infrastructure(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| RateLimitError::Infrastructure(error.to_string()))?;

        Ok(RateLimitDecision {
            allowed: true,
            limit: policy.limit,
            remaining: policy.limit.saturating_sub(count.saturating_add(1)),
            reset_at: Timestamp::from_nanos(
                (now_ms.saturating_add(window_ms).max(0) as u64).saturating_mul(1_000_000),
            ),
        })
    }
}

/// Rate-limit event timestamps keyed by (subject, resource class).
///
/// Named per clippy's `type_complexity` lint (reproduced in CI — session 6
/// audit finding #24): the bare nested type triggers the lint at every use
/// site, and the alias documents what the tuple key represents.
type RateLimitEventLog = Arc<Mutex<HashMap<(String, ResourceClass), VecDeque<u64>>>>;

/// Deterministic development/test adapter. Production composition MUST use
/// `PostgresSlidingWindowRateLimiter` per ruling R8.
#[derive(Clone)]
pub struct InMemorySlidingWindowRateLimiter {
    policies: Arc<HashMap<ResourceClass, RateLimitPolicy>>,
    events: RateLimitEventLog,
}

impl Default for InMemorySlidingWindowRateLimiter {
    fn default() -> Self {
        Self {
            policies: Arc::new(default_policies()),
            events: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl InMemorySlidingWindowRateLimiter {
    pub fn with_policies(policies: HashMap<ResourceClass, RateLimitPolicy>) -> Self {
        Self {
            policies: Arc::new(policies),
            events: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl RateLimiter for InMemorySlidingWindowRateLimiter {
    async fn check_and_record(
        &self,
        request: &RateLimitRequest,
    ) -> Result<RateLimitDecision, RateLimitError> {
        let policy = self
            .policies
            .get(&request.resource_class)
            .copied()
            .unwrap_or(RateLimitPolicy {
                limit: 100,
                window_seconds: 60,
            });
        let key = (organization_string(request), request.resource_class);
        let now = request.occurred_at.0;
        let window = policy.window_seconds.saturating_mul(1_000_000_000);
        let cutoff = now.saturating_sub(window);
        let mut all_events = self.events.lock().await;
        let events = all_events.entry(key).or_default();
        while events
            .front()
            .copied()
            .map(|at| at < cutoff)
            .unwrap_or(false)
        {
            events.pop_front();
        }
        let reset_at = Timestamp::from_nanos(
            events
                .front()
                .copied()
                .unwrap_or(now)
                .saturating_add(window),
        );
        if events.len() as u32 >= policy.limit {
            return Ok(RateLimitDecision {
                allowed: false,
                limit: policy.limit,
                remaining: 0,
                reset_at,
            });
        }
        events.push_back(now);
        Ok(RateLimitDecision {
            allowed: true,
            limit: policy.limit,
            remaining: policy.limit.saturating_sub(events.len() as u32),
            reset_at: Timestamp::from_nanos(now.saturating_add(window)),
        })
    }
}
