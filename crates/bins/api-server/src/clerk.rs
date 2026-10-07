//! Clerk-backed identity exchange for ONYX.
//!
//! Clerk answers identity ("who are you?"). ONYX remains authoritative for
//! registration and authorization ("are you registered here, and what may you
//! do?"). A successful Clerk authentication therefore never creates an
//! ordinary ONYX account implicitly.

use axum::{extract::State, http::{HeaderMap, StatusCode}, Json};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::{distributions::Alphanumeric, Rng};
use ring::signature::{UnparsedPublicKey, RSA_PKCS1_2048_8192_SHA256};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc, time::{Duration, SystemTime, UNIX_EPOCH}};
use tokio::sync::RwLock;

use security_application::{NewUser, UserClass, UserRecord};

use super::{admin, authenticate_headers, issue_token, ApiError, ApiState, LoginResponse, LoginUser};

pub const ALLFATHER_USERNAME: &str = "allfather";

#[derive(Clone)]
pub struct ClerkAuth {
    issuer: String,
    jwks_url: String,
    allfather_user_id: String,
    client: reqwest::Client,
    keys: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

#[derive(Debug, Deserialize)]
struct JwkSet { keys: Vec<Jwk> }
#[derive(Debug, Deserialize)]
struct Jwk { kid: String, kty: String, alg: Option<String>, n: Option<String>, e: Option<String> }

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    iss: String,
    exp: u64,
    #[serde(default)] nbf: Option<u64>,
    #[serde(default)] sid: Option<String>,
    #[serde(default)] status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Header { alg: String, kid: String, typ: Option<String> }

impl ClerkAuth {
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        let allfather = match std::env::var("CLERK_ALLFATHER_USER_ID").ok().filter(|v| !v.is_empty()) {
            Some(v) => v,
            None => return Ok(None),
        };
        let issuer = match std::env::var("CLERK_ISSUER").ok().filter(|v| !v.is_empty()) {
            Some(v) => v.trim_end_matches('/').to_owned(),
            None => return Ok(None),
        };
        let jwks_url = std::env::var("CLERK_JWKS_URL")
            .ok().filter(|v| !v.is_empty())
            .unwrap_or_else(|| format!("{issuer}/.well-known/jwks.json"));
        Ok(Some(Self { issuer, jwks_url, allfather_user_id: allfather, client: reqwest::Client::new(), keys: Arc::new(RwLock::new(HashMap::new())) }))
    }

    async fn load_key(&self, kid: &str) -> Result<Vec<u8>, ApiError> {
        if let Some(key) = self.keys.read().await.get(kid).cloned() { return Ok(key); }
        let set: JwkSet = self.client.get(&self.jwks_url).send().await
            .map_err(|_| auth_error("CLERK_JWKS_UNAVAILABLE"))?
            .error_for_status().map_err(|_| auth_error("CLERK_JWKS_UNAVAILABLE"))?
            .json().await.map_err(|_| auth_error("CLERK_JWKS_INVALID"))?;
        let jwk = set.keys.into_iter().find(|k| k.kid == kid && k.kty == "RSA" && k.alg.as_deref().unwrap_or("RS256") == "RS256")
            .ok_or_else(|| auth_error("CLERK_SIGNING_KEY_NOT_FOUND"))?;
        let n = URL_SAFE_NO_PAD.decode(jwk.n.ok_or_else(|| auth_error("CLERK_JWK_INVALID"))?).map_err(|_| auth_error("CLERK_JWK_INVALID"))?;
        let e = URL_SAFE_NO_PAD.decode(jwk.e.ok_or_else(|| auth_error("CLERK_JWK_INVALID"))?).map_err(|_| auth_error("CLERK_JWK_INVALID"))?;
        let der = rsa_public_key_der(&n, &e);
        self.keys.write().await.insert(kid.to_owned(), der.clone());
        Ok(der)
    }

