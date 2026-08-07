use async_trait::async_trait;
use platform_kernel::{
    ActorContext, AuthorityProof, DomainObjectRef, Timestamp, VerifiedAuthority,
};
use serde::{Deserialize, Serialize};

/// Complete context bound into an AuthorityProof verification decision.
#[derive(Clone, Debug)]
pub struct AuthorityVerificationRequest {
    pub proof: AuthorityProof,
    pub actor: ActorContext,
    pub target: DomainObjectRef,
    pub command_type: String,
    pub trusted_now: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityVerificationReceipt {
    pub verified_at: Timestamp,
    pub organization_id: String,
    pub object_type: String,
    pub command_type: String,
    pub delegation_depth: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthorityVerificationError {
    #[error("authority proof signature is missing")]
    MissingSignature,
    #[error("authority proof signature is invalid")]
    InvalidSignature,
    #[error("authority proof has expired")]
    Expired,
    #[error("authority proof is not yet valid")]
    NotYetValid,
    #[error("authority proof is outside the requested organization")]
    OrganizationMismatch,
    #[error("authority proof is outside the requested object scope")]
    ObjectScopeMismatch,
    #[error("authority proof does not authorize command {0}")]
    CommandNotAuthorized(String),
    #[error("authority proof has been revoked")]
    Revoked,
    #[error("authority verification infrastructure failed: {0}")]
    Infrastructure(String),
}

#[async_trait]
pub trait AuthorityVerifier: Send + Sync {
    async fn verify(
        &self,
        request: &AuthorityVerificationRequest,
    ) -> Result<(VerifiedAuthority, AuthorityVerificationReceipt), AuthorityVerificationError>;
}
