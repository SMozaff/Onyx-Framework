//! In-memory session authority for the native Slint host.
//!
//! Access and refresh tokens are published as one state value. Refresh calls
//! are single-flight and cancellation-safe: the network rotation runs in a
//! detached task so cancelling one consumer does not discard a successful
//! rotation. Logout disables local authority immediately, waits for an
//! in-flight rotation, then revokes the latest pair accepted by the server.
//! Tokens are never persisted or formatted into diagnostics.

use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex as StdMutex,
    },
    time::Duration,
};

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::{Mutex, Notify, RwLock};

use crate::authority::{SessionSourceError, SessionTokenSource};

const REFRESH_SKEW: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
struct TokenPair {
    access_token: String,
    refresh_token: String,
    access_expires_at: tokio::time::Instant,
}

#[derive(Default)]
struct SessionState {
    tokens: Option<TokenPair>,
    /// Changes for each authenticated session, but not for each token rotation.
    session_generation: u64,
    /// While true, callers get no authority while logout drains a refresh.
    logging_out: bool,
}

pub struct SessionService {
    api_origin: Arc<str>,
    client: reqwest::Client,
    state: Arc<RwLock<SessionState>>,
    refresh_gate: Arc<Mutex<()>>,
}

#[derive(Deserialize)]
struct RefreshResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

impl SessionService {
    pub fn new(api_origin: impl Into<String>) -> Result<Arc<Self>, SessionSourceError> {
        let api_origin = api_origin.into().trim_end_matches('/').to_owned();
        let parsed = reqwest::Url::parse(&api_origin).map_err(|_| SessionSourceError)?;
        if !matches!(parsed.scheme(), "https" | "http")
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !matches!(parsed.path(), "" | "/")
        {
            return Err(SessionSourceError);
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| SessionSourceError)?;

        Ok(Arc::new(Self {
            api_origin: Arc::from(api_origin),
            client,
            state: Arc::new(RwLock::new(SessionState::default())),
            refresh_gate: Arc::new(Mutex::new(())),
        }))
    }

    /// Installs a freshly authenticated token pair with one atomic state update.
    pub async fn install_tokens(
        &self,
        access_token: String,
        refresh_token: String,
        expires_in: u64,
    ) -> Result<(), SessionSourceError> {
        if access_token.trim().is_empty() || refresh_token.trim().is_empty() || expires_in == 0 {
            return Err(SessionSourceError);
        }

        let mut state = self.state.write().await;
        state.session_generation = state.session_generation.wrapping_add(1);
        state.logging_out = false;
        state.tokens = Some(TokenPair {
            access_token,
            refresh_token,
            access_expires_at: tokio::time::Instant::now() + Duration::from_secs(expires_in),
        });
        Ok(())
    }

    /// Revokes the latest pair after immediately denying further local use.
    ///
    /// The API's logout contract requires both an access-token bearer header
    /// and the refresh token in the JSON body. Waiting for the refresh gate is
    /// important: if refresh rotation is already in flight, logout must revoke
    /// the newly rotated refresh token rather than the now-revoked old value.
    pub async fn logout(&self) {
        let logout_generation = {
            let mut state = self.state.write().await;
            state.logging_out = true;
            state.session_generation
        };

        // A refresh operation holds this gate across its HTTP request and
        // publication. Local authority has already been disabled above.
        let _refresh_guard = self.refresh_gate.lock().await;
        let pair = {
            let mut state = self.state.write().await;
            // A new login during logout owns a newer generation and must win.
            if state.session_generation != logout_generation || !state.logging_out {
                return;
            }
            state.session_generation = state.session_generation.wrapping_add(1);
            state.logging_out = false;
            state.tokens.take()
        };

        if let Some(pair) = pair {
            // The server response is deliberately ignored: local logout has
            // already completed. Do not log credentials or transport details.
            let _ = self
                .client
                .post(format!("{}/api/auth/logout", self.api_origin))
                .bearer_auth(pair.access_token)
                .json(&serde_json::json!({ "refresh_token": pair.refresh_token }))
                .send()
                .await;
        }
    }

