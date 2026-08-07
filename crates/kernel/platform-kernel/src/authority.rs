//! Authority and actor-context types.
//!
//! In Increment 1, cryptographic proof verification is stubbed (`signature`
//! is optional and unchecked). Full verification lands in Increment 7.

use crate::identifiers::ObjectId;
use crate::time::Timestamp;
use serde::{Deserialize, Serialize};

/// Alias — a user identity is just an `ObjectId` in this bounded context.
pub type UserId = ObjectId;
/// Alias — a device identity is just an `ObjectId`.
pub type DeviceId = ObjectId;
/// Alias — an organization identity is just an `ObjectId`.
pub type OrganizationId = ObjectId;

/// The identity of the actor (user, device, organization) issuing a command.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActorContext {
    /// The user issuing the command.
    pub user_id: UserId,
    /// The device the command was issued from.
    pub device_id: DeviceId,
    /// The organization (tenant) the actor belongs to.
    pub organization_id: OrganizationId,
}

/// A proof of authority accompanying a command.
///
/// For Increment 1, `signature` is optional and unvalidated — a stub. Full
/// cryptographic validation is deferred to Increment 7 per the frozen
/// contract (§3.4).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthorityProof {
    /// The proof mechanism used (JWT, PASETO, delegation chain).
    pub proof_type: ProofType,
    /// The scope of authority this proof grants.
    pub scope: AuthorityScope,
    /// When the proof was issued.
    pub issued_at: Timestamp,
    /// When the proof expires.
    pub expires_at: Timestamp,
    /// The raw signature bytes, if present. Unvalidated stub in Increment 1.
    pub signature: Option<Vec<u8>>,
}

/// The mechanism by which an [`AuthorityProof`] is represented.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProofType {
    /// A JSON Web Token.
    Jwt,
    /// A PASETO token.
    Paseto,
    /// A chain of delegated authority grants.
    DelegationChain,
}

/// The bounded scope (tenant, object type, object instance, command set,
/// delegation depth) that an [`AuthorityProof`] grants authority over.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthorityScope {
    /// The organization this scope's authority is confined to.
    pub organization_id: OrganizationId,
    /// The domain object type this scope applies to (e.g. `"mission"`).
    pub object_type: String,
    /// If set, restricts the scope to a single object instance.
    pub object_id: Option<ObjectId>,
    /// The command types this scope authorizes.
    pub command_types: Vec<String>,
    /// How many hops of delegation this proof has traversed.
    pub delegation_depth: u32,
}

/// Placeholder for verified authority, resolved once a command's
/// `AuthorityProof` has passed validation. Full implementation lands in
/// Increment 7 (per project ruling: Inc 1 domain code calls stub methods
/// below, all of which return `true`).
///
/// # Increment 1 Ruling
/// All `can_*` methods are stubbed to return `true` unconditionally. This
/// lets `decide()` methods exercise their authority-check code paths (and
/// keeps `DomainError::Unauthorized` / `MissionError::Unauthorized` /
/// `TaskError::Unauthorized` reachable in the type system) without requiring
/// real policy evaluation, which does not exist until Increment 7.
#[derive(Clone, Debug, Default)]
pub struct VerifiedAuthority;

impl VerifiedAuthority {
    /// TODO(Inc 7): implement actual authority/policy evaluation.
    /// Increment 1 stub: always authorized.
    pub fn is_authorized(&self, _scope_hint: &str) -> bool {
        true
    }
}

/// Placeholder for the outcome of policy evaluation (rate limits, quotas,
/// business-rule policies external to the aggregate). Full implementation
/// lands in Increment 7.
#[derive(Clone, Debug, Default)]
pub struct PolicyDecisionSet;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_authority_stub_always_authorizes_in_inc1() {
        let auth = VerifiedAuthority;
        assert!(auth.is_authorized("mission.pause"));
        assert!(auth.is_authorized("anything"));
    }
}
