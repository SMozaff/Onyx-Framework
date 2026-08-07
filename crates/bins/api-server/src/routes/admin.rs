//! First-run bootstrap and admin-only user management.
//!
//! # Provenance
//! Audit finding **H-01**. Owner decision: user management is exposed through
//! admin-only API endpoints.
//!
//! # The bootstrap problem
//! Admin-only endpoints cannot authenticate against an empty `users` table —
//! there is no admin yet, so no one could ever create the first one. This
//! module resolves that with a **one-time bootstrap token**:
//!
//! * `POST /api/admin/bootstrap` is the only unauthenticated write endpoint.
//! * It is refused unless the store is **completely empty**. The moment the
//!   first user exists it is permanently closed, so it cannot be used to add a
//!   back-door admin to a live system.
//! * It requires `ONYX_BOOTSTRAP_TOKEN`, compared in constant time. If that
//!   variable is unset the endpoint is disabled outright rather than
//!   defaulting to open — this fails **closed**, unlike the `ONYX_TEST_MODE`
//!   pattern flagged as audit finding M-03.
//!
//! The result is that the token is useless the instant it is used once, and
//! useless on any system that already has users.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use security_adapter::constant_time_eq;
use security_application::{NewUser, UserRecord, UserStoreError};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{authenticate_headers, ApiError, ApiState, ORGANIZATION_ID};

/// Header carrying the one-time bootstrap token.
pub const BOOTSTRAP_TOKEN_HEADER: &str = "x-onyx-bootstrap-token";
/// Environment variable holding the expected bootstrap token.
pub const BOOTSTRAP_TOKEN_ENV: &str = "ONYX_BOOTSTRAP_TOKEN";

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub is_admin: bool,
    /// Defaults to the deployment's default tenant when omitted.
    #[serde(default)]
    pub organization_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetPasswordRequest {
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct UserDto {
    pub id: String,
    pub username: String,
    pub organization_id: String,
    pub is_admin: bool,
    pub is_active: bool,
}

impl From<UserRecord> for UserDto {
    fn from(user: UserRecord) -> Self {
        // Note the absence of `password_hash`: the hash must never cross the
        // API boundary, so the DTO simply has no field for it.
        Self {
            id: user.user_id,
            username: user.username,
            organization_id: user.organization_id,
            is_admin: user.is_admin,
            is_active: user.is_active,
        }
    }
}

fn correlation() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn store_error(error: UserStoreError) -> ApiError {
    match error {
        UserStoreError::DuplicateUsername => ApiError::new(
            StatusCode::CONFLICT,
            "USERNAME_TAKEN",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"That username already exists"}),
        ),
        UserStoreError::NotFound => ApiError::new(
            StatusCode::NOT_FOUND,
            "USER_NOT_FOUND",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation(),
            json!({}),
        ),
        UserStoreError::Infrastructure(message) => {
            tracing::error!(error = %message, "user store failure");
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "USER_STORE_UNAVAILABLE",
                "INFRASTRUCTURE",
                "TRANSIENT",
                correlation(),
                json!({}),
            )
        }
    }
}

fn password_error(error: security_adapter::PasswordError) -> ApiError {
    match error {
        security_adapter::PasswordError::Policy(message) => ApiError::new(
            StatusCode::BAD_REQUEST,
            "WEAK_PASSWORD",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation(),
            // The policy text is safe to return: it describes the rule, not
            // anything about the submitted secret.
            json!({ "message": message }),
        ),
        other => {
            tracing::error!(error = %other, "password hashing failure");
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "PASSWORD_HASHING_FAILED",
                "INFRASTRUCTURE",
                "TRANSIENT",
                correlation(),
                json!({}),
            )
        }
    }
}

/// Authenticates the caller and confirms they are an admin.
///
/// The `is_admin` flag is re-read from the store rather than trusted from the
/// token: a token minted before an admin was demoted must not retain admin
/// power until it expires.
async fn require_admin(state: &ApiState, headers: &HeaderMap) -> Result<UserRecord, ApiError> {
    let auth = authenticate_headers(state, headers).await?;
    let user = state
        .user_store
        .find_by_id(&auth.user_id)
        .await
        .map_err(store_error)?
        .ok_or_else(|| ApiError::unauthorized(correlation()))?;
    if !user.is_active || !user.is_admin {
        // 403, not 404: the caller is authenticated, just not permitted.
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "ADMIN_REQUIRED",
            "AUTHORITY",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"Administrator privileges are required"}),
        ));
    }
    Ok(user)
}

