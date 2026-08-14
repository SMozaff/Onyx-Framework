use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use security_application::UserRecord;

use super::{authenticate_headers, issue_token, validate_token, ApiError, ApiState};

/// Single failure response for every authentication failure mode.
///
/// Audit finding H-01: unknown username, wrong password and disabled account
/// must be indistinguishable to the caller. Returning distinct codes or
/// messages would let an attacker enumerate valid usernames.
fn invalid_credentials() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "INVALID_CREDENTIALS",
        "AUTHORITY",
        "NON_RETRYABLE",
        uuid::Uuid::new_v4().to_string(),
        json!({"message":"Invalid username or password"}),
    )
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginUser {
    pub id: String,
    pub username: String,
    pub organization_id: String,
    /// Added 2026-08-14 for the Admin platform (`admin-shell`), so a
    /// client can show "you don't have admin access" immediately after
    /// login rather than only discovering it via a 403 on the first
    /// admin action. Additive — `web-ui`/`desktop-shell` simply ignore
    /// fields they don't read; this does not change the meaning of any
    /// existing field.
    pub is_admin: bool,
    /// See `is_admin` above — same rationale, for `UserClass`-gated
    /// capabilities (e.g. work-stats visibility, once a UI checks it
    /// client-side). Wire format matches `UserClass::as_str()`.
    pub class: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub user: LoginUser,
}

#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

pub async fn login(
    State(state): State<ApiState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    // Look the user up. A store failure is infrastructure (500), not a
    // credential failure, and must not be reported as one.
    let candidate = state
        .user_store
        .find_by_username(&payload.username)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "user store lookup failed during login");
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "USER_STORE_UNAVAILABLE",
                "INFRASTRUCTURE",
                "TRANSIENT",
                uuid::Uuid::new_v4().to_string(),
                json!({}),
            )
        })?;

    let user: UserRecord = match candidate {
        Some(user) if user.is_active => {
            // Verify unconditionally, then branch. Argon2id verification is
            // the dominant cost of this handler, so it must run on every
            // path that reaches a real user.
            let matches = state
                .password_hasher
                .verify(&payload.password, &user.password_hash)
                .map_err(|error| {
                    // A malformed stored hash is a data-integrity fault, not a
                    // wrong password. Surface it as 500 so it gets fixed
                    // rather than being hidden behind a 401.
                    tracing::error!(error = %error, user_id = %user.user_id, "stored password hash is malformed");
                    ApiError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "CREDENTIAL_STORE_CORRUPT",
                        "INFRASTRUCTURE",
                        "NON_RETRYABLE",
                        uuid::Uuid::new_v4().to_string(),
                        json!({}),
                    )
                })?;
            if !matches {
                return Err(invalid_credentials());
            }
            user
        }
        // Unknown OR disabled user: burn equivalent CPU against a dummy hash
        // so response latency does not reveal which usernames exist.
        _ => {
            state.password_hasher.verify_dummy(&payload.password);
            return Err(invalid_credentials());
        }
    };

    let access_token = issue_token(&state, &user, "access", 3600)
        .await
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "TOKEN_ISSUANCE_FAILED",
                "INFRASTRUCTURE",
                "TRANSIENT",
                uuid::Uuid::new_v4().to_string(),
                json!({}),
            )
        })?;
    let refresh_token = issue_token(&state, &user, "refresh", 7 * 24 * 3600)
        .await
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "TOKEN_ISSUANCE_FAILED",
                "INFRASTRUCTURE",
                "TRANSIENT",
                uuid::Uuid::new_v4().to_string(),
                json!({}),
            )
        })?;
    Ok(Json(LoginResponse {
        access_token,
        refresh_token,
        expires_in: 3600,
        user: LoginUser {
            id: user.user_id,
            username: user.username,
            organization_id: user.organization_id,
            is_admin: user.is_admin,
            class: user.class.map(|c| c.as_str().to_string()),
        },
    }))
}

pub async fn logout(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(payload): Json<LogoutRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let auth = authenticate_headers(&state, &headers).await?;
    let _ = validate_token(&state, &payload.refresh_token, "refresh").await?;
    let mut revoked = state.revoked_tokens.write().await;
    revoked.insert(auth.token);
    revoked.insert(payload.refresh_token);
    Ok(Json(json!({"success":true})))
}
