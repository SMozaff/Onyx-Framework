//! Admin in-memory session (blocker Q-01).
//!
//! Contract (approved 2026-10-27, decision log Q-01):
//! - Tokens live in memory only. Nothing is written to disk, the OS keyring,
//!   or any other store. A new `AdminSession` always starts signed out.
//! - Tokens are never shown in `Debug` output, logs, or UI state. This module
//!   has no `Display`/`Serialize` impl for tokens.
//! - Access tokens expire. An expired token fails closed: the session is
//!   cleared and the caller must sign in again.
//! - A refresh failure, a revocation signal, or an explicit sign-out clears
//!   the session immediately (`SessionPolicy::for_client(Admin)` →
//!   `FailClosedRequireLogin`).
//! - Cleared token strings are zeroized.

use zeroize::Zeroize;

use crate::session_policy::{ClientKind, RefreshFailure, SessionPolicy};

/// Refresh this many seconds before the access token expires.
pub const REFRESH_MARGIN_SECS: i64 = 60;

/// A token pair as returned by the server, with the access-token expiry in
/// unix seconds.
pub struct AdminTokens {
    access: String,
    refresh: String,
    access_expires_at: i64,
}

impl AdminTokens {
    pub fn new(access: String, refresh: String, access_expires_at: i64) -> Self {
        Self {
            access,
            refresh,
            access_expires_at,
        }
    }

    pub fn access_expires_at(&self) -> i64 {
        self.access_expires_at
    }
}

impl std::fmt::Debug for AdminTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdminTokens")
            .field("access", &"<redacted>")
            .field("refresh", &"<redacted>")
            .field("access_expires_at", &self.access_expires_at)
            .finish()
    }
}

impl Drop for AdminTokens {
    fn drop(&mut self) {
        self.access.zeroize();
        self.refresh.zeroize();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminSessionError {
    /// No token in memory. Sign in required.
    NotSignedIn,
    /// Access token expired. The session was cleared; sign in required.
    Expired,
}

#[derive(Debug)]
pub struct AdminSession {
    tokens: Option<AdminTokens>,
}

impl Default for AdminSession {
    fn default() -> Self {
        Self::new()
    }
}

impl AdminSession {
    /// Always starts signed out. There is no restore path by design.
    pub fn new() -> Self {
        // Guard: the Admin policy must fail closed on refresh failure.
        debug_assert_eq!(
            SessionPolicy::for_client(ClientKind::Admin).on_refresh_failure,
            RefreshFailure::FailClosedRequireLogin
        );
        Self { tokens: None }
    }

    pub fn is_signed_in(&self) -> bool {
        self.tokens.is_some()
    }

    /// Store a new token pair after a successful sign-in. Replaces any
    /// previous pair, which is zeroized on drop.
    pub fn sign_in(&mut self, tokens: AdminTokens) {
        self.tokens = Some(tokens);
    }

    /// Returns the access token for a request at `now` (unix seconds).
    /// An expired token clears the session and fails closed.
    pub fn access_token(&mut self, now: i64) -> Result<&str, AdminSessionError> {
        let expires_at = match &self.tokens {
            None => return Err(AdminSessionError::NotSignedIn),
            Some(t) => t.access_expires_at,
        };
        if now >= expires_at {
            self.clear();
            return Err(AdminSessionError::Expired);
        }
        match &self.tokens {
            Some(t) => Ok(t.access.as_str()),
            None => Err(AdminSessionError::NotSignedIn),
        }
    }

    /// True when the access token should be refreshed at `now`.
    pub fn needs_refresh(&self, now: i64) -> bool {
        self.tokens
            .as_ref()
            .is_some_and(|t| now >= t.access_expires_at - REFRESH_MARGIN_SECS)
    }

    /// The refresh token, if signed in. Used only to call the refresh endpoint.
    pub fn refresh_token(&self) -> Option<&str> {
        self.tokens.as_ref().map(|t| t.refresh.as_str())
    }

    /// A refresh succeeded: replace the token pair.
    pub fn refresh_succeeded(&mut self, tokens: AdminTokens) {
        self.sign_in(tokens);
    }

    /// A refresh failed: fail closed and clear the session.
    pub fn refresh_failed(&mut self) {
        self.clear();
    }

    /// The server reported the token as revoked (for example, a 401 after
    /// revocation): clear the session.
    pub fn revoked(&mut self) {
        self.clear();
    }

    /// Explicit sign-out: clear the session.
    pub fn sign_out(&mut self) {
        self.clear();
    }

    fn clear(&mut self) {
        // Dropping `AdminTokens` zeroizes both strings.
        self.tokens = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(expires_at: i64) -> AdminTokens {
        AdminTokens::new("access-secret".into(), "refresh-secret".into(), expires_at)
    }

    #[test]
    fn starts_signed_out_and_has_no_restore_path() {
        let mut s = AdminSession::new();
        assert!(!s.is_signed_in());
        assert_eq!(s.access_token(0), Err(AdminSessionError::NotSignedIn));
        assert_eq!(s.refresh_token(), None);
    }

    #[test]
    fn access_token_is_returned_before_expiry() {
        let mut s = AdminSession::new();
        s.sign_in(pair(1_000));
        assert_eq!(s.access_token(999), Ok("access-secret"));
    }

    #[test]
    fn expired_token_fails_closed_and_clears_session() {
        let mut s = AdminSession::new();
        s.sign_in(pair(1_000));
        assert_eq!(s.access_token(1_000), Err(AdminSessionError::Expired));
        assert!(!s.is_signed_in());
        assert_eq!(s.access_token(1_000), Err(AdminSessionError::NotSignedIn));
    }

    #[test]
    fn needs_refresh_inside_margin_before_expiry() {
        let mut s = AdminSession::new();
        s.sign_in(pair(1_000));
        assert!(!s.needs_refresh(1_000 - REFRESH_MARGIN_SECS - 1));
        assert!(s.needs_refresh(1_000 - REFRESH_MARGIN_SECS));
    }

    #[test]
    fn refresh_success_replaces_tokens() {
        let mut s = AdminSession::new();
        s.sign_in(pair(1_000));
        s.refresh_succeeded(AdminTokens::new(
            "new-access".into(),
            "new-refresh".into(),
            2_000,
        ));
        assert_eq!(s.access_token(1_500), Ok("new-access"));
        assert_eq!(s.refresh_token(), Some("new-refresh"));
    }

    #[test]
    fn refresh_failure_clears_session() {
        let mut s = AdminSession::new();
        s.sign_in(pair(1_000));
        s.refresh_failed();
        assert!(!s.is_signed_in());
    }

    #[test]
    fn revocation_clears_session() {
        let mut s = AdminSession::new();
        s.sign_in(pair(1_000));
        s.revoked();
        assert!(!s.is_signed_in());
    }

    #[test]
    fn sign_out_clears_session() {
        let mut s = AdminSession::new();
        s.sign_in(pair(1_000));
        s.sign_out();
        assert!(!s.is_signed_in());
        assert_eq!(s.access_token(0), Err(AdminSessionError::NotSignedIn));
    }

    #[test]
    fn debug_output_never_contains_tokens() {
        let mut s = AdminSession::new();
        s.sign_in(pair(1_000));
        let dbg = format!("{s:?}");
        assert!(!dbg.contains("access-secret"));
        assert!(!dbg.contains("refresh-secret"));
        let tdbg = format!("{:?}", pair(5));
        assert!(!tdbg.contains("access-secret"));
        assert!(!tdbg.contains("refresh-secret"));
    }
}
