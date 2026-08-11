//! Value types for the Policy bounded context. Source: Part I §4.18.

use platform_kernel::ObjectId;
use serde::{Deserialize, Serialize};

/// The identity of a Policy aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PolicyId(pub ObjectId);

impl PolicyId {
    /// Generates a new random policy identifier.
    pub fn new_random() -> Self {
        Self(ObjectId::new_random())
    }
}

/// The identity of a LegalHold aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LegalHoldId(pub ObjectId);

impl LegalHoldId {
    /// Generates a new random legal hold identifier.
    pub fn new_random() -> Self {
        Self(ObjectId::new_random())
    }
}

/// What kind of governance rule a `Policy` encodes (§4.18.2's
/// "authorization, lifecycle, approval, retention, notification,
/// automation, and data-governance rules"). Kept as a small closed set
/// for Phase 1's actual consumers (admin feature toggles and thresholds)
/// rather than the full open vocabulary the blueprint's prose implies —
/// new variants are additive and do not require migrating existing
/// published `PolicyVersion`s, since each version's rules are opaque
/// `PolicyRule` values already scoped by `PolicyType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyType {
    /// Enables/disables a named platform feature for a scope (e.g.
    /// `"messaging.enabled"`, `"file_sharing.enabled"`).
    FeatureToggle,
    /// A numeric or duration threshold (e.g. file size cap override,
    /// message retention window, upload quota).
    Threshold,
    /// Authorization/approval-chain governance. Modeled per spec; no
    /// Phase 1 UI workflow drives this yet (evaluation still works via
    /// `EvaluatePolicy` for any caller that supplies it).
    Authorization,
    /// Data retention and deletion governance, distinct from a specific
    /// `LegalHold` — the default schedule a `RetentionClass::Standard`
    /// object follows absent an override.
    Retention,
}

/// What this policy (or a specific rule within it) applies to. Organization-
/// wide by default; narrower scopes let an org-wide default be overridden
/// for one workspace/team without republishing the whole policy — see
/// invariant "concurrent policy revisions require controlled publication"
/// (§4.18.4), which this scope narrowing does not bypass: each scope's
/// rules still only change via a new, reviewed `PolicyVersion`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyScope {
    /// Applies to the entire organization.
    Organization(ObjectId),
    /// Applies to a specific conversation/supergroup (see
    /// `communication_domain`'s forthcoming `Supergroup` conversation
    /// type) — e.g. a stricter retention window for one team's channel.
    Conversation(ObjectId),
}

/// A single named rule within a `PolicyVersion`. Deliberately opaque
/// (`key`/`value` rather than a typed enum per rule) — same rationale as
/// `communication_domain::RedactionReason` staying free text: this crate
/// defines the governance *mechanism* (versioning, publication, immutability,
/// evaluation), not every rule any admin will ever want, and a closed Rust
/// enum here would require a crate release for every new setting an admin
/// wants to add.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    /// What category of rule this is.
    pub rule_type: PolicyType,
    /// The rule's identifier within its policy (e.g.
    /// `"messaging.enabled"`, `"file.max_size_bytes"`).
    pub key: String,
    /// The rule's value, serialized (bool for toggles, a number for
    /// thresholds, etc.) — validated at the call site the rule is meant
    /// for (e.g. `file-domain` still enforces its own
    /// `MAX_FILE_SIZE_BYTES` hard ceiling; a `Threshold` rule here can
    /// only ever tighten that, never loosen it past the hard cap. This
    /// crate does not know or enforce that relationship itself).
    pub value: serde_json::Value,
}

/// Why a policy violation was raised, or a legal hold applied/released.
/// Free text, same rationale as `PolicyRule::key` above.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceReason(String);

impl GovernanceReason {
    /// Constructs a `GovernanceReason`, rejecting empty input.
    pub fn new(reason: impl Into<String>) -> Result<Self, String> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err("reason must not be empty".to_string());
        }
        Ok(Self(reason))
    }

    /// The reason as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What a `LegalHold` is placed on — a target object elsewhere in the
/// system (a `FileAsset`, a `Conversation`, etc). Policy does not own or
/// mutate that object (§5.2's "prefer IDs and typed references over
/// object copies"); it only records that a hold exists and lets the
/// owning domain's own retention/deletion logic consult it.
///
/// `target_type` is an owned `String`, not `&'static str` — a
/// `LegalHold` is rehydrated from persisted/deserialized events (like
/// every other aggregate in this workspace), and `serde::Deserialize`
/// cannot produce a `'static` borrow from arbitrary input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HoldTarget {
    /// The held object's id.
    pub target_id: ObjectId,
    /// The held object's aggregate type name (e.g. `"file_asset"`,
    /// `"conversation"`), matching the convention
    /// `platform_contracts::DomainObjectRef::r#type` already uses.
    pub target_type: String,
}
