use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{
    authenticate_headers, client_type::ClientType, issue_token, token_hash, unix_seconds,
    validate_token, ApiError, ApiState,
};

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
    /// Which kind of client is logging in. `Option<ClientType>`, not a
    /// bare `Option<String>` any more (H10/ONYX-MOB-00 v1.1): an
    /// unrecognized string is now rejected outright at the JSON
    /// boundary (`ClientType`'s `Deserialize` impl calls
    /// `unknown_variant` for anything outside its five closed
    /// variants), which the old loose-string field never did.
    ///
    /// # Reversed tradeoff — read this before "fixing" the `Option`
    /// This field's previous doc comment explained a real, deliberate
    /// decision: treat an absent `client_type` as `None`, which was
    /// never gated, "so any caller this project doesn't yet know about
    /// is not silently locked out." That reasoning is *not* overridden
    /// by accident here — the field stays `Option` for exactly that
    /// reason, confirmed still necessary by grepping this project's own
    /// tests: `test_harness.rs` (shared by every end-to-end journey)
    /// and a dozen other real internal test callers still omit
    /// `client_type` entirely. Requiring it outright would have broken
    /// all of them, not just a hypothetical unknown caller, so it was
    /// not done.
    ///
    /// What *does* change, and is a deliberate reversal: absence used
    /// to mean "never gated, full access" as an unexamined side effect
    /// of `Option<String>`'s design. It still means full access today
    /// (see `ClientType::default_on_absence`), but now as an explicit,
    /// documented compatibility policy rather than an accident of the
    /// type — because the new `MobileObserver` capability ceiling this
    /// field now also carries is a real security boundary, and a
    /// security boundary cannot be permissive-by-default the way the
    /// old backward-compatibility concern was. The two concerns are
    /// reconciled, not silently traded off against each other: the
    /// *shape* stays additive (`Option`, absence tolerated); the
    /// *ceiling* for whatever type is ultimately resolved is absolute.
    /// Every first-party client (`mobile`, `desktop-shell`,
    /// `admin-shell`, `web-ui`) already sends its own real value; only
    /// this project's own internal test/tooling callers rely on the
    /// absence default.
    pub client_type: Option<ClientType>,
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

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

/// `POST /api/auth/refresh` — redeems a still-valid `refresh` token for
/// a new `access` token, without the caller re-entering credentials.
///
/// # Provenance
/// Confirmed absent by reading this file and `routes/mod.rs` directly
/// (not assumed): `login` has always issued a 7-day refresh token
/// alongside the 1-hour access token, but until this route existed
/// nothing in this codebase ever redeemed one — every client's access
/// token simply expired after an hour with no way to renew it short of
/// a full password login again. Flagged explicitly as a real,
/// pre-existing gap when FFI-mode mobile's own session-persistence work
/// first needed one (see `DECISIONS.md`), and closed here as a
/// standalone fix any client can use, not something mobile-specific.
///
/// # Rotation
/// The refresh token presented is revoked (recorded in the shared
/// `token_revocation_store`, same mechanism `logout` already uses) and a
/// **new** refresh token is issued alongside the new access token, rather
/// than reissuing only the access token and leaving the same refresh
/// token valid indefinitely across every renewal. This bounds how long a
/// single refresh token value remains useful if it were ever leaked, at
/// the cost of the caller needing to persist the new refresh token from
/// every response — the same tradeoff most refresh-token implementations
/// make. Rotation is enforced by the same durable, shared store `logout`
/// uses (audit finding H-02, closed: revocation is now visible to every
/// API replica immediately, not just the one that handled this request,
/// and durable — see `security_application::ports::token_revocation`).
///
/// The user is re-fetched from the store (not trusted from the token's
/// own claims) and confirmed still active, mirroring `login`'s and
/// `authenticate_headers`'s own "never trust cached flags from a token
/// that could have been minted before a demotion/deactivation"
/// reasoning.
///
/// # Client classification survives refresh (H10/P1.5, MBP-012)
/// `claims.client_type` -- the presented refresh token's own bound
/// classification -- is threaded into both freshly minted tokens
/// unchanged, never re-derived from a request body (this endpoint takes
/// no `client_type` field at all). An observer session therefore cannot
/// be silently upgraded to unrestricted merely by rotating its token;
/// it stays `MobileObserver` for as long as it keeps refreshing.
pub async fn refresh(
    State(state): State<ApiState>,
    Json(payload): Json<RefreshRequest>,
) -> Result<Json<RefreshResponse>, ApiError> {
    let claims = validate_token(&state, &payload.refresh_token, "refresh").await?;

    let user = state
        .user_store
        .find_by_id(&claims.sub)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "user store lookup failed during token refresh");
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "USER_STORE_UNAVAILABLE",
                "INFRASTRUCTURE",
                "TRANSIENT",
                uuid::Uuid::new_v4().to_string(),
                json!({}),
            )
        })?
        .filter(|u| u.is_active)
        .ok_or_else(|| ApiError::unauthorized(uuid::Uuid::new_v4().to_string()))?;

    let access_token = issue_token(&state, &user, "access", 3600, claims.client_type)
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
    let refresh_token = issue_token(&state, &user, "refresh", 7 * 24 * 3600, claims.client_type)
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

    state
        .token_revocation_store
        .revoke_token(&token_hash(&payload.refresh_token), unix_seconds())
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "token revocation store failed during refresh rotation");
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "TOKEN_REVOCATION_STORE_UNAVAILABLE",
                "INFRASTRUCTURE",
                "TRANSIENT",
                uuid::Uuid::new_v4().to_string(),
                json!({}),
            )
        })?;

    Ok(Json(RefreshResponse {
        access_token,
        refresh_token,
        expires_in: 3600,
    }))
}

pub async fn logout(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(payload): Json<LogoutRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let auth = authenticate_headers(&state, &headers).await?;
    let _ = validate_token(&state, &payload.refresh_token, "refresh").await?;
    let now = unix_seconds();
    let revoke_err = |error: security_application::TokenRevocationError| {
        tracing::error!(error = %error, "token revocation store failed during logout");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "TOKEN_REVOCATION_STORE_UNAVAILABLE",
            "INFRASTRUCTURE",
            "TRANSIENT",
            uuid::Uuid::new_v4().to_string(),
            json!({}),
        )
    };
    state
        .token_revocation_store
        .revoke_token(&token_hash(&auth.token), now)
        .await
        .map_err(revoke_err)?;
    state
        .token_revocation_store
        .revoke_token(&token_hash(&payload.refresh_token), now)
        .await
        .map_err(revoke_err)?;
    Ok(Json(json!({"success":true})))
}
