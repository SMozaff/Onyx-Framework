//! Real per-user login for `desktop-shell`.
//!
//! # Token policy (docs/migrations/slint/desktop-token-storage-policy.md)
//! - The OS keyring (`SecureStorage`) stores the **refresh token only**, plus
//!   the non-secret identity needed to rebuild local state on launch.
//! - The **access token lives in memory only** (see `session_authority`). It is
//!   never written to disk, the keyring, logs, diagnostics or IPC payloads.
//! - Startup restore exchanges the stored refresh token for a new access token
//!   through `POST /api/auth/refresh`, which rotates the refresh token. Every
//!   rotation is persisted before it is used.
//!
//! The HTTP calls (login, refresh, logout) are behind the `SessionApi` trait so
//! the authority can be tested without a network. `HttpSessionApi` is the real
//! implementation against the Worker routes in
//! `deploy/cloudflare/edge-gateway/src/index.ts` (`authLogin`, `authRefresh`,
//! `authLogout`).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use platform_kernel::{ObjectId, OrganizationId};
use serde::{Deserialize, Serialize};

use crate::secure_storage::SecureStorage;

/// Single storage key for the whole session, stored as one JSON blob rather
/// than several keys so a save or clear is atomic.
pub const SESSION_KEY: &str = "auth.session";

/// The persisted, refresh-only record. Contains **no access token**.
///
/// A record written by an earlier build may still carry an `access_token`
/// field. serde ignores unknown fields, so it is dropped on load and the
/// next save rewrites the record without it.
#[derive(Clone, Serialize, Deserialize)]
pub struct StoredSession {
    pub server_address: String,
    pub refresh_token: String,
    pub user_id: ObjectId,
    pub organization_id: OrganizationId,
    pub username: String,
    pub is_admin: bool,
}

impl std::fmt::Debug for StoredSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredSession")
            .field("server_address", &self.server_address)
            .field("refresh_token", &"<redacted>")
            .field("user_id", &self.user_id)
            .field("organization_id", &self.organization_id)
            .field("username", &self.username)
            .field("is_admin", &self.is_admin)
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("network error contacting server: {0}")]
    Network(String),
    #[error("server rejected credentials")]
    InvalidCredentials,
    #[error("server returned an unexpected response: {0}")]
    UnexpectedResponse(String),
    #[error("storage error: {0}")]
    Storage(String),
}

/// A freshly issued token pair with its access-token lifetime. Not `Debug`
/// derived: the tokens are redacted by the manual impl.
#[derive(Clone)]
pub struct IssuedPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

impl std::fmt::Debug for IssuedPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuedPair")
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

/// Non-secret identity returned with a login.
#[derive(Debug, Clone)]
pub struct LoginIdentity {
    pub user_id: ObjectId,
    pub organization_id: OrganizationId,
    pub username: String,
    pub is_admin: bool,
}

#[derive(Debug, Clone)]
pub struct LoginResult {
    pub pair: IssuedPair,
    pub identity: LoginIdentity,
}

/// Why a refresh did not produce a new pair.
///
/// `Rejected` is a definitive server answer (401 `INVALID_REFRESH_TOKEN`: the
/// token is revoked, replayed, expired or the account is inactive). Callers
/// clear the stored refresh token on `Rejected`. `Unavailable` (network,
/// timeout, 5xx, malformed body) is not a verdict on the session, so the
/// stored token is kept for the next launch. Neither variant carries server
/// text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshError {
    Rejected,
    Unavailable,
}

/// Session HTTP contract. Implementations must not put tokens in URLs or error
/// text.
#[async_trait]
pub trait SessionApi: Send + Sync {
    async fn login(&self, username: &str, password: &str) -> Result<LoginResult, SessionError>;
    async fn refresh(&self, refresh_token: &str) -> Result<IssuedPair, RefreshError>;
    /// Best-effort server-side revocation. Local state is already cleared.
    async fn revoke(&self, access_token: &str, refresh_token: &str) -> Result<(), SessionError>;
}

/// Mirrors `api-server::routes::auth::LoginRequest` (see `authLogin`).
#[derive(Serialize)]
struct LoginRequest<'a> {
    username: &'a str,
    password: &'a str,
    client_type: &'a str,
}

