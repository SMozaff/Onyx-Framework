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
//! # `is_manager` (added, Phase 1 — Desktop & Web Completion; superseded below)
//! Exactly the additive change the paragraph above anticipated. A distinct
//! Manager role, separate from Admin and narrower in scope — gates
//! Policy/Settings administration (feature toggles, thresholds, legal
//! hold, etc.; see `policy-domain`) without granting the full
//! user-management power `is_admin` carries. Not a ranked "Admin >
//! Manager" hierarchy: the two flags are independent booleans, so a user
//! can be a Manager without being an Admin (the common case) or, in
//! principle, both. See `PLAN_Desktop_Web_Completion.md` §7 item 3.
//!
//! # `UserClass` / `parent_user_id` (added, Phase A — User Hierarchy)
//! Per `DESIGN_User_Hierarchy_Chain_of_Authority.md` and
//! `IMPLEMENTATION_PLAN_User_Hierarchy.md` §2: a flat `is_manager`
//! boolean cannot express the confirmed class hierarchy (Top-level
//! Manager, Senior Manager, Team Leader, Supervisor, Staff — each with
//! distinct add/remove rights and observation scope), so [`UserClass`]
//! supersedes it. `is_admin` is **not** superseded — Admin remains its
//! own field, distinct in kind from `UserClass` (design doc §2's table;
//! Admin sits at the top of the *organizational* hierarchy, Allfather
//! above that is a separate, not-yet-built, platform-level concept per
//! design doc §2.4 and is deliberately not represented here — see
//! `IMPLEMENTATION_PLAN_User_Hierarchy.md` §3/B.1).
//!
//! `is_manager` is kept on this port during the migration window (the
//! underlying migration is deliberately two-step — see
//! `20260107000000_add_user_class_hierarchy.up.sql`'s header comment)
//! and is **deprecated**: new code should read/write `class` instead.
//! It will be removed once the backfill decision (what class existing
//! `is_manager = true` users become) is made and executed — see
//! `IMPLEMENTATION_PLAN_User_Hierarchy.md` §2 A.3, which flags that
//! decision to the person rather than picking a default silently.

use async_trait::async_trait;

/// The confirmed class hierarchy, per
/// `DESIGN_User_Hierarchy_Chain_of_Authority.md` §2-§2.3. Deliberately
/// excludes Admin (a separate field, see the module doc comment above)
/// and Allfather (not yet built — a distinct, global, platform-level
/// concept per design doc §2.4, out of scope for this per-organization
/// enum).
///
/// Ordered from highest to lowest organizational authority for
/// readability; **this ordering is not itself meaningful to the type
/// system** — permission checks must compare against the specific
/// class(es) that grant a given ability (see
/// `IMPLEMENTATION_PLAN_User_Hierarchy.md` §2 A.5's
/// `require_class_at_least`-style guard), not assume a simple `>`
/// ordering captures every rule (e.g. Team Leader's real-but-narrow
/// authority, design doc §2.2, does not fit a single linear rank next
/// to Senior Manager's different-shaped authority).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UserClass {
    /// One level below Admin. Can add/remove staff. Observes everything
    /// beneath their own chain. Design doc §2.
    TopLevelManager,
    /// Work-related authority only — cannot add/remove staff. Observes
    /// supergroups/subgroups of their own. Design doc §2.
    SeniorManager,
    /// Supergroup leader. Real, standing (non-delegated) authority to
    /// pre-check Todo/Target items within their supergroup — optional,
    /// parallel, not a verification gate. No add/remove, no team
    /// ownership beyond their own supergroup. Design doc §2.2.
    TeamLeader,
    /// Observes/monitors their own subgroup only. No team
    /// responsibility (that belongs to Team Leader), no verification
    /// authority, no escalation authority. Design doc §2.3.
    Supervisor,
    /// Base level. Authors Todo/Target list items (or receives them via
    /// Manager assignment — design doc §4.0.1); items require parent
    /// Manager verification. Design doc §4.
    Staff,
}

