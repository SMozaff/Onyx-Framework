//! Identity persistence port.
//!
//! # Provenance
//! Introduced by the production-readiness audit (finding **H-01**): the
//! delivered `api-server` authenticated against the compile-time constants
//! `DEFAULT_USERNAME`/`DEFAULT_PASSWORD` with a plaintext `!=` comparison and
//! no user store at all. This port is the application-layer contract that
//! replaces those constants; infrastructure implementations live in
//! `security-adapter` (Postgres and SQLite).
//!
//! # Deliberate scope boundary
//! Per the audit decision log, authorization scope remains **uniform** for
//! this change: every authenticated principal continues to receive the same
//! `TokenScope` the delivered `issue_token` produced. This port therefore
//! carries identity and the single `is_admin` flag required to gate
//! user-management endpoints — it intentionally does **not** model roles or
//! per-user permissions. Adding those later is an additive change to
//! `UserRecord` and does not alter this trait's shape.
//!
//! # `is_manager` (added, Phase 1 — Desktop & Web Completion)
//! Exactly the additive change the paragraph above anticipated. A distinct
//! Manager role, separate from Admin and narrower in scope — gates
//! Policy/Settings administration (feature toggles, thresholds, legal
//! hold, etc.; see `policy-domain`) without granting the full
//! user-management power `is_admin` carries. Not a ranked "Admin >
//! Manager" hierarchy: the two flags are independent booleans, so a user
//! can be a Manager without being an Admin (the common case) or, in
//! principle, both. See `PLAN_Desktop_Web_Completion.md` §7 item 3.

use async_trait::async_trait;

/// A stored identity.
///
/// `password_hash` is a PHC-format Argon2id string (`$argon2id$v=19$m=...`),
/// never a raw digest: the KDF parameters travel with the hash so they can be
/// raised over time without invalidating existing credentials.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserRecord {
    /// Stable UUID; becomes the token's `sub` claim.
    pub user_id: String,
    pub username: String,
    /// Tenant the user belongs to; becomes the token's `organization_id`.
    pub organization_id: String,
    pub password_hash: String,
    /// Gates the user-management endpoints only. Not a general authorization
    /// role — see the scope boundary note above.
    pub is_admin: bool,
    /// Gates Policy/Settings administration. Independent of `is_admin` —
    /// see this module's `is_manager` doc note above.
    pub is_manager: bool,
    /// Soft-disable. A disabled user must fail authentication exactly as an
    /// unknown user does, without a distinguishable error.
    pub is_active: bool,
}

/// A new identity to persist. Separated from [`UserRecord`] so callers cannot
/// accidentally supply a plaintext password where a hash is expected: the
/// field is named `password_hash` in both, and hashing is performed by the
/// adapter's `PasswordHasher` before this type is constructed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewUser {
    pub user_id: String,
    pub username: String,
    pub organization_id: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub is_manager: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UserStoreError {
    #[error("username already exists")]
    DuplicateUsername,
    #[error("user not found")]
    NotFound,
    #[error("user store failed: {0}")]
    Infrastructure(String),
}

#[async_trait]
pub trait UserStore: Send + Sync {
    /// Look up by username. Returns `Ok(None)` for an unknown username so the
    /// caller can perform a dummy verification and keep the timing of
    /// "unknown user" indistinguishable from "wrong password".
    async fn find_by_username(&self, username: &str) -> Result<Option<UserRecord>, UserStoreError>;

    async fn find_by_id(&self, user_id: &str) -> Result<Option<UserRecord>, UserStoreError>;

    /// Persist a new user. Must return [`UserStoreError::DuplicateUsername`]
    /// on unique-constraint violation rather than a generic infrastructure
    /// error, so the API can map it to 409 rather than 500.
    async fn create(&self, user: NewUser) -> Result<UserRecord, UserStoreError>;

    async fn list(&self) -> Result<Vec<UserRecord>, UserStoreError>;

    async fn set_active(&self, user_id: &str, is_active: bool) -> Result<(), UserStoreError>;

    /// Grants or revokes the Manager role. Admin-only, same as every other
    /// user-management mutation — see `api_server::routes::admin`'s
    /// `require_admin` guard.
    async fn set_manager(&self, user_id: &str, is_manager: bool) -> Result<(), UserStoreError>;

    async fn set_password_hash(
        &self,
        user_id: &str,
        password_hash: &str,
    ) -> Result<(), UserStoreError>;

    /// Number of stored users. Used solely by the first-run bootstrap check,
    /// which must be able to distinguish an empty store from a populated one.
    async fn count(&self) -> Result<u64, UserStoreError>;
}