#[derive(Deserialize)]
struct TokenResponseWire {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

#[derive(Deserialize)]
struct LoginResponseWire {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    user: LoginUserWire,
}

#[derive(Deserialize)]
struct LoginUserWire {
    id: String,
    username: String,
    organization_id: String,
    is_admin: bool,
}

/// Real HTTP implementation of `SessionApi`.
pub struct HttpSessionApi {
    origin: String,
    client: reqwest::Client,
}

impl HttpSessionApi {
    pub fn new(server_address: &str) -> Result<Self, SessionError> {
        Self::with_timeout(server_address, Duration::from_secs(15))
    }

    pub fn with_timeout(server_address: &str, timeout: Duration) -> Result<Self, SessionError> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| SessionError::Network(e.to_string()))?;
        Ok(Self {
            origin: server_address.trim_end_matches('/').to_string(),
            client,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.origin, path)
    }
}

fn non_empty(value: &str) -> bool {
    !value.trim().is_empty()
}

#[async_trait]
impl SessionApi for HttpSessionApi {
    async fn login(&self, username: &str, password: &str) -> Result<LoginResult, SessionError> {
        let response = self
            .client
            .post(self.url("/api/auth/login"))
            .json(&LoginRequest {
                username,
                password,
                client_type: "desktop",
            })
            .send()
            .await
            .map_err(|e| SessionError::Network(e.to_string()))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SessionError::InvalidCredentials);
        }
        if !response.status().is_success() {
            return Err(SessionError::UnexpectedResponse(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let body: LoginResponseWire = response
            .json()
            .await
            .map_err(|e| SessionError::UnexpectedResponse(e.to_string()))?;
        if !non_empty(&body.access_token) || !non_empty(&body.refresh_token) || body.expires_in == 0
        {
            return Err(SessionError::UnexpectedResponse(
                "login response missing token material".to_string(),
            ));
        }
        let user_id = ObjectId::from_uuid_str(&body.user.id)
            .map_err(|e| SessionError::UnexpectedResponse(format!("invalid user id: {e}")))?;
        let organization_id =
            OrganizationId::from_uuid_str(&body.user.organization_id).map_err(|e| {
                SessionError::UnexpectedResponse(format!("invalid organization id: {e}"))
            })?;

        Ok(LoginResult {
            pair: IssuedPair {
                access_token: body.access_token,
                refresh_token: body.refresh_token,
                expires_in: body.expires_in,
            },
            identity: LoginIdentity {
                user_id,
                organization_id,
                username: body.user.username,
                is_admin: body.user.is_admin,
            },
        })
    }

    async fn refresh(&self, refresh_token: &str) -> Result<IssuedPair, RefreshError> {
        let response = self
            .client
            .post(self.url("/api/auth/refresh"))
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send()
            .await
            .map_err(|_| RefreshError::Unavailable)?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(RefreshError::Rejected);
        }
        if !response.status().is_success() {
            return Err(RefreshError::Unavailable);
        }
        let body: TokenResponseWire = response
            .json()
            .await
            .map_err(|_| RefreshError::Unavailable)?;
        if !non_empty(&body.access_token) || !non_empty(&body.refresh_token) || body.expires_in == 0
        {
            return Err(RefreshError::Unavailable);
        }
        Ok(IssuedPair {
            access_token: body.access_token,
            refresh_token: body.refresh_token,
            expires_in: body.expires_in,
        })
    }

    async fn revoke(&self, access_token: &str, refresh_token: &str) -> Result<(), SessionError> {
        // Worker `authLogout` accepts the bearer and the refresh token together.
        // It always answers `{success: true}` for well-formed requests.
        let response = self
            .client
            .post(self.url("/api/auth/logout"))
            .bearer_auth(access_token)
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send()
            .await
            .map_err(|e| SessionError::Network(e.to_string()))?;
        if !response.status().is_success() {
            return Err(SessionError::UnexpectedResponse(format!(
                "HTTP {}",
                response.status()
            )));
        }
        Ok(())
    }
}