    pub async fn verify_bearer(&self, headers: &HeaderMap) -> Result<Claims, ApiError> {
        let value = headers.get("authorization").and_then(|v| v.to_str().ok()).ok_or_else(|| auth_error("CLERK_AUTH_REQUIRED"))?;
        let token = value.strip_prefix("Bearer ").or_else(|| value.strip_prefix("bearer ")).ok_or_else(|| auth_error("CLERK_AUTH_REQUIRED"))?;
        let mut parts = token.split('.');
        let h = parts.next().ok_or_else(|| auth_error("CLERK_TOKEN_INVALID"))?;
        let p = parts.next().ok_or_else(|| auth_error("CLERK_TOKEN_INVALID"))?;
        let s = parts.next().ok_or_else(|| auth_error("CLERK_TOKEN_INVALID"))?;
        if parts.next().is_some() { return Err(auth_error("CLERK_TOKEN_INVALID")); }
        let header: Header = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(h).map_err(|_| auth_error("CLERK_TOKEN_INVALID"))?).map_err(|_| auth_error("CLERK_TOKEN_INVALID"))?;
        if header.alg != "RS256" || header.kid.is_empty() { return Err(auth_error("CLERK_TOKEN_INVALID")); }
        let key = self.load_key(&header.kid).await?;
        let signed = format!("{h}.{p}");
        let sig = URL_SAFE_NO_PAD.decode(s).map_err(|_| auth_error("CLERK_TOKEN_INVALID"))?;
        UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, key).verify(signed.as_bytes(), &sig).map_err(|_| auth_error("CLERK_TOKEN_INVALID"))?;
        let claims: Claims = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(p).map_err(|_| auth_error("CLERK_TOKEN_INVALID"))?).map_err(|_| auth_error("CLERK_TOKEN_INVALID"))?;
        if claims.iss != self.issuer { return Err(auth_error("CLERK_TOKEN_INVALID")); }
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        if claims.exp <= now || claims.nbf.map(|v| v > now).unwrap_or(false) { return Err(auth_error("CLERK_TOKEN_EXPIRED")); }
        if claims.status.as_deref() == Some("pending") { return Err(auth_error("CLERK_SESSION_PENDING")); }
        if claims.sub.is_empty() { return Err(auth_error("CLERK_TOKEN_INVALID")); }
        Ok(claims)
    }

    pub fn is_allfather(&self, subject: &str) -> bool { subject == self.allfather_user_id }
}

#[derive(Debug, Deserialize)]
pub struct ClerkProvisionRequest {
    pub clerk_user_id: String,
    #[serde(default)] pub is_admin: bool,
    #[serde(default)] pub is_manager: bool,
    #[serde(default)] pub class: Option<String>,
    #[serde(default)] pub parent_user_id: Option<String>,
    #[serde(default)] pub organization_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ClerkIdentityResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub user: LoginUser,
}

fn auth_error(code: &str) -> ApiError {
    ApiError::new(StatusCode::UNAUTHORIZED, code, "AUTHORITY", "NON_RETRYABLE", uuid::Uuid::new_v4().to_string(), json!({}))
}

fn provisioned_username(subject: &str) -> String { format!("clerk:{subject}") }

fn unusable_password() -> String {
    format!("clerk-{}-{}", uuid::Uuid::new_v4(), rand::thread_rng().sample_iter(&Alphanumeric).take(24).map(char::from).collect::<String>())
}

fn rsa_public_key_der(n: &[u8], e: &[u8]) -> Vec<u8> {
    fn integer(mut x: Vec<u8>) -> Vec<u8> { while x.len() > 1 && x[0] == 0 { x.remove(0); } if x[0] & 0x80 != 0 { x.insert(0, 0); } tlv(0x02, &x) }
    fn len(n: usize) -> Vec<u8> { if n < 128 { vec![n as u8] } else { let mut b = Vec::new(); let mut x=n; while x>0 { b.push((x&255) as u8); x >>= 8; } b.reverse(); let mut out=vec![0x80|b.len() as u8]; out.extend(b); out } }
    fn tlv(tag:u8, value:&[u8])->Vec<u8>{ let mut out=vec![tag]; out.extend(len(value.len())); out.extend(value); out }
    tlv(0x30, &[integer(n.to_vec()), integer(e.to_vec())].concat())
}

