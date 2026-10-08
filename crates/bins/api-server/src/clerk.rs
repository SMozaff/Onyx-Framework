//! Clerk-backed identity exchange for ONYX.
//!
//! Clerk authenticates the human identity. ONYX remains authoritative for
//! registration and authorization. Allfather is bound to the designated,
//! verified Google/Clerk email address rather than to a client-supplied value
//! or a hard-coded password.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::http::HeaderMap;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ring::signature::{UnparsedPublicKey, RSA_PKCS1_2048_8192_SHA256};
use serde::Deserialize;
use tokio::sync::RwLock;



use super::{auth::LoginUser, issue_token, ApiError, ApiState};

pub const ALLFATHER_USERNAME: &str = "allfather";
pub const ALLFATHER_EMAIL: &str = "so.muzaff@gmail.com";

#[derive(Clone)]
pub struct ClerkAuth {
    issuer: String,
    jwks_url: String,
    secret_key: String,
    client: reqwest::Client,
    keys: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

#[derive(Debug, Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: String,
    kty: String,
    alg: Option<String>,
    n: Option<String>,
    e: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Claims {
    pub sub: String,
    iss: String,
    exp: u64,
    #[serde(default)]
    nbf: Option<u64>,
    #[serde(default)]
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Header {
    alg: String,
    kid: String,
}

#[derive(Debug, Deserialize)]
struct ClerkEmailAddress {
    id: String,
    email_address: String,
    #[serde(default)]
    verification: Option<ClerkVerification>,
}

#[derive(Debug, Deserialize)]
struct ClerkVerification {
    status: String,
}

#[derive(Debug, Deserialize)]
struct ClerkUser {
    primary_email_address_id: Option<String>,
    email_addresses: Vec<ClerkEmailAddress>,
}

impl ClerkAuth {
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        let issuer = match std::env::var("CLERK_ISSUER")
            .ok()
            .filter(|value| !value.trim().is_empty())
        {
            Some(value) => value.trim_end_matches('/').to_owned(),
            None => return Ok(None),
        };

        let secret_key = match std::env::var("CLERK_SECRET_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
        {
            Some(value) => value,
            None => anyhow::bail!(
                "CLERK_SECRET_KEY is required when Clerk authentication is configured"
            ),
        };

        let jwks_url = std::env::var("CLERK_JWKS_URL")
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("{issuer}/.well-known/jwks.json"));

        Ok(Some(Self {
            issuer,
            jwks_url,
            secret_key,
            client: reqwest::Client::new(),
            keys: Arc::new(RwLock::new(HashMap::new())),
        }))
    }

    async fn load_key(&self, kid: &str) -> Result<Vec<u8>, ApiError> {
        if let Some(key) = self.keys.read().await.get(kid).cloned() {
            return Ok(key);
        }

        let set: JwkSet = self
            .client
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|_| auth_error("CLERK_JWKS_UNAVAILABLE"))?
            .error_for_status()
            .map_err(|_| auth_error("CLERK_JWKS_UNAVAILABLE"))?
            .json()
            .await
            .map_err(|_| auth_error("CLERK_JWKS_INVALID"))?;

        let jwk = set
            .keys
            .into_iter()
            .find(|key| {
                key.kid == kid
                    && key.kty == "RSA"
                    && key.alg.as_deref().unwrap_or("RS256") == "RS256"
            })
            .ok_or_else(|| auth_error("CLERK_SIGNING_KEY_NOT_FOUND"))?;

        let n = URL_SAFE_NO_PAD
            .decode(jwk.n.ok_or_else(|| auth_error("CLERK_JWK_INVALID"))?)
            .map_err(|_| auth_error("CLERK_JWK_INVALID"))?;
        let e = URL_SAFE_NO_PAD
            .decode(jwk.e.ok_or_else(|| auth_error("CLERK_JWK_INVALID"))?)
            .map_err(|_| auth_error("CLERK_JWK_INVALID"))?;

        let der = rsa_public_key_der(&n, &e);
        self.keys.write().await.insert(kid.to_owned(), der.clone());

        Ok(der)
    }