    async fn access_token(&self) -> Result<Option<String>, SessionSourceError> {
        {
            let state = self.state.read().await;
            if state.logging_out {
                return Ok(None);
            }
            let Some(pair) = state.tokens.as_ref() else {
                return Ok(None);
            };
            if pair.access_expires_at > tokio::time::Instant::now() + REFRESH_SKEW {
                return Ok(Some(pair.access_token.clone()));
            }
        }

        // Do not await the network refresh inside the caller's cancellable
        // future. Dropping this JoinHandle detaches the rotation task; the
        // task still atomically publishes the rotated pair before releasing
        // the single-flight gate. This is essential because the server revokes
        // the old refresh token as part of successful rotation.
        let state = Arc::clone(&self.state);
        let refresh_gate = Arc::clone(&self.refresh_gate);
        let client = self.client.clone();
        let api_origin = Arc::clone(&self.api_origin);
        tokio::spawn(async move {
            coordinated_refresh(state, refresh_gate, client, api_origin).await
        })
        .await
        .map_err(|_| SessionSourceError)?
    }
}

async fn invalidate_if_current(state: &RwLock<SessionState>, session_generation: u64) {
    let mut state = state.write().await;
    // During logout, preserve the current pair until logout owns the refresh
    // gate; it may be needed to revoke a rotation that just completed.
    if state.session_generation == session_generation && !state.logging_out {
        state.session_generation = state.session_generation.wrapping_add(1);
        state.tokens = None;
    }
}

async fn coordinated_refresh(
    state: Arc<RwLock<SessionState>>,
    refresh_gate: Arc<Mutex<()>>,
    client: reqwest::Client,
    api_origin: Arc<str>,
) -> Result<Option<String>, SessionSourceError> {
    let _single_flight = refresh_gate.lock().await;
    let (refresh_token, session_generation) = {
        let state = state.read().await;
        if state.logging_out {
            return Ok(None);
        }
        let Some(pair) = state.tokens.as_ref() else {
            return Ok(None);
        };
        if pair.access_expires_at > tokio::time::Instant::now() + REFRESH_SKEW {
            return Ok(Some(pair.access_token.clone()));
        }
        (pair.refresh_token.clone(), state.session_generation)
    };

    let response = match client
        .post(format!("{api_origin}/api/auth/refresh"))
        .json(&serde_json::json!({ "refresh_token": refresh_token }))
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            invalidate_if_current(&state, session_generation).await;
            return Err(SessionSourceError);
        }
    };

    if !response.status().is_success() {
        invalidate_if_current(&state, session_generation).await;
        return Err(SessionSourceError);
    }

    let rotated: RefreshResponse = match response.json().await {
        Ok(response) => response,
        Err(_) => {
            invalidate_if_current(&state, session_generation).await;
            return Err(SessionSourceError);
        }
    };
    if rotated.access_token.trim().is_empty()
        || rotated.refresh_token.trim().is_empty()
        || rotated.expires_in == 0
    {
        invalidate_if_current(&state, session_generation).await;
        return Err(SessionSourceError);
    }

    let mut state = state.write().await;
    // A new authenticated session may have replaced this one during I/O.
    if state.session_generation != session_generation || state.tokens.is_none() {
        return Ok(None);
    }

    // Publish both rotated values under one write lock. When logout is pending,
    // publish them for the revocation request, but never return the access token
    // to a caller after logout disabled local authority.
    state.tokens = Some(TokenPair {
        access_token: rotated.access_token.clone(),
        refresh_token: rotated.refresh_token,
        access_expires_at: tokio::time::Instant::now()
            + Duration::from_secs(rotated.expires_in),
    });
    if state.logging_out {
        Ok(None)
    } else {
        Ok(Some(rotated.access_token))
    }
}

#[async_trait]
impl SessionTokenSource for SessionService {
    async fn current_access_token(&self) -> Result<Option<String>, SessionSourceError> {
        self.access_token().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        extract::State,
        http::{HeaderMap, StatusCode},
        routing::post,
        Json, Router,
    };
    use serde_json::{json, Value};
    use tokio::net::TcpListener;

