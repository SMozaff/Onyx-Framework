//! `DecisionHandler` implementations. See `handlers/mod.rs` for shared
//! provenance notes.

use std::sync::Arc;

use platform_kernel::{
    ActorContext, CorrelationId, LifecycleEpoch, ObjectId, ObjectVersion, OperationId, VectorClock,
};
use query_application::{IdempotencyStore, Repository, UnitOfWorkFactory};

use crate::command_registry::{CommandResult, DecisionHandler};

/// Wraps `api_server::handle_command` for `Mission`, for every
/// `MissionCommand` variant other than `CreateMission`.
pub struct MissionDecisionHandler {
    repo: Arc<dyn Repository>,
    unit_factory: Arc<dyn UnitOfWorkFactory>,
    idempotency_store: Arc<dyn IdempotencyStore>,
}

impl MissionDecisionHandler {
    pub fn new(
        repo: Arc<dyn Repository>,
        unit_factory: Arc<dyn UnitOfWorkFactory>,
        idempotency_store: Arc<dyn IdempotencyStore>,
    ) -> Self {
        Self {
            repo,
            unit_factory,
            idempotency_store,
        }
    }
}

#[async_trait::async_trait]
impl DecisionHandler for MissionDecisionHandler {
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
    ) -> Result<CommandResult, api_server::CommandError> {
        let command: mission_domain::MissionCommand = serde_json::from_value(payload)
            .map_err(api_server::CommandError::Serialization)?;

        api_server::handle_command::<mission_domain::Mission, _, _, _>(
            command,
            target_id,
            operation_id,
            actor,
            expected_version,
            expected_lifecycle_epoch,
            platform_kernel::AuthorityEpoch(0), // see R2: handle_command's own `_expected_authority_epoch` param is unused (`_`-prefixed) in its real signature.
            vector_clock,
            correlation_id,
            "mission",
            Arc::clone(&self.repo),
            Arc::clone(&self.unit_factory),
            Arc::clone(&self.idempotency_store),
        )
        .await
    }
}

/// Wraps `api_server::handle_command` for `Task`, for every `TaskCommand`
/// variant other than `CreateTask`.
pub struct TaskDecisionHandler {
    repo: Arc<dyn Repository>,
    unit_factory: Arc<dyn UnitOfWorkFactory>,
    idempotency_store: Arc<dyn IdempotencyStore>,
}

impl TaskDecisionHandler {
    pub fn new(
        repo: Arc<dyn Repository>,
        unit_factory: Arc<dyn UnitOfWorkFactory>,
        idempotency_store: Arc<dyn IdempotencyStore>,
    ) -> Self {
        Self {
            repo,
            unit_factory,
            idempotency_store,
        }
    }
}

#[async_trait::async_trait]
impl DecisionHandler for TaskDecisionHandler {
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
    ) -> Result<CommandResult, api_server::CommandError> {
        let command: work_domain::TaskCommand = serde_json::from_value(payload)
            .map_err(api_server::CommandError::Serialization)?;

        api_server::handle_command::<work_domain::Task, _, _, _>(
            command,
            target_id,
            operation_id,
            actor,
            expected_version,
            expected_lifecycle_epoch,
            platform_kernel::AuthorityEpoch(0),
            vector_clock,
            correlation_id,
            "task",
            Arc::clone(&self.repo),
            Arc::clone(&self.unit_factory),
            Arc::clone(&self.idempotency_store),
        )
        .await
    }
}
