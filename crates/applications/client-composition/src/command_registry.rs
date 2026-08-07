//! `CommandRegistry` — maps a command's `command_type` string to the
//! concrete, statically-typed handler logic for that command, so the
//! Tauri/FFI boundary can dispatch a JSON-encoded
//! `CommandEnvelope<serde_json::Value>` without itself knowing about every
//! aggregate type in the system.
//!
//! # Provenance
//! Team Prompt 5 §3.2/§4.1 (illustrative, not literal — see R1/R2 in
//! `DECISIONS.md`). Built against the real `platform_contracts::AggregateRoot`
//! (generic, non-object-safe) and `api_server::handle_command` (Increment 2,
//! exposed via ruling S1).
//!
//! # Creation vs. decision dispatch (ruling R3)
//! `api_server::handle_command()` unconditionally loads an existing
//! aggregate and calls `decide()`; it has no path for `CreateMission` /
//! `CreateTask`, which Team 1's ruling (§C) routes through a dedicated
//! `Aggregate::create()` associated function instead. This registry
//! therefore holds two handler kinds:
//! - [`CreationHandler`]: no existing aggregate to load, no
//!   version/epoch check. Calls `Aggregate::create()` then
//!   `Repository::commit()` directly.
//! - [`DecisionHandler`]: wraps `api_server::handle_command()` unmodified
//!   for every command that acts on an existing aggregate.
//!
//! Each `command_type` string is registered to exactly one of the two.

use std::collections::HashMap;

use platform_contracts::CommandEnvelope;
use platform_kernel::{ActorContext, CorrelationId, LifecycleEpoch, ObjectId, ObjectVersion, OperationId, VectorClock};

