//! Web Push subscription registration for the PWA ObserverClient.
//! MIGRATION_PLAN Phase 1.2 addition.
//!
//! # What this is and isn't
//! These routes only *register* a client's Web Push subscription (the
//! endpoint + VAPID keys the browser handed back after the user granted
//! permission). There is no push *delivery* worker yet — nothing in this
//! repo reads these rows to send a notification (MIGRATION_PLAN Phase 3.2
//! is that worker; the table this route writes is what it will read).
//!
//! # Capability gate
//! Both routes are subject to `can_read_notifications`: registering a
//! subscription is how a read-class observer receives notifications, so
//! the read ceiling applies, not a mutation flag. `MobileObserver`
//! grants `can_read_notifications` (ONYX-MOB-01 §8), so observers may
//! register here; the check still exists so a future client class that
//! must not receive notifications is refused without changing routes.
//!
//! # Ownership
//! Subscriptions are owned by (user, organization) — the authenticated
//! session's own ids are used for the unique key (`user_id`,
//! `organization_id`, `endpoint`), so re-registering the same endpoint
//! updates its keys idempotently, and unregistration only ever deletes a
//! row belonging to the caller. No other tenant or user can observe or
//! remove another session's subscription.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{
    authenticate_headers, client_type::require_capability, ApiError, ApiState, ProjectionPool,
};

/// Request/response DTO for `POST /api/push/subscriptions`.
#[derive(Debug, Deserialize)]
pub struct RegisterSubscriptionRequest {
    /// The Web Push `PushSubscription` endpoint URL (https).
    pub endpoint: String,
    /// Base64url-encoded `p256dh` subscription key.
    pub p256dh: String,
    /// Base64url-encoded `auth` subscription secret.
    pub auth: String,
    /// Free-form client platform tag, e.g. `"pwa"`, `"android"`, `"ios"`.
    #[serde(default)]
    pub platform: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionDto {
    pub id: String,
    pub user_id: String,
    pub organization_id: String,
    pub endpoint: String,
    pub platform: String,
    pub created_at: u64,
}

/// `POST /api/push/subscriptions` — registers (or re-registers) the
/// caller's Web Push subscription. Returns `201` with the stored row's
/// id so a client can `DELETE` by it later. Idempotent per (user, org,
/// endpoint): re-registering the same endpoint updates its VAPID keys.
pub async fn register_subscription(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(payload): Json<RegisterSubscriptionRequest>,
) -> Result<(StatusCode, Json<SubscriptionDto>), ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let actor = authenticate_headers(&state, &headers).await?;
    require_capability(&actor, |c| c.can_read_notifications, "read_notifications")?;

    validate_subscription(&payload, correlation_id.clone())?;

    let now = super::unix_seconds() as i64;
    let id = uuid::Uuid::new_v4();
    let user_id = uuid::Uuid::parse_str(&actor.user_id)
        .map_err(|_| ApiError::unauthorized(correlation_id.clone()))?;
    let organization_id = uuid::Uuid::parse_str(&actor.organization_id)
        .map_err(|_| ApiError::unauthorized(correlation_id.clone()))?;

    let stored_id = upsert_subscription(
        &state.projection_pool,
        &id,
        &user_id,
        &organization_id,
        &payload,
        now,
        correlation_id.clone(),
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(SubscriptionDto {
            id: stored_id,
            user_id: actor.user_id,
            organization_id: actor.organization_id,
            endpoint: payload.endpoint,
            platform: payload.platform,
            created_at: now as u64,
        }),
    ))
}

/// `DELETE /api/push/subscriptions/:subscription_id` — removes the
/// caller's subscription with that id. `204` on success; `404` if no
/// subscription with that id belongs to this session.
pub async fn unregister_subscription(
    State(state): State<ApiState>,
    Path(subscription_id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let actor = authenticate_headers(&state, &headers).await?;
    require_capability(&actor, |c| c.can_read_notifications, "read_notifications")?;

    let id = uuid::Uuid::parse_str(&subscription_id).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_SUBSCRIPTION_ID",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation_id.clone(),
            json!({"message": "subscription id must be a UUID"}),
        )
    })?;
    let user_id = uuid::Uuid::parse_str(&actor.user_id)
        .map_err(|_| ApiError::unauthorized(correlation_id.clone()))?;
    let organization_id = uuid::Uuid::parse_str(&actor.organization_id)
        .map_err(|_| ApiError::unauthorized(correlation_id.clone()))?;

    match &state.projection_pool {
        ProjectionPool::Sqlite(pool) => {
            let result = sqlx::query(
                "DELETE FROM push_subscriptions \
                 WHERE id = ? AND user_id = ? AND organization_id = ?",
            )
            .bind(id.as_bytes().to_vec())
            .bind(user_id.as_bytes().to_vec())
            .bind(organization_id.as_bytes().to_vec())
            .execute(pool)
            .await
            .map_err(|error| store_error(error, correlation_id.clone()))?;
            if result.rows_affected() == 0 {
                Err(ApiError::new(
                    StatusCode::NOT_FOUND,
                    "SUBSCRIPTION_NOT_FOUND",
                    "VALIDATION",
                    "NON_RETRYABLE",
                    correlation_id,
                    json!({}),
                ))
            } else {
                Ok(StatusCode::NO_CONTENT)
            }
        }
        ProjectionPool::Postgres(pool) => {
            let result = sqlx::query(
                "DELETE FROM push_subscriptions \
                 WHERE id = $1 AND user_id = $2 AND organization_id = $3",
            )
            .bind(id)
            .bind(user_id)
            .bind(organization_id)
            .execute(pool)
            .await
            .map_err(|error| store_error(error, correlation_id.clone()))?;
            if result.rows_affected() == 0 {
                Err(ApiError::new(
                    StatusCode::NOT_FOUND,
                    "SUBSCRIPTION_NOT_FOUND",
                    "VALIDATION",
                    "NON_RETRYABLE",
                    correlation_id,
                    json!({}),
                ))
            } else {
                Ok(StatusCode::NO_CONTENT)
            }
        }
    }
}

