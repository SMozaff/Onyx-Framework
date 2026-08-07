//! Authority & actor identity primitives.
//! Source: Handover Document §1 "authority" (lines 117-158),
//! plus Architectural Ruling (Team 2 Initiation) for VerifiedAuthority,
//! PolicyDecisionSet, DataClassification, AuthError, SecretRef (Increment 7 stubs).

use crate::identifiers::{DeviceId, ObjectId, OrganizationId, UserId};
use crate::time::Timestamp;
use serde::{Deserialize, Serialize};

/// Identifies the actor performing an operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorContext {
    pub user_id: UserId,
    pub device_id: DeviceId,
    pub organization_id: OrganizationId,
}

/// Type of authority proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofType {
    Jwt,
    Paseto,
    DelegationChain,
}

/// Scope of an authority proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityScope {
    pub organization_id: OrganizationId,
    pub object_type: String,
    pub object_id: Option<ObjectId>,
    pub command_types: Vec<String>,
    pub delegation_depth: u32,
}

/// Verifiable, scope-limited evidence of authority.
///
/// Constraints (per contract, enforced by the security layer, NOT this stub):
/// - Scope must include organization_id
/// - Must be cryptographically verifiable
/// - Expired proofs MUST be rejected
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityProof {
    pub proof_type: ProofType,
    pub scope: AuthorityScope,
    pub issued_at: Timestamp,
    pub expires_at: Timestamp,
    pub signature: Option<Vec<u8>>,
}

/// Result of verifying an AuthorityProof. Stub per Architectural Ruling;
/// full implementation lands in Increment 7 (security-application).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedAuthority;

/// Aggregated policy engine outcomes for a decision. Stub per Architectural
/// Ruling; full implementation lands in Increment 7.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecisionSet;

/// Data sensitivity classification, used in AuditMetadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataClassification {
    Public,
    Internal,
    Confidential,
    Restricted,
}

/// Authority/authentication verification errors. Per Architectural Ruling
/// (Team 2 Initiation); owned by security-application (Increment 7), stubbed
/// here so dependent crates compile.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum AuthError {
    #[error("invalid signature")]
    InvalidSignature,
    #[error("authority proof expired")]
    Expired,
    #[error("scope mismatch")]
    ScopeMismatch,
    #[error("authority revoked")]
    Revoked,
    #[error("delegation chain too deep")]
    DelegationChainTooDeep,
    #[error("organization mismatch")]
    OrganizationMismatch,
}

/// Reference to a secret, resolved via a SecretProvider. Per Architectural
/// Ruling; owned by security-application (Increment 7), stubbed here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRef(pub String);