/// The outcome of a successful command dispatch. Mirrors
/// `api_server::CommandResult` (a `serde_json::Value`) for decision
/// handlers; creation handlers produce the same shape by convention (see
/// [`CreationHandler`]).
pub type CommandResult = serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum CommandDispatchError {
    #[error("no handler registered for command type {0:?}")]
    UnknownCommandType(String),
    #[error("command payload did not match the shape expected by its registered handler: {0}")]
    PayloadMismatch(String),
    #[error(transparent)]
    Creation(#[from] CreationError),
    #[error(transparent)]
    Decision(#[from] api_server::CommandError),
}

#[derive(Debug, thiserror::Error)]
pub enum CreationError {
    #[error("domain error during creation: {0}")]
    Domain(String),
    #[error("persistence error during creation: {0}")]
    Persistence(String),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// A handler for a command that creates a brand-new aggregate (no existing
/// state to load, no `expected_version`/`expected_lifecycle_epoch` guard).
/// One implementation per aggregate type that has a creation command (e.g.
/// `Mission`, `Task`).
///
/// # Repository scoping (correction — see DECISIONS.md)
/// `query_application::Repository` implementations are scoped per
/// aggregate type (e.g. `SqliteRepository::new(pool, aggregate_type)` —
/// one instance per type, not one shared instance for the whole app). An
/// earlier draft of this registry held a single, registry-wide
/// `Arc<dyn Repository>`, which cannot correctly serve more than one
/// aggregate type. Each handler now owns its own aggregate-type-scoped
/// `Repository`, supplied at registration time.
#[async_trait::async_trait]
pub trait CreationHandler: Send + Sync {
    /// Handle a creation command, given its raw JSON payload and the
    /// envelope's cross-cutting fields. Returns the same `CommandResult`
    /// shape `api_server::handle_command` returns on success, so UI code
    /// need not distinguish creation from decision results.
    async fn handle_creation(
        &self,
        payload: serde_json::Value,
        actor: ActorContext,
        operation_id: OperationId,
        correlation_id: CorrelationId,
        vector_clock: VectorClock,
    ) -> Result<CommandResult, CreationError>;
}

/// A handler for a command that acts on an existing aggregate, wrapping
/// `api_server::handle_command` for a single concrete aggregate type. See
/// [`CreationHandler`]'s doc comment for the repository-scoping
/// correction this also reflects.
#[async_trait::async_trait]
pub trait DecisionHandler: Send + Sync {
    /// Handle a decision command against an existing aggregate.
    #[allow(clippy::too_many_arguments)]
    async fn handle_decision(
        &self,
        payload: serde_json::Value,
        target_id: ObjectId,
        operation_id: OperationId,
        actor: ActorContext,
        expected_version: ObjectVersion,
        expected_lifecycle_epoch: LifecycleEpoch,
        vector_clock: VectorClock,
        correlation_id: CorrelationId,
    ) -> Result<CommandResult, api_server::CommandError>;
}

enum RegisteredHandler {
    Creation(Box<dyn CreationHandler>),
    Decision(Box<dyn DecisionHandler>),
}

/// Maps `command_type` strings to their registered handler. Built once at
/// client startup (desktop-shell's `main.rs` / mobile-core's
/// `mobile_core_new`) and shared (`Arc<CommandRegistry>`) across the
/// client's lifetime. Each handler carries its own aggregate-type-scoped
/// `Repository`/`UnitOfWorkFactory` (see [`CreationHandler`]'s doc
/// comment) — the registry itself holds only the cross-cutting
/// `IdempotencyStore`, which per Team 2's design is not aggregate-scoped
/// (`IdempotencyStore::get`/`put` key by `OperationId` alone, valid across
/// every aggregate type).
pub struct CommandRegistry {
    handlers: HashMap<String, RegisteredHandler>,
}

impl CommandRegistry {
    /// Construct an empty registry.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a creation handler for `command_type` (e.g. `"CreateMission"`).
    pub fn register_creation(
        &mut self,
        command_type: impl Into<String>,
        handler: impl CreationHandler + 'static,
    ) -> &mut Self {
        self.handlers
            .insert(command_type.into(), RegisteredHandler::Creation(Box::new(handler)));
        self
    }

    /// Register a decision handler for `command_type` (e.g. `"PauseMission"`).
    pub fn register_decision(
        &mut self,
        command_type: impl Into<String>,
        handler: impl DecisionHandler + 'static,
    ) -> &mut Self {
        self.handlers
            .insert(command_type.into(), RegisteredHandler::Decision(Box::new(handler)));
        self
    }

    /// Dispatch a JSON-encoded command envelope to its registered handler.
    /// This is the function the Tauri `execute_command` handler and the
    /// FFI `mobile_core_execute_command` both call.
    pub async fn dispatch(
        &self,
        envelope: CommandEnvelope<serde_json::Value>,
    ) -> Result<CommandResult, CommandDispatchError> {
        let handler = self
            .handlers
            .get(&envelope.command_type)
            .ok_or_else(|| CommandDispatchError::UnknownCommandType(envelope.command_type.clone()))?;

        match handler {
            RegisteredHandler::Creation(h) => {
                let result = h
                    .handle_creation(
                        envelope.payload,
                        envelope.actor,
                        envelope.operation_id,
                        envelope.correlation_id,
                        envelope.vector_clock,
                    )
                    .await?;
                Ok(result)
            }
            RegisteredHandler::Decision(h) => {
                let result = h
                    .handle_decision(
                        envelope.payload,
                        envelope.target.id,
                        envelope.operation_id,
                        envelope.actor,
                        envelope.expected_version,
                        envelope.expected_lifecycle_epoch,
                        envelope.vector_clock,
                        envelope.correlation_id,
                    )
                    .await?;
                Ok(result)
            }
        }
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_kernel::{
        AuthorityEpoch, AuthorityProof, AuthorityScope, CommandId, CorrelationId,
        DomainObjectRef, ObjectId, ProofType, SchemaVersion, Timestamp,
    };

    fn sample_envelope(command_type: &str, payload: serde_json::Value) -> CommandEnvelope<serde_json::Value> {
        let org = ObjectId::new_random();
        CommandEnvelope {
            command_id: CommandId::new_random(),
            operation_id: OperationId::new_random(),
            command_type: command_type.to_string(),
            schema_version: SchemaVersion::new("1.0.0"),
            target: DomainObjectRef {
                id: ObjectId::new_random(),
                r#type: "test".to_string(),
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
                    object_type: "test".to_string(),
                    object_id: None,
                    command_types: vec![command_type.to_string()],
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
            payload,
        }
    }

    struct AlwaysSucceedsCreation;

    #[async_trait::async_trait]
    impl CreationHandler for AlwaysSucceedsCreation {
        async fn handle_creation(
            &self,
            payload: serde_json::Value,
            _actor: ActorContext,
            operation_id: OperationId,
            _correlation_id: CorrelationId,
            _vector_clock: VectorClock,
        ) -> Result<CommandResult, CreationError> {
            Ok(serde_json::json!({
                "success": true,
                "operation_id": operation_id,
                "echoed_payload": payload,
            }))
        }
    }

    struct AlwaysFailsDecision;

    #[async_trait::async_trait]
    impl DecisionHandler for AlwaysFailsDecision {
        async fn handle_decision(
            &self,
            _payload: serde_json::Value,
            target_id: ObjectId,
            _operation_id: OperationId,
            _actor: ActorContext,
            _expected_version: ObjectVersion,
            _expected_lifecycle_epoch: LifecycleEpoch,
            _vector_clock: VectorClock,
            _correlation_id: CorrelationId,
        ) -> Result<CommandResult, api_server::CommandError> {
            Err(api_server::CommandError::NotFound(target_id))
        }
    }

    #[tokio::test]
    async fn dispatch_routes_to_registered_creation_handler() {
        let mut registry = CommandRegistry::new();
        registry.register_creation("CreateThing", AlwaysSucceedsCreation);

        let envelope = sample_envelope("CreateThing", serde_json::json!({"name": "widget"}));
        let result = registry.dispatch(envelope).await.unwrap();

        assert_eq!(result["success"], serde_json::json!(true));
        assert_eq!(result["echoed_payload"]["name"], serde_json::json!("widget"));
    }

    #[tokio::test]
    async fn dispatch_routes_to_registered_decision_handler_and_propagates_its_error() {
        let mut registry = CommandRegistry::new();
        registry.register_decision("PauseThing", AlwaysFailsDecision);

        let envelope = sample_envelope("PauseThing", serde_json::json!({}));
        let err = registry.dispatch(envelope).await.unwrap_err();

        match err {
            CommandDispatchError::Decision(api_server::CommandError::NotFound(_)) => {}
            other => panic!("expected Decision(NotFound), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn dispatch_rejects_unknown_command_type() {
        let registry = CommandRegistry::new();
        let envelope = sample_envelope("NoSuchCommand", serde_json::json!({}));

        let err = registry.dispatch(envelope).await.unwrap_err();
        match err {
            CommandDispatchError::UnknownCommandType(t) => assert_eq!(t, "NoSuchCommand"),
            other => panic!("expected UnknownCommandType, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn creation_and_decision_handlers_coexist_under_different_command_types() {
        let mut registry = CommandRegistry::new();
        registry
            .register_creation("CreateThing", AlwaysSucceedsCreation)
            .register_decision("PauseThing", AlwaysFailsDecision);

        let create_result = registry
            .dispatch(sample_envelope("CreateThing", serde_json::json!({})))
            .await;
        assert!(create_result.is_ok());

        let decide_result = registry
            .dispatch(sample_envelope("PauseThing", serde_json::json!({})))
            .await;
        assert!(decide_result.is_err());
    }
}