/// Inserts or updates the subscription, returning the stable stored id
/// (unchanged on re-registration so `DELETE` by id keeps working).
async fn upsert_subscription(
    pool: &ProjectionPool,
    id: &uuid::Uuid,
    user_id: &uuid::Uuid,
    organization_id: &uuid::Uuid,
    payload: &RegisterSubscriptionRequest,
    created_at: i64,
    correlation_id: String,
) -> Result<String, ApiError> {
    match pool {
        ProjectionPool::Sqlite(pool) => {
            let (stored_id,): (Vec<u8>,) = sqlx::query_as(
                "INSERT INTO push_subscriptions \
                 (id, user_id, organization_id, endpoint, p256dh, auth, platform, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
                 ON CONFLICT (user_id, organization_id, endpoint) DO UPDATE SET \
                 p256dh = excluded.p256dh, auth = excluded.auth, \
                 platform = excluded.platform, created_at = excluded.created_at \
                 RETURNING id",
            )
            .bind(id.as_bytes().to_vec())
            .bind(user_id.as_bytes().to_vec())
            .bind(organization_id.as_bytes().to_vec())
            .bind(&payload.endpoint)
            .bind(&payload.p256dh)
            .bind(&payload.auth)
            .bind(&payload.platform)
            .bind(created_at)
            .fetch_one(pool)
            .await
            .map_err(|error| store_error(error, correlation_id.clone()))?;
            // UUIDs travel through sqlite TEXT columns as raw 16-byte
            // blobs (the repo-wide convention, see routes::relay), so the
            // stored key comes back as bytes, not a utf-8 string.
            uuid::Uuid::from_slice(&stored_id)
                .map(|uuid| uuid.to_string())
                .map_err(|_| {
                    ApiError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "PUSH_STORE_UNAVAILABLE",
                        "INFRASTRUCTURE",
                        "TRANSIENT",
                        uuid::Uuid::new_v4().to_string(),
                        json!({}),
                    )
                })
        }
        ProjectionPool::Postgres(pool) => {
            let (stored_id,): (uuid::Uuid,) = sqlx::query_as(
                "INSERT INTO push_subscriptions \
                 (id, user_id, organization_id, endpoint, p256dh, auth, platform, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
                 ON CONFLICT (user_id, organization_id, endpoint) DO UPDATE SET \
                 p256dh = excluded.p256dh, auth = excluded.auth, \
                 platform = excluded.platform, created_at = excluded.created_at \
                 RETURNING id",
            )
            .bind(*id)
            .bind(*user_id)
            .bind(*organization_id)
            .bind(&payload.endpoint)
            .bind(&payload.p256dh)
            .bind(&payload.auth)
            .bind(&payload.platform)
            .bind(created_at)
            .fetch_one(pool)
            .await
            .map_err(|error| store_error(error, correlation_id))?;
            Ok(stored_id.to_string())
        }
    }
}

/// Rejects an endpoint that isn't an https URL and VAPID keys that
/// aren't non-empty base64url — the shapes a browser `PushSubscription`
/// actually produces. The endpoint check is deliberately structural
/// (`https://` + a non-empty host) rather than a full URL parser: push
/// endpoints are opaque server URLs from the browser, and this route's
/// job is to refuse obviously-wrong input, not to interpret the URL.
fn validate_subscription(
    payload: &RegisterSubscriptionRequest,
    correlation_id: String,
) -> Result<(), ApiError> {
    let trimmed = payload.endpoint.trim();
    let host = trimmed
        .strip_prefix("https://")
        .map(|rest| rest.split('/').next().unwrap_or_default())
        .unwrap_or_default();
    if host.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_SUBSCRIPTION_ENDPOINT",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation_id.clone(),
            json!({"message": "endpoint must be an https URL"}),
        ));
    }
    for (label, value) in [("p256dh", &payload.p256dh), ("auth", &payload.auth)] {
        if value.is_empty()
            || base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, value)
                .is_err()
        {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "INVALID_SUBSCRIPTION_KEY",
                "VALIDATION",
                "NON_RETRYABLE",
                correlation_id.clone(),
                json!({"message": format!("{label} must be base64url")}),
            ));
        }
    }
    Ok(())
}

fn store_error(error: sqlx::Error, correlation_id: String) -> ApiError {
    tracing::error!(error = %error, "push subscription store failure");
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "PUSH_STORE_UNAVAILABLE",
        "INFRASTRUCTURE",
        "TRANSIENT",
        correlation_id,
        json!({}),
    )
}