pub async fn login(State(state): State<ApiState>, headers: HeaderMap) -> Result<Json<ClerkIdentityResponse>, ApiError> {
    let clerk = state.clerk_auth.as_ref().ok_or_else(|| auth_error("CLERK_NOT_CONFIGURED"))?;
    let claims = clerk.verify_bearer(&headers).await?;
    let username = if clerk.is_allfather(&claims.sub) { ALLFATHER_USERNAME.to_owned() } else { provisioned_username(&claims.sub) };
    let user = state.user_store.find_by_username(&username).await.map_err(|_| auth_error("USER_STORE_UNAVAILABLE"))?;
    let user = match user {
        Some(u) if u.is_active => u,
        _ => return Err(ApiError::new(StatusCode::FORBIDDEN, "ONYX_ACCOUNT_NOT_PROVISIONED", "AUTHORITY", "NON_RETRYABLE", uuid::Uuid::new_v4().to_string(), json!({"message":"This Clerk identity has not been registered in ONYX"}))),
    };
    let access_token = issue_token(&state, &user, "access", 3600, super::client_type::ClientType::Web).await.map_err(|_| auth_error("TOKEN_ISSUANCE_FAILED"))?;
    let refresh_token = issue_token(&state, &user, "refresh", 7*24*3600, super::client_type::ClientType::Web).await.map_err(|_| auth_error("TOKEN_ISSUANCE_FAILED"))?;
    Ok(Json(ClerkIdentityResponse { access_token, refresh_token, expires_in: 3600, user: LoginUser { id:user.user_id, username:user.username, organization_id:user.organization_id, is_admin:user.is_admin, class:user.class.map(|c| c.as_str().to_owned()) } }))
}

pub async fn provision(State(state): State<ApiState>, headers: HeaderMap, Json(payload): Json<ClerkProvisionRequest>) -> Result<Json<admin::UserDto>, ApiError> {
    let admin = super::admin::require_admin(&state, &headers).await?;
    if payload.clerk_user_id.trim().is_empty() { return Err(ApiError::new(StatusCode::BAD_REQUEST,"INVALID_CLERK_USER_ID","VALIDATION","NON_RETRYABLE",uuid::Uuid::new_v4().to_string(),json!({}))); }
    if payload.clerk_user_id == clerk_subject_for_allfather(&state).await.unwrap_or_default() { return Err(ApiError::new(StatusCode::CONFLICT,"ALLFATHER_IDENTITY_RESERVED","VALIDATION","NON_RETRYABLE",uuid::Uuid::new_v4().to_string(),json!({}))); }
    let username = provisioned_username(&payload.clerk_user_id);
    if state.user_store.find_by_username(&username).await.map_err(|_| auth_error("USER_STORE_UNAVAILABLE"))?.is_some() { return Err(ApiError::new(StatusCode::CONFLICT,"ONYX_ACCOUNT_ALREADY_PROVISIONED","VALIDATION","NON_RETRYABLE",uuid::Uuid::new_v4().to_string(),json!({}))); }
    let class = payload.class.as_deref().map(UserClass::parse).transpose().map_err(|_| ApiError::new(StatusCode::BAD_REQUEST,"INVALID_CLASS","VALIDATION","NON_RETRYABLE",uuid::Uuid::new_v4().to_string(),json!({})))?;
    let hash = state.password_hasher.hash(&unusable_password()).map_err(|_| auth_error("PASSWORD_HASH_FAILED"))?;
    let record = state.user_store.create(NewUser { user_id:uuid::Uuid::new_v4().to_string(), username, organization_id:payload.organization_id.unwrap_or(admin.organization_id), password_hash:hash, is_admin:payload.is_admin, is_manager:payload.is_manager, class, parent_user_id:payload.parent_user_id }).await.map_err(|_| ApiError::new(StatusCode::CONFLICT,"ONYX_ACCOUNT_ALREADY_PROVISIONED","VALIDATION","NON_RETRYABLE",uuid::Uuid::new_v4().to_string(),json!({})))?;
    Ok(Json(record.into()))
}

async fn clerk_subject_for_allfather(state: &ApiState) -> Option<String> { state.clerk_auth.as_ref().map(|c| c.allfather_user_id.clone()) }