    #[derive(Default)]
    struct MockApi {
        refresh_calls: AtomicUsize,
        logout_calls: AtomicUsize,
        block_refresh: AtomicBool,
        fail_refresh: AtomicBool,
        refresh_entered: Notify,
        release_refresh: Notify,
        last_refresh_token: StdMutex<Option<String>>,
        last_logout_refresh: StdMutex<Option<String>>,
        last_logout_bearer: StdMutex<Option<String>>,
    }

    async fn refresh_handler(
        State(state): State<Arc<MockApi>>,
        Json(payload): Json<Value>,
    ) -> (StatusCode, Json<Value>) {
        state.refresh_calls.fetch_add(1, Ordering::SeqCst);
        *state.last_refresh_token.lock().unwrap() = payload
            .get("refresh_token")
            .and_then(Value::as_str)
            .map(str::to_owned);
        state.refresh_entered.notify_one();

        if state.block_refresh.load(Ordering::SeqCst) {
            state.release_refresh.notified().await;
        }
        if state.fail_refresh.load(Ordering::SeqCst) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "message": "ACCESS_SECRET_DO_NOT_LEAK REFRESH_SECRET_DO_NOT_LEAK"
                })),
            );
        }

        (
            StatusCode::OK,
            Json(json!({
                "access_token": "access-rotated",
                "refresh_token": "refresh-rotated",
                "expires_in": 3600
            })),
        )
    }

    async fn logout_handler(
        State(state): State<Arc<MockApi>>,
        headers: HeaderMap,
        Json(payload): Json<Value>,
    ) -> StatusCode {
        state.logout_calls.fetch_add(1, Ordering::SeqCst);
        *state.last_logout_refresh.lock().unwrap() = payload
            .get("refresh_token")
            .and_then(Value::as_str)
            .map(str::to_owned);
        *state.last_logout_bearer.lock().unwrap() = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        StatusCode::OK
    }

    async fn start_mock_api(block_refresh: bool) -> (String, Arc<MockApi>) {
        let state = Arc::new(MockApi {
            block_refresh: AtomicBool::new(block_refresh),
            ..MockApi::default()
        });
        let app = Router::new()
            .route("/api/auth/refresh", post(refresh_handler))
            .route("/api/auth/logout", post(logout_handler))
            .with_state(Arc::clone(&state));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{address}"), state)
    }

    async fn wait_for_logout_intent(service: &SessionService) {
        loop {
            if service.state.read().await.logging_out {
                return;
            }
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn logout_invalidates_local_authority_and_uses_api_contract() {
        let (origin, api) = start_mock_api(false).await;
        let service = SessionService::new(origin).unwrap();
        service
            .install_tokens("access-test".into(), "refresh-test".into(), 3600)
            .await
            .unwrap();

        service.logout().await;

        assert_eq!(service.current_access_token().await.unwrap(), None);
        assert_eq!(api.logout_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            api.last_logout_refresh.lock().unwrap().as_deref(),
            Some("refresh-test")
        );
        assert_eq!(
            api.last_logout_bearer.lock().unwrap().as_deref(),
            Some("Bearer access-test")
        );
    }

    #[tokio::test]
    async fn rejects_blank_or_zero_lifetime_tokens() {
        let service = SessionService::new("https://example.invalid").unwrap();
        assert!(
            service
                .install_tokens(" ".into(), "refresh".into(), 3600)
                .await
                .is_err()
        );
        assert!(
            service
                .install_tokens("access".into(), "refresh".into(), 0)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn rejects_non_http_api_origins_and_url_credentials() {
        assert!(SessionService::new("file:///tmp/onyx").is_err());
        assert!(SessionService::new("ftp://example.invalid").is_err());
        assert!(SessionService::new("https://user:password@example.invalid").is_err());
        assert!(SessionService::new("https://example.invalid/api").is_err());
    }

    #[tokio::test]
    async fn concurrent_expired_token_reads_share_one_rotation() {
        let (origin, api) = start_mock_api(false).await;
        let service = SessionService::new(origin).unwrap();
        service
            .install_tokens("access-old".into(), "refresh-old".into(), 1)
            .await
            .unwrap();

        let first = Arc::clone(&service);
        let second = Arc::clone(&service);
        let (first, second) = tokio::join!(
            async move { first.current_access_token().await },
            async move { second.current_access_token().await }
        );

        assert_eq!(first.unwrap().as_deref(), Some("access-rotated"));
        assert_eq!(second.unwrap().as_deref(), Some("access-rotated"));
        assert_eq!(api.refresh_calls.load(Ordering::SeqCst), 1);
        let state = service.state.read().await;
        let pair = state.tokens.as_ref().unwrap();
        assert_eq!(pair.access_token, "access-rotated");
        assert_eq!(pair.refresh_token, "refresh-rotated");
    }

    #[tokio::test]
    async fn logout_during_refresh_revokes_the_rotated_pair() {
        let (origin, api) = start_mock_api(true).await;
        let service = SessionService::new(origin).unwrap();
        service
            .install_tokens("access-old".into(), "refresh-old".into(), 1)
            .await
            .unwrap();

        let refresh_service = Arc::clone(&service);
        let refresh = tokio::spawn(async move { refresh_service.current_access_token().await });
        api.refresh_entered.notified().await;

        let logout_service = Arc::clone(&service);
        let logout = tokio::spawn(async move { logout_service.logout().await });
        wait_for_logout_intent(&service).await;
        assert_eq!(service.current_access_token().await.unwrap(), None);

        api.release_refresh.notify_one();
        assert_eq!(refresh.await.unwrap().unwrap(), None);
        logout.await.unwrap();

        assert_eq!(api.refresh_calls.load(Ordering::SeqCst), 1);
        assert_eq!(api.logout_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            api.last_logout_refresh.lock().unwrap().as_deref(),
            Some("refresh-rotated")
        );
        assert_eq!(
            api.last_logout_bearer.lock().unwrap().as_deref(),
            Some("Bearer access-rotated")
        );
        assert_eq!(service.state.read().await.tokens.is_none(), true);
    }

    #[tokio::test]
    async fn cancelling_a_refresh_caller_does_not_discard_rotation() {
        let (origin, api) = start_mock_api(true).await;
        let service = SessionService::new(origin).unwrap();
        service
            .install_tokens("access-old".into(), "refresh-old".into(), 1)
            .await
            .unwrap();

        let refresh_service = Arc::clone(&service);
        let caller = tokio::spawn(async move { refresh_service.current_access_token().await });
        api.refresh_entered.notified().await;
        caller.abort();
        api.release_refresh.notify_one();

        // The detached refresh owns the gate and publishes before this second
        // reader can enter its own coordinated refresh task.
        assert_eq!(
            service.current_access_token().await.unwrap().as_deref(),
            Some("access-rotated")
        );
        assert_eq!(api.refresh_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn refresh_errors_do_not_disclose_tokens() {
        let (origin, api) = start_mock_api(false).await;
        api.fail_refresh.store(true, Ordering::SeqCst);
        let service = SessionService::new(origin).unwrap();
        service
            .install_tokens(
                "ACCESS_SECRET_DO_NOT_LEAK".into(),
                "REFRESH_SECRET_DO_NOT_LEAK".into(),
                1,
            )
            .await
            .unwrap();

        let error = service.current_access_token().await.unwrap_err();
        let diagnostic = format!("{error:?}");
        assert!(!diagnostic.contains("ACCESS_SECRET_DO_NOT_LEAK"));
        assert!(!diagnostic.contains("REFRESH_SECRET_DO_NOT_LEAK"));
        assert!(!diagnostic.contains("ACCESS_SECRET_DO_NOT_LEAK REFRESH_SECRET_DO_NOT_LEAK"));
        assert!(service.state.read().await.tokens.is_none());
    }
}
