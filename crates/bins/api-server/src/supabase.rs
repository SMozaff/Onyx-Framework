//! Supabase Auth identity exchange for the designated ONYX All-Father account.
//!
//! Supabase verifies the bearer access token through its authenticated-user
//! endpoint. ONYX still owns the principal, role, tenant, and authorization.
//! Only the confirmed, designated email may exchange a Supabase identity for
//! an ONYX session.

use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{auth::LoginUser, client_type::ClientType, issue_token, ApiError, ApiState};

pub const ALLFATHER_USERNAME: &str = "allfather";
pub const ALLFATHER_EMAIL: &str = "so.muzaff@gmail.com";

#[derive(Clone)]
pub struct SupabaseAuth {
    url: String,
    publishable_key: String,
    allfather_user_id: String,
    client: Client,
}

#[derive(Debug, Deserialize)]
struct SupabaseUser {
    id: String,
    email: Option<String>,
    email_confirmed_at: Option<String>,
    confirmed_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SupabaseIdentityResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub user: LoginUser,
}

impl SupabaseAuth {
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        let url = std::env::var("SUPABASE_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_owned())
            .filter(|value| !value.is_empty());
        let publishable_key = std::env::var("SUPABASE_PUBLISHABLE_KEY")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let allfather_user_id = std::env::var("ONYX_ALLFATHER_SUPABASE_USER_ID")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());

        match (url, publishable_key, allfather_user_id) {
            (None, None, None) => Ok(None),
            (Some(url), Some(publishable_key), Some(allfather_user_id)) => {
                if !url.starts_with("https://") {
                    anyhow::bail!("SUPABASE_URL must use HTTPS");
                }
                Ok(Some(Self {
                    url,
                    publishable_key,
                    allfather_user_id,
                    client: Client::new(),
                }))
            }
            _ => anyhow::bail!(
                "SUPABASE_URL, SUPABASE_PUBLISHABLE_KEY, and ONYX_ALLFATHER_SUPABASE_USER_ID must be configured together"
            ),
        }
    }

    async fn verified_user(&self, headers: &HeaderMap) -> Result<SupabaseUser, ApiError> {
        let value = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| auth_error("SUPABASE_AUTH_REQUIRED"))?;
        let token = value
            .strip_prefix("Bearer ")
            .or_else(|| value.strip_prefix("bearer "))
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| auth_error("SUPABASE_AUTH_REQUIRED"))?;

        let response = self
            .client
            .get(format!("{}/auth/v1/user", self.url))
            .header("apikey", &self.publishable_key)
            .bearer_auth(token)
            .header("accept", "application/json")
            .send()
            .await
            .map_err(|_| auth_error("SUPABASE_AUTH_UNAVAILABLE"))?;

        if response.status() == StatusCode::UNAUTHORIZED
            || response.status() == StatusCode::FORBIDDEN
        {
            return Err(auth_error("INVALID_SUPABASE_TOKEN"));
        }
        if !response.status().is_success() {
            return Err(auth_error("SUPABASE_AUTH_UNAVAILABLE"));
        }

        response
            .json::<SupabaseUser>()
            .await
            .map_err(|_| auth_error("SUPABASE_USER_RESPONSE_INVALID"))
    }
}

fn is_designated_identity(identity: &SupabaseUser, expected_subject: &str) -> bool {
    let email = identity.email.as_deref().unwrap_or("").trim().to_ascii_lowercase();
    let confirmed = identity
        .email_confirmed_at
        .as_deref()
        .or(identity.confirmed_at.as_deref())
        .is_some_and(|value| !value.trim().is_empty());
    confirmed
        && email == ALLFATHER_EMAIL
        && !identity.id.trim().is_empty()
        && identity.id == expected_subject
}