/// Persists the refresh-only record via `SecureStorage`.
pub async fn save(
    storage: &Arc<dyn SecureStorage>,
    session: &StoredSession,
) -> Result<(), SessionError> {
    let json = serde_json::to_vec(session)
        .map_err(|e| SessionError::Storage(format!("serializing session: {e}")))?;
    storage
        .store_secret(SESSION_KEY, &json)
        .await
        .map_err(|e| SessionError::Storage(e.to_string()))
}

/// Loads a saved refresh-only record, if any. `Ok(None)` when nothing is saved.
pub async fn load(storage: &Arc<dyn SecureStorage>) -> Result<Option<StoredSession>, SessionError> {
    let bytes = storage
        .get_secret(SESSION_KEY)
        .await
        .map_err(|e| SessionError::Storage(e.to_string()))?;
    match bytes {
        None => Ok(None),
        Some(b) => serde_json::from_slice(&b)
            .map(Some)
            .map_err(|e| SessionError::Storage(format!("corrupt stored session: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_storage::StorageError;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeStorage {
        entries: Mutex<HashMap<String, Vec<u8>>>,
    }

    #[async_trait::async_trait]
    impl SecureStorage for FakeStorage {
        async fn store_secret(&self, key: &str, value: &[u8]) -> Result<(), StorageError> {
            self.entries
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_vec());
            Ok(())
        }

        async fn get_secret(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
            Ok(self.entries.lock().unwrap().get(key).cloned())
        }

        async fn delete_secret(&self, key: &str) -> Result<(), StorageError> {
            self.entries.lock().unwrap().remove(key);
            Ok(())
        }
    }

    fn sample_session() -> StoredSession {
        StoredSession {
            server_address: "http://192.168.0.250:3000".to_string(),
            refresh_token: "generated-refresh-value".to_string(),
            user_id: ObjectId::new_random(),
            organization_id: OrganizationId::new_random(),
            username: "generated-user".to_string(),
            is_admin: true,
        }
    }

    #[tokio::test]
    async fn load_returns_none_when_nothing_saved() {
        let storage: Arc<dyn SecureStorage> = Arc::new(FakeStorage::default());
        assert!(load(&storage).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn save_then_load_round_trips_every_persisted_field() {
        let storage: Arc<dyn SecureStorage> = Arc::new(FakeStorage::default());
        let original = sample_session();

        save(&storage, &original).await.unwrap();
        let loaded = load(&storage).await.unwrap().expect("session should exist");

        assert_eq!(loaded.server_address, original.server_address);
        assert_eq!(loaded.refresh_token, original.refresh_token);
        assert_eq!(loaded.user_id, original.user_id);
        assert_eq!(loaded.organization_id, original.organization_id);
        assert_eq!(loaded.username, original.username);
        assert_eq!(loaded.is_admin, original.is_admin);
    }

    #[tokio::test]
    async fn persisted_record_never_contains_an_access_token_field() {
        let storage = Arc::new(FakeStorage::default());
        let dyn_storage: Arc<dyn SecureStorage> = storage.clone();
        save(&dyn_storage, &sample_session()).await.unwrap();

        let raw = storage
            .entries
            .lock()
            .unwrap()
            .get(SESSION_KEY)
            .cloned()
            .unwrap();
        let text = String::from_utf8(raw).unwrap();
        assert!(
            !text.contains("access_token"),
            "record must be refresh-only"
        );
    }

    #[tokio::test]
    async fn legacy_record_with_access_token_loads_without_it() {
        // Records written before the token policy carried an access_token.
        // They must still load (for the refresh exchange) and must not
        // expose the old access token afterwards.
        let storage: Arc<dyn SecureStorage> = Arc::new(FakeStorage::default());
        let mut legacy = serde_json::to_value(sample_session()).unwrap();
        legacy["refresh_token"] = serde_json::json!("legacy-refresh-generated");
        legacy["access_token"] = serde_json::json!("legacy-access-generated");
        storage
            .store_secret(SESSION_KEY, legacy.to_string().as_bytes())
            .await
            .unwrap();

        let loaded = load(&storage).await.unwrap().expect("legacy record loads");
        assert_eq!(loaded.refresh_token, "legacy-refresh-generated");
        let debug = format!("{loaded:?}");
        assert!(!debug.contains("legacy-refresh-generated"));
        assert!(!debug.contains("legacy-access-generated"));
    }

    #[test]
    fn stored_session_debug_redacts_refresh_token() {
        let debug = format!("{:?}", sample_session());
        assert!(!debug.contains("generated-refresh-value"));
        assert!(debug.contains("<redacted>"));
    }

    #[test]
    fn issued_pair_debug_redacts_both_tokens() {
        let pair = IssuedPair {
            access_token: "generated-access-value".into(),
            refresh_token: "generated-refresh-value".into(),
            expires_in: 3600,
        };
        let debug = format!("{pair:?}");
        assert!(!debug.contains("generated-access-value"));
        assert!(!debug.contains("generated-refresh-value"));
    }

    #[tokio::test]
    async fn clear_on_empty_storage_is_not_an_error() {
        let storage: Arc<dyn SecureStorage> = Arc::new(FakeStorage::default());
        storage.delete_secret(SESSION_KEY).await.unwrap();
    }

    #[tokio::test]
    async fn load_surfaces_corrupt_stored_data_as_an_error_not_a_panic() {
        let storage: Arc<dyn SecureStorage> = Arc::new(FakeStorage::default());
        storage
            .store_secret(SESSION_KEY, b"not valid json")
            .await
            .unwrap();

        let result = load(&storage).await;
        assert!(matches!(result, Err(SessionError::Storage(_))));
    }
}

#[cfg(test)]
mod http_contract_tests {
    //! Contract tests for `HttpSessionApi` against an axum mock of the Worker
    //! auth routes (`authLogin`, `authRefresh`, `authLogout`). The mock is a
    //! stand-in. Live Worker checks are not run here.
    use super::*;
    use axum::{
        extract::State,
        http::{HeaderMap, StatusCode},
        routing::post,
        Json, Router,
    };
    use serde_json::{json, Value};
    use std::sync::Mutex;

    #[derive(Default)]
    struct Mock {
        login_status: Mutex<u16>,
        refresh_status: Mutex<u16>,
        refresh_delay: Mutex<Duration>,
        logout_status: Mutex<u16>,
        last_logout_auth: Mutex<Option<String>>,
        last_logout_body: Mutex<Option<String>>,
        last_refresh_body: Mutex<Option<String>>,
    }

    fn token_body() -> Value {
        json!({
            "access_token": "generated-access-http",
            "refresh_token": "generated-refresh-http",
            "expires_in": 3600,
            "user": {"id": uuid::Uuid::from_bytes([1; 16]).to_string(),
                     "username": "generated-user",
                     "organization_id": uuid::Uuid::from_bytes([2; 16]).to_string(),
                     "is_admin": true}
        })
    }

    async fn login(State(m): State<Arc<Mock>>, Json(_b): Json<Value>) -> (StatusCode, Json<Value>) {
        let status = *m.login_status.lock().unwrap();
        (StatusCode::from_u16(status).unwrap(), Json(token_body()))
    }

    async fn refresh(
        State(m): State<Arc<Mock>>,
        Json(b): Json<Value>,
    ) -> (StatusCode, Json<Value>) {
        *m.last_refresh_body.lock().unwrap() = Some(b.to_string());
        let delay = *m.refresh_delay.lock().unwrap();
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        let status = *m.refresh_status.lock().unwrap();
        (StatusCode::from_u16(status).unwrap(), Json(token_body()))
    }

    async fn logout(
        State(m): State<Arc<Mock>>,
        headers: HeaderMap,
        Json(b): Json<Value>,
    ) -> (StatusCode, Json<Value>) {
        *m.last_logout_auth.lock().unwrap() = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        *m.last_logout_body.lock().unwrap() = Some(b.to_string());
        let status = *m.logout_status.lock().unwrap();
        (
            StatusCode::from_u16(status).unwrap(),
            Json(json!({"success": true})),
        )
    }

    async fn serve(mock: Arc<Mock>) -> String {
        let app = Router::new()
            .route("/api/auth/login", post(login))
            .route("/api/auth/refresh", post(refresh))
            .route("/api/auth/logout", post(logout))
            .with_state(mock);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{address}")
    }

    fn mock_with(login: u16, refresh: u16, logout: u16) -> Arc<Mock> {
        let m = Mock::default();
        *m.login_status.lock().unwrap() = login;
        *m.refresh_status.lock().unwrap() = refresh;
        *m.logout_status.lock().unwrap() = logout;
        Arc::new(m)
    }

    #[tokio::test]
    async fn login_success_returns_pair_and_identity() {
        let origin = serve(mock_with(200, 200, 200)).await;
        let api = HttpSessionApi::new(&origin).unwrap();
        let result = api
            .login("generated-user", "generated-password")
            .await
            .unwrap();
        assert_eq!(result.pair.expires_in, 3600);
        assert_eq!(result.pair.access_token, "generated-access-http");
        assert!(result.identity.is_admin);
    }

    #[tokio::test]
    async fn login_401_is_invalid_credentials() {
        let origin = serve(mock_with(401, 200, 200)).await;
        let api = HttpSessionApi::new(&origin).unwrap();
        assert!(matches!(
            api.login("generated-user", "wrong").await,
            Err(SessionError::InvalidCredentials)
        ));
    }

    #[tokio::test]
    async fn refresh_200_rotates_both_tokens() {
        let mock = mock_with(200, 200, 200);
        let origin = serve(Arc::clone(&mock)).await;
        let api = HttpSessionApi::new(&origin).unwrap();
        let pair = api.refresh("generated-refresh-old").await.unwrap();
        assert_eq!(pair.refresh_token, "generated-refresh-http");
        let body = mock.last_refresh_body.lock().unwrap().clone().unwrap();
        assert!(body.contains("generated-refresh-old"));
    }

    #[tokio::test]
    async fn refresh_401_is_rejected_so_the_stored_token_can_be_cleared() {
        let origin = serve(mock_with(200, 401, 200)).await;
        let api = HttpSessionApi::new(&origin).unwrap();
        assert_eq!(
            api.refresh("generated-refresh-old").await.unwrap_err(),
            RefreshError::Rejected
        );
    }

    #[tokio::test]
    async fn refresh_5xx_is_unavailable_not_a_verdict_on_the_session() {
        let origin = serve(mock_with(200, 503, 200)).await;
        let api = HttpSessionApi::new(&origin).unwrap();
        assert_eq!(
            api.refresh("generated-refresh-old").await.unwrap_err(),
            RefreshError::Unavailable
        );
    }

    #[tokio::test]
    async fn refresh_timeout_is_unavailable() {
        let mock = mock_with(200, 200, 200);
        *mock.refresh_delay.lock().unwrap() = Duration::from_secs(2);
        let origin = serve(mock).await;
        let api = HttpSessionApi::with_timeout(&origin, Duration::from_millis(150)).unwrap();
        assert_eq!(
            api.refresh("generated-refresh-old").await.unwrap_err(),
            RefreshError::Unavailable
        );
    }

    #[tokio::test]
    async fn refresh_network_failure_is_unavailable_and_not_fatal() {
        // Nothing listens on this port.
        let api =
            HttpSessionApi::with_timeout("http://127.0.0.1:9", Duration::from_secs(2)).unwrap();
        assert_eq!(
            api.refresh("generated-refresh-old").await.unwrap_err(),
            RefreshError::Unavailable
        );
    }

    #[tokio::test]
    async fn logout_sends_bearer_header_and_refresh_token_body() {
        let mock = mock_with(200, 200, 200);
        let origin = serve(Arc::clone(&mock)).await;
        let api = HttpSessionApi::new(&origin).unwrap();
        api.revoke("generated-access-http", "generated-refresh-http")
            .await
            .unwrap();
        assert_eq!(
            mock.last_logout_auth.lock().unwrap().as_deref(),
            Some("Bearer generated-access-http")
        );
        let body = mock.last_logout_body.lock().unwrap().clone().unwrap();
        assert!(body.contains("generated-refresh-http"));
    }

    #[tokio::test]
    async fn logout_server_error_is_reported_without_token_text() {
        let origin = serve(mock_with(200, 200, 500)).await;
        let api = HttpSessionApi::new(&origin).unwrap();
        let error = api
            .revoke("generated-access-http", "generated-refresh-http")
            .await
            .unwrap_err();
        let text = error.to_string();
        assert!(!text.contains("generated-access-http"));
        assert!(!text.contains("generated-refresh-http"));
    }
}