impl UserClass {
    /// The stable, lowercase-snake-case string this class is stored as
    /// in the `class` column (see the Postgres migration's `CHECK`
    /// constraint, which enumerates exactly these five values).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TopLevelManager => "top_level_manager",
            Self::SeniorManager => "senior_manager",
            Self::TeamLeader => "team_leader",
            Self::Supervisor => "supervisor",
            Self::Staff => "staff",
        }
    }

    /// Parses the stored string form back into a `UserClass`. Returns
    /// `None` for anything else, including case variants — the stored
    /// form is a fixed, lowercase wire format, not a free-text field a
    /// caller should be lenient about.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "top_level_manager" => Some(Self::TopLevelManager),
            "senior_manager" => Some(Self::SeniorManager),
            "team_leader" => Some(Self::TeamLeader),
            "supervisor" => Some(Self::Supervisor),
            "staff" => Some(Self::Staff),
            _ => None,
        }
    }
}

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
    ///
    /// **Deprecated** — superseded by [`class`](Self::class). See the
    /// module doc comment's "`UserClass` / `parent_user_id`" section for
    /// the migration status.
    pub is_manager: bool,
    /// The user's position in the organizational class hierarchy, if
    /// assigned. `None` means unclassified — distinct from an explicit
    /// `Staff` value (see the migration's own note on why this column
    /// is nullable rather than defaulting every existing row to Staff).
    pub class: Option<UserClass>,
    /// This user's parent (owning Manager) in the reporting-line tree,
    /// if any. `None` for a user with no parent (e.g. Admin, or a
    /// Top-level Manager reporting to no one within the organization).
    /// Design doc §2.1: "every staff member has exactly one true owning
    /// Manager." Does **not** account for an active staff loan (design
    /// doc §2.1's borrowing mechanism) — loans are a separate overlay,
    /// tracked elsewhere (see `IMPLEMENTATION_PLAN_User_Hierarchy.md`
    /// §4, not yet built), not a second value on this field.
    pub parent_user_id: Option<String>,
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
    /// **Deprecated** — see [`UserRecord::is_manager`]'s doc comment.
    pub is_manager: bool,
    /// See [`UserRecord::class`]'s doc comment. A new user may be
    /// created with a class already assigned (design doc §5, question 2
    /// — confirmed: class assignment is allowed both at creation time
    /// and as a later action) or left `None`.
    pub class: Option<UserClass>,
    /// See [`UserRecord::parent_user_id`]'s doc comment.
    pub parent_user_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UserStoreError {
    #[error("username already exists")]
    DuplicateUsername,
    #[error("user not found")]
    NotFound,
    #[error("user store failed: {0}")]
    Infrastructure(String),
    /// `parent_user_id` does not refer to an existing user. Application-
    /// level check — see the SQLite migration's own note that its
    /// `REFERENCES` clause is currently unenforced (no connection sets
    /// `PRAGMA foreign_keys = ON`), so this is not optional there.
    #[error("parent user does not exist")]
    ParentNotFound,
    /// Assigning `parent_user_id` would create a cycle (a user would
    /// become its own ancestor). Per
    /// `IMPLEMENTATION_PLAN_User_Hierarchy.md` §2 A.2: no database
    /// constraint can express this, so it must be checked in
    /// application code before writing.
    #[error("assigning this parent would create a cycle in the reporting line")]
    ParentCycle,
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
    ///
    /// **Deprecated** — see [`UserRecord::is_manager`]'s doc comment.
    /// Superseded by [`set_class`](Self::set_class).
    async fn set_manager(&self, user_id: &str, is_manager: bool) -> Result<(), UserStoreError>;

    /// Assigns or clears a user's class. Admin-only (design doc §1: "no
    /// one else could add or remove users or change user's class and
    /// types"). Confirmed to be usable both at user-creation time (via
    /// [`NewUser::class`]) and as a later, separate action (design doc
    /// §5 question 2) — this method is that later action.
    async fn set_class(
        &self,
        user_id: &str,
        class: Option<UserClass>,
    ) -> Result<(), UserStoreError>;

    /// Sets or clears a user's parent (owning Manager) in the
    /// reporting-line tree. Must return
    /// [`UserStoreError::ParentNotFound`] if `parent_user_id` does not
    /// exist, and [`UserStoreError::ParentCycle`] if the assignment
    /// would make `user_id` an ancestor of itself — both are
    /// application-level checks this method's implementation is
    /// responsible for making (see the SQLite migration's note that its
    /// FK is currently unenforced, and this module's own note that no
    /// database constraint can express "no cycles").
    async fn set_parent(
        &self,
        user_id: &str,
        parent_user_id: Option<&str>,
    ) -> Result<(), UserStoreError>;

    async fn set_password_hash(
        &self,
        user_id: &str,
        password_hash: &str,
    ) -> Result<(), UserStoreError>;

    /// Number of stored users. Used solely by the first-run bootstrap check,
    /// which must be able to distinguish an empty store from a populated one.
    async fn count(&self) -> Result<u64, UserStoreError>;

    /// The set of `UserClass` values, as their wire strings
    /// (`UserClass::as_str()`), currently granted mobile access within
    /// `organization_id`. Backed by the `mobile_class_access` table
    /// (per-org, per-class allow-list rows — see that migration's own
    /// doc comment). An empty result means mobile login is denied for
    /// every class in this organization until an admin adds a grant —
    /// per the explicit, restrictive-by-default product decision this
    /// table implements, absence of a row is never treated as "allow".
    async fn list_mobile_access(&self, organization_id: &str) -> Result<Vec<String>, UserStoreError>;

    /// Replaces the full set of mobile-access grants for
    /// `organization_id` with exactly `classes` (each a
    /// `UserClass::as_str()` value) in one atomic operation, so a caller
    /// setting "Staff and Supervisor only" cannot race with a concurrent
    /// read into a state where some other class is transiently also
    /// granted. Admin-only at the route layer, mirroring `set_class`.
    async fn set_mobile_access(
        &self,
        organization_id: &str,
        classes: &[String],
    ) -> Result<(), UserStoreError>;
}
