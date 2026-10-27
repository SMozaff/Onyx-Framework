//! In-memory session authority for the native Slint host.
//!
//! Access and refresh tokens are published as one state value. Refresh calls
//! are single-flight; logout invalidates the generation before any network
//! request, so a late refresh response cannot resurrect a logged-out session.
//! Tokens are never persisted or formatted into diagnostics.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use tokio::sync::{Mutex, RwLock};
use crate::authority::{SessionSourceError, SessionTokenSource};

const REFRESH_SKEW: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
struct TokenPair {
    access_token: String,
    refresh_token: String,
    access_expires_at: tokio::time::Instant,
    generation: u64,
}

#[derive(Default)]
struct SessionState {
    tokens: Option<TokenPair>,
    generation: u64,
}

/// A real HTTP-backed session service. Credentials remain process-memory-only.
pub struct SessionService {
    api_origin: String,
    client: reqwest::Client,
    state: RwLock<SessionState>,
    refresh_gate: Mutex<()>,
}

#[derive(serde::Deserialize)]
struct RefreshResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

impl SessionService {
    pub fn new(api_origin: impl Into<String>) -> Result<Arc<Self>, SessionSourceError> {
        let api_origin = api_origin.into().trim_end_matches('/').to_owned();
        let parsed = reqwest::Url::parse(&api_origin).map_err(|_| SessionSourceError)?;
        if !matches!(parsed.scheme(), "https" | "http") || parsed.host_str().is_none() {
            return Err(SessionSourceError);
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| SessionSourceError)?;
        Ok(Arc::new(Self {
            api_origin,
            client,
            state: RwLock::new(SessionState::default()),
            refresh_gate: Mutex::new(()),
        }))
    }

    /// Installs a freshly authenticated token pair as one atomic state update.
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
        state.generation = state.generation.wrapping_add(1);
        state.tokens = Some(TokenPair {
            access_token,
            refresh_token,
            access_expires_at: tokio::time::Instant::now() + Duration::from_secs(expires_in),
            generation: state.generation,
        });
        Ok(())
    }

    /// Invalidates locally first. A refresh already in flight may finish on
    /// the server, but its result is discarded and cannot restore local auth.
    pub async fn logout(&self) {
        let refresh_token = {
            let mut state = self.state.write().await;
            state.generation = state.generation.wrapping_add(1);
            state.tokens.take().map(|pair| pair.refresh_token)
        };

        // Best-effort server revocation; never delay local logout or expose
        // transport details that might contain request metadata.
        if let Some(refresh_token) = refresh_token {
            let _ = self.client
                .post(format!("{}/api/auth/logout", self.api_origin))
                .json(&serde_json::json!({ "refresh_token": refresh_token }))
                .send()
                .await;
        }
    }

    async fn access_token(&self) -> Result<Option<String>, SessionSourceError> {
        {
            let state = self.state.read().await;
            let Some(pair) = state.tokens.as_ref() else { return Ok(None) };
            if pair.access_expires_at > tokio::time::Instant::now() + REFRESH_SKEW {
                return Ok(Some(pair.access_token.clone()));
            }
        }

        let _single_flight = self.refresh_gate.lock().await;
        let (refresh_token, generation) = {
            let state = self.state.read().await;
            let Some(pair) = state.tokens.as_ref() else { return Ok(None) };
            if pair.access_expires_at > tokio::time::Instant::now() + REFRESH_SKEW {
                return Ok(Some(pair.access_token.clone()));
            }
            (pair.refresh_token.clone(), pair.generation)
        };

        let response = self.client
            .post(format!("{}/api/auth/refresh", self.api_origin))
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send().await.map_err(|_| SessionSourceError)?;
        if !response.status().is_success() {
            let mut state = self.state.write().await;
            if state.generation == generation {
                state.generation = state.generation.wrapping_add(1);
                state.tokens = None;
            }
            return Err(SessionSourceError);
        }
        let rotated: RefreshResponse = response.json().await.map_err(|_| SessionSourceError)?;
        if rotated.access_token.trim().is_empty()
            || rotated.refresh_token.trim().is_empty()
            || rotated.expires_in == 0
        {
            return Err(SessionSourceError);
        }

        let mut state = self.state.write().await;
        // Logout or a new login may have won while the network request ran.
        if state.generation != generation || state.tokens.is_none() {
            return Ok(None);
        }
        // Publish both rotated tokens in a single critical section. No caller
        // can observe a new access token paired with an old refresh token.
        state.generation = state.generation.wrapping_add(1);
        let next_generation = state.generation;
        state.tokens = Some(TokenPair {
            access_token: rotated.access_token.clone(),
            refresh_token: rotated.refresh_token,
            access_expires_at: tokio::time::Instant::now()
                + Duration::from_secs(rotated.expires_in),
            generation: next_generation,
        });
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

    #[tokio::test]
    async fn tokens_are_memory_only_and_logout_invalidates_authority() {
        let service = SessionService::new("https://example.invalid").unwrap();
        service.install_tokens("access-test".into(), "refresh-test".into(), 3600).await.unwrap();
        assert_eq!(service.current_access_token().await.unwrap().as_deref(), Some("access-test"));
        service.logout().await;
        assert_eq!(service.current_access_token().await.unwrap(), None);
    }

    #[tokio::test]
    async fn rejects_blank_or_zero_lifetime_tokens() {
        let service = SessionService::new("https://example.invalid").unwrap();
        assert!(service.install_tokens(" ".into(), "refresh".into(), 3600).await.is_err());
        assert!(service.install_tokens("access".into(), "refresh".into(), 0).await.is_err());
    }

    #[tokio::test]
    async fn rejects_non_http_api_origins() {
        assert!(SessionService::new("file:///tmp/onyx").is_err());
    }
}
