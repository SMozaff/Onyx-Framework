//! Value types for the StaffProfile aggregate. New domain, requested
//! 2026-08-13: "a full detailed profile page for each Staff member...
//! public info, and access for modifications for admin."

use platform_kernel::{ObjectId, UserId};
use serde::{Deserialize, Serialize};

/// The identity of a StaffProfile aggregate. Deliberately its own id,
/// not a reuse of `UserId` as the aggregate id directly — a profile is
/// a distinct aggregate linked to a user (see `StaffProfile::user_id`),
/// matching the confirmed product decision to keep profile data as "a
/// separate UserProfile table/aggregate, linked by user_id" rather than
/// fields directly on `users`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StaffProfileId(pub ObjectId);

impl StaffProfileId {
    /// Generates a new random profile identifier.
    pub fn new_random() -> Self {
        Self(ObjectId::new_random())
    }
}

/// Basic identity fields. Confirmed visibility: **public to everyone in
/// the organization** — no gating on this struct's fields anywhere in
/// this crate; visibility is enforced by the composing application
/// (query/projection layer), not by withholding data at the domain
/// layer, matching how every other domain in this workspace keeps
/// authority checks at the seam (`context.authority`) rather than
/// baking read-visibility into the aggregate itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasicIdentity {
    /// The staff member's full display name. Required — see `new()`'s
    /// validation.
    pub full_name: String,
    /// A reference to stored photo content — same content-addressed
    /// convention as `file-domain`/`BlobStore` (a hash-like key, not
    /// inline bytes). This crate does not depend on `file-domain` or
    /// `query-application` (domain crates don't depend on each other,
    /// §5.2), so this is a plain `Option<String>` key the composing
    /// application resolves against whatever blob store it's using —
    /// same "speaks in a domain-agnostic vocabulary" rationale as
    /// `query_application::BlobStore`'s own `BlobKey`.
    pub photo_blob_key: Option<String>,
    /// The staff member's job title, if set.
    pub job_title: Option<String>,
    /// A public-facing contact email, if set — not necessarily the
    /// same as the account's login email/username.
    pub contact_email: Option<String>,
    /// A public-facing contact phone number, if set.
    pub contact_phone: Option<String>,
}

impl BasicIdentity {
    /// Constructs a `BasicIdentity`, rejecting an empty `full_name` —
    /// every other field is optional (a brand-new profile may not have
    /// a photo or phone yet), but a profile with no name at all is not
    /// meaningfully a profile.
    pub fn new(
        full_name: impl Into<String>,
        photo_blob_key: Option<String>,
        job_title: Option<String>,
        contact_email: Option<String>,
        contact_phone: Option<String>,
    ) -> Result<Self, String> {
        let full_name = full_name.into();
        if full_name.trim().is_empty() {
            return Err("full_name must not be empty".to_string());
        }
        Ok(Self {
            full_name,
            photo_blob_key,
            job_title,
            contact_email,
            contact_phone,
        })
    }
}

/// Organizational info. Confirmed visibility: **public to everyone in
/// the organization**, same as `BasicIdentity`.
///
/// Deliberately duplicates rather than re-derives `class`/
/// `parent_user_id` from `security-application`'s `UserRecord`: this
/// crate does not depend on `security-application` (domain crates stay
/// independent, §5.2), and a profile's org-info snapshot is allowed to
/// be a point-in-time copy the composing application keeps in sync,
/// not a live join — matching how `communication_domain::HoldTarget`
/// (in `policy-domain`) references other domains' objects by id/value
/// rather than importing their types directly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationalInfo {
    /// Free-text department name — this crate does not define a closed
    /// department vocabulary (no such requirement was given), same
    /// "mechanism not every value" rationale as `policy_domain::PolicyRule::key`.
    pub department: Option<String>,
    /// A denormalized copy of the user's class label at the time this
    /// profile was last synced, for display purposes. The
    /// authoritative value remains `security_application::UserRecord::class`
    /// — this field can drift if not kept in sync by the composing
    /// application on every class change, which is a real
    /// synchronization obligation, not automatic. Flagged here rather
    /// than silently assumed to always match.
    pub class_label: Option<String>,
    /// A denormalized copy of the reporting-line parent's display name,
    /// for the same reason and with the same sync obligation as
    /// `class_label`.
    pub parent_display_name: Option<String>,
    /// Supergroup/team membership names this profile displays. Free
    /// text display strings, not `communication_domain::ConversationId`
    /// references — same domain-independence rationale.
    pub team_memberships: Vec<String>,
}

/// Work-stats section. Confirmed visibility: **Top-level Manager and
/// Admin only** — narrower than `BasicIdentity`/`OrganizationalInfo`.
/// This gating must be enforced by the composing application's query
/// layer (the same `require_class`-style check pattern already used in
/// `api-server::routes::admin`), not by this crate, which has no
/// concept of "who is asking."
///
/// # Status: no real data source yet
/// Confirmed 2026-08-13: `work-domain::Task` has no assignee field, and
/// the Todo/Target domain this was meant to summarize
/// (`IMPLEMENTATION_PLAN_User_Hierarchy.md` Phase D) is not built yet.
/// This struct's shape is therefore provisional — defined now so the
/// rest of the profile (identity, org info, the admin edit screen) is
/// not blocked on Phase D, but every field here is `Option`/empty-
/// capable specifically so "not yet available" is a real, representable
/// state, not a placeholder value that could be mistaken for a real
/// zero. The composing application must not synthesize fake numbers
/// for this struct — leave it as `WorkStats::unavailable()` until a
/// real data source exists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkStats {
    /// Count of items currently assigned to this staff member, if
    /// known.
    pub assigned_items_count: Option<u32>,
    /// Count of items this staff member has completed, if known.
    pub completed_items_count: Option<u32>,
    /// Free-text or `None` — no target-completion metric shape has been
    /// decided yet (see `IMPLEMENTATION_PLAN_User_Hierarchy.md` §5 Phase
    /// D.2's own open question on what "reaching a target" means
    /// structurally).
    pub target_summary: Option<String>,
}

impl WorkStats {
    /// The explicit "no data source exists yet" state. Composing
    /// applications should construct this rather than `WorkStats {
    /// assigned_items_count: Some(0), ... }`, which would misrepresent
    /// "zero assigned items" (a real, meaningful fact) as
    /// indistinguishable from "we don't know."
    pub fn unavailable() -> Self {
        Self {
            assigned_items_count: None,
            completed_items_count: None,
            target_summary: None,
        }
    }
}

/// A reference back to the account this profile belongs to. Plain
/// re-export of the kernel's `UserId` rather than a newtype — this
/// crate has no reason to distinguish "a user id" from "the user id a
/// profile belongs to" the way `StaffProfileId` is distinguished from a
/// generic `ObjectId`.
pub type ProfileOwnerId = UserId;
