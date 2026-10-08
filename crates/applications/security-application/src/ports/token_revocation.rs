//! Session/token revocation, made durable and shared across replicas.
//!
//! # Provenance
//! Audit finding H-02. `api-server`'s `ApiState::revoked_tokens` was a bare
//! `Arc<RwLock<HashSet<String>>>` — real, in-process memory, private to
//! whichever replica handled the revoking request. In the actual production
//! topology (multiple horizontally-scaled API replicas behind a load
//! balancer), a token revoked via replica A remained fully valid against
//! replica B, C, ... for as long as its `exp` claim allowed. This port
//! replaces that with a store every replica reads and writes, so a
//! revocation performed anywhere is visible everywhere immediately.
//!
//! # Design: token-level revocation *and* a per-user watermark
//! Two related but distinct operations are modeled, not one:
//!
//! - [`TokenRevocationStore::revoke_token`] / [`TokenRevocationStore::is_token_revoked`]
//!   revoke one specific, already-issued token by its hash. This matches
//!   `logout` and refresh-token rotation's existing behavior exactly: the
//!   caller hands back the one token it holds, and only that one stops
//!   working.
//! - [`TokenRevocationStore::revoke_all_for_user`] / [`TokenRevocationStore::user_revoked_before`]
//!   invalidate *every* token a user has outstanding as of a point in time,
//!   without the server ever having tracked which individual tokens exist.
//!   This is required for user deactivation and admin-driven password
//!   resets: `api-server`'s `set_user_password` previously did nothing to
//!   existing sessions at all (see its own doc comment, pointing at this
//!   exact finding), so a stolen password could be "reset" while the
//!   attacker's already-issued tokens kept working for up to their full
//!   TTL. Individual-token tracking alone cannot fix this — the server
//!   would have to enumerate every refresh token it ever issued for that
//!   user, which it does not retain. A watermark compared against each
//!   token's own `iat` claim solves it in one write, independent of how
//!   many sessions the user has.
use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TokenRevocationError {
    #[error("token revocation store infrastructure failed: {0}")]
    Infrastructure(String),
}

#[async_trait]
pub trait TokenRevocationStore: Send + Sync {
    /// Revoke one token, identified by a hash of its raw value (never the
    /// raw token itself — see callers' `token_hash` helper). `revoked_at`
    /// is unix seconds, recorded for observability/cleanup, not read back
    /// by `is_token_revoked`.
    async fn revoke_token(
        &self,
        token_hash: &str,
        revoked_at: u64,
    ) -> Result<(), TokenRevocationError>;

    /// Whether this specific token has been individually revoked.
    async fn is_token_revoked(&self, token_hash: &str) -> Result<bool, TokenRevocationError>;

    /// Invalidate every token issued (`iat`) before `revoked_before` (unix
    /// seconds) for this user. Idempotent and monotonic: an
    /// implementation must never move an existing watermark backwards, so
    /// concurrent calls (e.g. two rapid password resets) can't
    /// accidentally re-validate a window that was already closed.
    async fn revoke_all_for_user(
        &self,
        user_id: &str,
        revoked_before: u64,
    ) -> Result<(), TokenRevocationError>;

    /// The watermark set by `revoke_all_for_user`, if this user has ever
    /// had one recorded. A token whose `iat` is strictly before this
    /// value must be treated as revoked.
    async fn user_revoked_before(&self, user_id: &str)
        -> Result<Option<u64>, TokenRevocationError>;
}
