//! Bearer token holder for the mobile core's cloud uploads. The Kotlin layer
//! sets it after login and each refresh, and clears it on sign-out. While it is
//! empty, uploads report missing credentials and rows stay pending.

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use sync_transport::placeholder_types::{AuthorityError, AuthorityProvider};

#[derive(Clone, Default)]
pub struct AccessTokenCell(Arc<RwLock<Option<String>>>);

impl AccessTokenCell {
    pub fn set(&self, token: Option<String>) {
        if let Ok(mut guard) = self.0.write() {
            *guard = token;
        }
    }
}

#[async_trait]
impl AuthorityProvider for AccessTokenCell {
    async fn bearer_token(&self) -> Result<String, AuthorityError> {
        let guard = self.0.read().map_err(|_| AuthorityError::NoCredentials)?;
        guard.clone().ok_or(AuthorityError::NoCredentials)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_cell_has_no_credentials_until_set() {
        let cell = AccessTokenCell::default();
        assert!(cell.bearer_token().await.is_err());
        cell.set(Some("tok".into()));
        assert_eq!(cell.bearer_token().await.unwrap(), "tok");
        cell.set(None);
        assert!(cell.bearer_token().await.is_err());
    }
}
