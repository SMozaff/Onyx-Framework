//! Session-backed relay authority for the isolated Slint desktop host.
//!
//! The provider deliberately does not own or persist credentials. A single
//! session authority remains responsible for login, refresh-token rotation,
//! logout, and invalidation. This adapter only requests the current access
//! token at connection time and fails closed when no usable token is available.
//!
//! Do not log tokens or include them in errors, UI properties, or diagnostics.

use std::sync::Arc;

use async_trait::async_trait;
use sync_transport::placeholder_types::{AuthorityError, AuthorityProvider};

/// The single session owner supplies a current access token.
///
/// Implementations must serialize refresh-token rotation and atomically publish
/// the rotated access/refresh pair before returning the new access token.
/// None means logged out or reauthentication is required. Errors are
/// intentionally opaque here so credential-bearing transport errors cannot
/// accidentally escape through AuthorityError.
#[async_trait]
pub trait SessionTokenSource: Send + Sync {
    async fn current_access_token(&self) -> Result<Option<String>, ()>;
}

/// Adapter from the session owner to the transport's relay authority contract.
///
/// This type keeps no token copy; every call consults the authoritative session
/// source. It is not a substitute for implementing refresh in that source.
pub struct SessionAuthorityProvider {
    source: Arc<dyn SessionTokenSource>,
}

impl SessionAuthorityProvider {
    pub fn new(source: Arc<dyn SessionTokenSource>) -> Self {
        Self { source }
    }
}

#[async_trait]
impl AuthorityProvider for SessionAuthorityProvider {
    async fn bearer_token(&self) -> Result<String, AuthorityError> {
        match self.source.current_access_token().await {
            Ok(Some(token)) if !token.trim().is_empty() => Ok(token),
            Ok(Some(_)) | Ok(None) => Err(AuthorityError::NoCredentials),
            Err(()) => Err(AuthorityError::RefreshFailed(
                "session authority unavailable".to_owned(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::RwLock;

    #[derive(Default)]
    struct FakeSession {
        token: RwLock<Option<String>>,
        unavailable: RwLock<bool>,
    }

    #[async_trait]
    impl SessionTokenSource for FakeSession {
        async fn current_access_token(&self) -> Result<Option<String>, ()> {
            if *self.unavailable.read().unwrap() {
                return Err(());
            }
            Ok(self.token.read().unwrap().clone())
        }
    }

    #[tokio::test]
    async fn missing_session_fails_closed() {
        let provider = SessionAuthorityProvider::new(Arc::new(FakeSession::default()));
        assert!(matches!(
            provider.bearer_token().await,
            Err(AuthorityError::NoCredentials)
        ));
    }

    #[tokio::test]
    async fn blank_token_fails_closed() {
        let session = FakeSession::default();
        *session.token.write().unwrap() = Some("  ".to_owned());
        let provider = SessionAuthorityProvider::new(Arc::new(session));
        assert!(matches!(
            provider.bearer_token().await,
            Err(AuthorityError::NoCredentials)
        ));
    }

    #[tokio::test]
    async fn reads_current_token_from_authoritative_session() {
        let session = Arc::new(FakeSession::default());
        *session.token.write().unwrap() = Some("first-token".to_owned());
        let provider = SessionAuthorityProvider::new(session.clone());

        assert_eq!(provider.bearer_token().await.unwrap(), "first-token");
        *session.token.write().unwrap() = Some("rotated-token".to_owned());
        assert_eq!(provider.bearer_token().await.unwrap(), "rotated-token");
    }

    #[tokio::test]
    async fn session_errors_are_redacted() {
        let session = FakeSession::default();
        *session.unavailable.write().unwrap() = true;
        let provider = SessionAuthorityProvider::new(Arc::new(session));
        let error = provider.bearer_token().await.unwrap_err();

        assert!(matches!(error, AuthorityError::RefreshFailed(_)));
        assert_eq!(error.to_string(), "credential refresh failed: session authority unavailable");
    }
}