fn auth_error(code: &str) -> ApiError {
    let (status, category, retryability) = match code {
        "SUPABASE_AUTH_UNAVAILABLE" | "SUPABASE_USER_RESPONSE_INVALID"
        | "SUPABASE_AUTH_NOT_CONFIGURED" | "USER_STORE_UNAVAILABLE"
        | "TOKEN_ISSUANCE_FAILED" => (
            StatusCode::SERVICE_UNAVAILABLE,
            "INFRASTRUCTURE",
            "TRANSIENT",
        ),
        _ => (StatusCode::UNAUTHORIZED, "AUTHORITY", "NON_RETRYABLE"),
    };
    ApiError::new(
        status,
        code,
        category,
        retryability,
        uuid::Uuid::new_v4().to_string(),
        json!({}),
    )
}

pub async fn login(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<SupabaseIdentityResponse>, ApiError> {
    let auth = state
        .supabase_auth
        .as_ref()
        .ok_or_else(|| auth_error("SUPABASE_AUTH_NOT_CONFIGURED"))?;
    let identity = auth.verified_user(&headers).await?;
    if !is_designated_identity(&identity, &auth.allfather_user_id) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "SUPABASE_IDENTITY_NOT_AUTHORIZED",
            "AUTHORITY",
            "NON_RETRYABLE",
            uuid::Uuid::new_v4().to_string(),
            json!({"message":"Only the confirmed, designated All-Father identity may use this sign-in route"}),
        ));
    }

    let user = state
        .user_store
        .find_by_username(ALLFATHER_USERNAME)
        .await
        .map_err(|_| auth_error("USER_STORE_UNAVAILABLE"))?
        .filter(|user| user.is_active)
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::FORBIDDEN,
                "ONYX_ACCOUNT_NOT_PROVISIONED",
                "AUTHORITY",
                "NON_RETRYABLE",
                uuid::Uuid::new_v4().to_string(),
                json!({}),
            )
        })?;

    if !user.is_admin {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "ALLFATHER_NOT_ADMIN",
            "AUTHORITY",
            "NON_RETRYABLE",
            uuid::Uuid::new_v4().to_string(),
            json!({}),
        ));
    }

    let access_token = issue_token(
        &state,
        &user,
        "access",
        3600,
        ClientType::Web,
    )
    .await
    .map_err(|_| auth_error("TOKEN_ISSUANCE_FAILED"))?;
    let refresh_token = issue_token(
        &state,
        &user,
        "refresh",
        7 * 24 * 3600,
        ClientType::Web,
    )
    .await
    .map_err(|_| auth_error("TOKEN_ISSUANCE_FAILED"))?;

    Ok(Json(SupabaseIdentityResponse {
        access_token,
        refresh_token,
        expires_in: 3600,
        user: LoginUser {
            id: user.user_id,
            username: user.username,
            organization_id: user.organization_id,
            is_admin: user.is_admin,
            class: user.class.map(|class| class.as_str().to_owned()),
        },
    }))
}


#[cfg(test)]
mod tests {
    use super::{is_designated_identity, SupabaseUser, ALLFATHER_EMAIL};

    fn user(id: &str, email: &str, confirmed: Option<&str>) -> SupabaseUser {
        SupabaseUser {
            id: id.to_owned(),
            email: Some(email.to_owned()),
            email_confirmed_at: confirmed.map(str::to_owned),
            confirmed_at: None,
        }
    }

    #[test]
    fn accepts_only_confirmed_designated_subject() {
        assert!(is_designated_identity(
            &user("supabase-user-123", ALLFATHER_EMAIL, Some("2026-10-01T00:00:00Z")),
            "supabase-user-123"
        ));
    }

    #[test]
    fn rejects_subject_mismatch_even_when_email_is_confirmed() {
        assert!(!is_designated_identity(
            &user("different-subject", ALLFATHER_EMAIL, Some("2026-10-01T00:00:00Z")),
            "supabase-user-123"
        ));
    }

    #[test]
    fn rejects_unconfirmed_or_non_designated_email() {
        assert!(!is_designated_identity(
            &user("supabase-user-123", ALLFATHER_EMAIL, None),
            "supabase-user-123"
        ));
        assert!(!is_designated_identity(
            &user("supabase-user-123", "other@example.com", Some("2026-10-01T00:00:00Z")),
            "supabase-user-123"
        ));
    }
}
