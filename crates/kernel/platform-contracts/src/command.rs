//! The command envelope: the transport-level wrapper around every command
//! payload, carrying identity, versioning, authority, causality, and audit
//! context.

use platform_kernel::{
    ActorContext, AuthorityEpoch, AuthorityProof, CommandId, CorrelationId, DomainObjectRef,
    EventId, LifecycleEpoch, ObjectVersion, OperationId, SchemaVersion, Timestamp, VectorClock,
};
use serde::{Deserialize, Serialize};

/// Wraps a domain command payload `C` with the full transport envelope
/// required for optimistic concurrency, authority verification, and causal
/// tracing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandEnvelope<C> {
    /// Unique identifier for this command instance.
    pub command_id: CommandId,
    /// Identifier for the logical operation this command is part of.
    pub operation_id: OperationId,
    /// The command's type name (e.g. `"PauseMission"`), for routing and
    /// diagnostics.
    pub command_type: String,
    /// The schema version this command payload was encoded with.
    pub schema_version: SchemaVersion,
    /// The aggregate this command targets.
    pub target: DomainObjectRef,
    /// The aggregate version the caller expects (optimistic concurrency).
    pub expected_version: ObjectVersion,
    /// The lifecycle epoch the caller expects.
    pub expected_lifecycle_epoch: LifecycleEpoch,
    /// The authority epoch the caller expects.
    pub expected_authority_epoch: AuthorityEpoch,
    /// The actor issuing the command.
    pub actor: ActorContext,
    /// Proof that the actor is authorized to issue this command.
    pub authority_proof: AuthorityProof,
    /// When the command was issued.
    pub issued_at: Timestamp,
    /// The issuer's vector clock at the time of issuance.
    pub vector_clock: VectorClock,
    /// Correlates this command with the broader request/workflow it
    /// belongs to.
    pub correlation_id: CorrelationId,
    /// If this command was issued in response to a prior event, that
    /// event's identifier.
    pub causation_id: Option<EventId>,
    /// The command-specific payload.
    pub payload: C,
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_kernel::{AuthorityScope, ObjectId, ProofType};

    fn sample_envelope() -> CommandEnvelope<serde_json::Value> {
        let org = ObjectId::new_random();
        CommandEnvelope {
            command_id: CommandId::new_random(),
            operation_id: OperationId::new_random(),
            command_type: "TestCommand".to_string(),
            schema_version: SchemaVersion::new("1.0.0"),
            target: DomainObjectRef {
                id: ObjectId::new_random(),
                r#type: "mission".to_string(),
                organization_id: org,
            },
            expected_version: ObjectVersion(0),
            expected_lifecycle_epoch: LifecycleEpoch(0),
            expected_authority_epoch: AuthorityEpoch(0),
            actor: ActorContext {
                user_id: ObjectId::new_random(),
                device_id: ObjectId::new_random(),
                organization_id: org,
            },
            authority_proof: AuthorityProof {
                proof_type: ProofType::Jwt,
                scope: AuthorityScope {
                    organization_id: org,
                    object_type: "mission".to_string(),
                    object_id: None,
                    command_types: vec!["TestCommand".to_string()],
                    delegation_depth: 0,
                },
                issued_at: Timestamp::from_nanos(0),
                expires_at: Timestamp::from_nanos(1),
                signature: None,
            },
            issued_at: Timestamp::from_nanos(0),
            vector_clock: VectorClock::new(),
            correlation_id: CorrelationId::new_random(),
            causation_id: None,
            payload: serde_json::json!({"key": "value"}),
        }
    }

    #[test]
    fn command_envelope_serde_roundtrips() {
        let envelope = sample_envelope();
        let json = serde_json::to_string(&envelope).unwrap();
        let back: CommandEnvelope<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(envelope.command_type, back.command_type);
        assert_eq!(envelope.payload, back.payload);
    }
}