    pub async fn verify_bearer(&self, headers: &HeaderMap) -> Result<Claims, ApiError> {
        let value = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| auth_error("CLERK_AUTH_REQUIRED"))?;

        let token = value
            .strip_prefix("Bearer ")
            .or_else(|| value.strip_prefix("bearer "))
            .ok_or_else(|| auth_error("CLERK_AUTH_REQUIRED"))?;

        let mut parts = token.split('.');
        let header_part = parts
            .next()
            .ok_or_else(|| auth_error("CLERK_TOKEN_INVALID"))?;
        let payload_part = parts
            .next()
            .ok_or_else(|| auth_error("CLERK_TOKEN_INVALID"))?;
        let signature_part = parts
            .next()
            .ok_or_else(|| auth_error("CLERK_TOKEN_INVALID"))?;

        if parts.next().is_some() {
            return Err(auth_error("CLERK_TOKEN_INVALID"));
        }

        let header: Header = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(header_part)
                .map_err(|_| auth_error("CLERK_TOKEN_INVALID"))?,
        )
        .map_err(|_| auth_error("CLERK_TOKEN_INVALID"))?;

        if header.alg != "RS256" || header.kid.is_empty() {
            return Err(auth_error("CLERK_TOKEN_INVALID"));
        }

        let key = self.load_key(&header.kid).await?;
        let signed = format!("{header_part}.{payload_part}");
        let signature = URL_SAFE_NO_PAD
            .decode(signature_part)
            .map_err(|_| auth_error("CLERK_TOKEN_INVALID"))?;

        UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, key)
            .verify(signed.as_bytes(), &signature)
            .map_err(|_| auth_error("CLERK_TOKEN_INVALID"))?;

        let claims: Claims = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(payload_part)
                .map_err(|_| auth_error("CLERK_TOKEN_INVALID"))?,
        )
        .map_err(|_| auth_error("CLERK_TOKEN_INVALID"))?;

        if claims.iss != self.issuer {
            return Err(auth_error("CLERK_TOKEN_INVALID"));
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if claims.exp <= now || claims.nbf.map(|value| value > now).unwrap_or(false) {
            return Err(auth_error("CLERK_TOKEN_EXPIRED"));
        }

        if claims.status.as_deref() == Some("pending") || claims.sub.is_empty() {
            return Err(auth_error("CLERK_TOKEN_INVALID"));
        }

        Ok(claims)
    }

    async fn verified_email_for_subject(&self, subject: &str) -> Result<Option<String>, ApiError> {
        let response = self
            .client
            .get(format!("https://api.clerk.com/v1/users/{subject}"))
            .bearer_auth(&self.secret_key)
            .send()
            .await
            .map_err(|_| auth_error("CLERK_USER_LOOKUP_UNAVAILABLE"))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let user: ClerkUser = response
            .error_for_status()
            .map_err(|_| auth_error("CLERK_USER_LOOKUP_FAILED"))?
            .json()
            .await
            .map_err(|_| auth_error("CLERK_USER_LOOKUP_INVALID"))?;

        let primary_id = match user.primary_email_address_id {
            Some(id) => id,
            None => return Ok(None),
        };

        let email = user
            .email_addresses
            .into_iter()
            .find(|item| item.id == primary_id)
            .filter(|item| {
                item.verification
                    .as_ref()
                    .map(|verification| verification.status == "verified")
                    .unwrap_or(false)
            })
            .map(|item| item.email_address.trim().to_ascii_lowercase());

        Ok(email)
    }

    pub async fn is_allfather(&self, subject: &str) -> Result<bool, ApiError> {
        Ok(self.verified_email_for_subject(subject).await?.as_deref() == Some(ALLFATHER_EMAIL))
    }
}