/// `POST /api/admin/bootstrap` — creates the first admin. See module docs.
pub async fn bootstrap(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserDto>), ApiError> {
    // 1. The endpoint is disabled unless a token is configured (fail closed).
    let expected = std::env::var(BOOTSTRAP_TOKEN_ENV).ok().filter(|token| !token.is_empty());
    let Some(expected) = expected else {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "BOOTSTRAP_DISABLED",
            "AUTHORITY",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"Bootstrap is not enabled"}),
        ));
    };

    // 2. Constant-time token comparison.
    let presented = headers
        .get(BOOTSTRAP_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if !constant_time_eq(presented, &expected) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "BOOTSTRAP_TOKEN_INVALID",
            "AUTHORITY",
            "NON_RETRYABLE",
            correlation(),
            json!({}),
        ));
    }

    // 3. Refuse once ANY user exists. This is what makes the token one-time:
    //    a second call can never succeed, even with the correct token.
    if state.user_store.count().await.map_err(store_error)? > 0 {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "BOOTSTRAP_ALREADY_COMPLETED",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"Bootstrap has already been completed"}),
        ));
    }

    let created = create_user_record(&state, payload, true).await?;
    tracing::warn!(
        user_id = %created.id,
        username = %created.username,
        "bootstrap admin created; ONYX_BOOTSTRAP_TOKEN should now be removed from the environment"
    );
    Ok((StatusCode::CREATED, Json(created)))
}

/// `POST /api/admin/users` — admin-only user creation.
pub async fn create_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserDto>), ApiError> {
    require_admin(&state, &headers).await?;
    let is_admin = payload.is_admin;
    let created = create_user_record(&state, payload, is_admin).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// Shared creation path for bootstrap and admin creation, so password policy,
/// hashing and duplicate handling cannot diverge between the two.
async fn create_user_record(
    state: &ApiState,
    payload: CreateUserRequest,
    is_admin: bool,
) -> Result<UserDto, ApiError> {
    let username = payload.username.trim().to_string();
    if username.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_USERNAME",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"Username must not be empty"}),
        ));
    }
    let password_hash = state
        .password_hasher
        .hash(&payload.password)
        .map_err(password_error)?;
    let organization_id = payload
        .organization_id
        .unwrap_or_else(|| ORGANIZATION_ID.to_string());

    let record = state
        .user_store
        .create(NewUser {
            user_id: uuid::Uuid::new_v4().to_string(),
            username,
            organization_id,
            password_hash,
            is_admin,
        })
        .await
        .map_err(store_error)?;
    Ok(record.into())
}

/// `GET /api/admin/users` — admin-only listing.
pub async fn list_users(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<UserDto>>, ApiError> {
    require_admin(&state, &headers).await?;
    let users = state.user_store.list().await.map_err(store_error)?;
    Ok(Json(users.into_iter().map(UserDto::from).collect()))
}

/// `POST /api/admin/users/:id/deactivate`
pub async fn deactivate_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    // Guard against an admin locking the system out of itself.
    if admin.user_id == user_id {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "CANNOT_DEACTIVATE_SELF",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"An administrator cannot deactivate their own account"}),
        ));
    }
    state
        .user_store
        .set_active(&user_id, false)
        .await
        .map_err(store_error)?;
    Ok(Json(json!({"success": true})))
}

/// `POST /api/admin/users/:id/activate`
pub async fn activate_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state, &headers).await?;
    state
        .user_store
        .set_active(&user_id, true)
        .await
        .map_err(store_error)?;
    Ok(Json(json!({"success": true})))
}

/// `POST /api/admin/users/:id/password` — admin-driven password reset.
pub async fn set_user_password(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(payload): Json<SetPasswordRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state, &headers).await?;
    let hash = state
        .password_hasher
        .hash(&payload.password)
        .map_err(password_error)?;
    state
        .user_store
        .set_password_hash(&user_id, &hash)
        .await
        .map_err(store_error)?;
    // NOTE: existing tokens for this user remain valid until they expire.
    // Durable, cross-replica revocation is audit finding H-02 and is tracked
    // separately; the in-memory revocation set cannot express "revoke all
    // tokens for a user" across pods.
    Ok(Json(json!({"success": true})))
}
