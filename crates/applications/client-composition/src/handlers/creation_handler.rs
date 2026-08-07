//! `CreationHandler` implementations. See `handlers/mod.rs` for shared
//! provenance notes.

use std::sync::Arc;

use platform_contracts::AggregateRoot;
use platform_kernel::{ActorContext, CorrelationId, OperationId, VectorClock};
use query_application::{Repository, UnitOfWorkFactory};

use crate::command_registry::{CommandResult, CreationError, CreationHandler};
use crate::handlers::creation_decision_context;

/// Creates a `Mission` from a `CreateMission` command via
/// `Mission::create()`, then persists it directly — no existing aggregate
/// to load, no version/epoch guard (see [`CreationHandler`]'s doc
/// comment on why this is a distinct path from `DecisionHandler`).
pub struct MissionCreationHandler {
    repo: Arc<dyn Repository>,
    unit_factory: Arc<dyn UnitOfWorkFactory>,
}

impl MissionCreationHandler {
    pub fn new(repo: Arc<dyn Repository>, unit_factory: Arc<dyn UnitOfWorkFactory>) -> Self {
        Self { repo, unit_factory }
    }
}

#[async_trait::async_trait]
impl CreationHandler for MissionCreationHandler {
    async fn handle_creation(
        &self,
        payload: serde_json::Value,
        actor: ActorContext,
        operation_id: OperationId,
        _correlation_id: CorrelationId,
        _vector_clock: VectorClock,
    ) -> Result<CommandResult, CreationError> {
        let command: mission_domain::MissionCommand = serde_json::from_value(payload)?;
        let organization_id = actor.organization_id;
        let ctx = creation_decision_context(actor);

        let events = mission_domain::Mission::create(command, &ctx)
            .map_err(|e| CreationError::Domain(e.to_string()))?;
        let mission = mission_domain::Mission::from_created_event(&events[0]);
        let aggregate_state = serde_json::to_value(&mission)?;

        let mut unit = self
            .unit_factory
            .create(organization_id)
            .await
            .map_err(|e| CreationError::Persistence(e.to_string()))?;

        // Creation produces no prior events to re-envelope here — the raw
        // domain event (`MissionCreated`) is not itself wrapped in a
        // `DomainEventEnvelope` by `Mission::create()`, matching how
        // `handle_command`'s Step 8 does that wrapping at the pipeline
        // level, not inside the aggregate. A full envelope requires
        // fields (aggregate_version, correlation_id's relationship to the
        // *committed* op, audit_metadata) this handler would otherwise
        // have to duplicate from `handle_command`'s Step 8 verbatim;
        // deferred as a follow-up rather than half-duplicating that logic
        // here. Flagged, not silently assumed complete: **no outbox
        // message is registered for a creation event yet**, so a
        // `MissionCreated` event will not currently reach the `EventBus`
        // via the outbox pump (`sync_agent.rs`) the way a decision
        // command's events do.
        self.repo
            .commit(aggregate_state, &[], &mut *unit)
            .await
            .map_err(|e| CreationError::Persistence(e.to_string()))?;

        let result = serde_json::json!({
            "success": true,
            "operation_id": operation_id,
            "mission_id": mission.id().0,
        });
        unit.register_idempotency_result(operation_id, result.clone());
        unit.commit()
            .await
            .map_err(|e| CreationError::Persistence(e.to_string()))?;

        Ok(result)
    }
}

/// Creates a `Task` from a `CreateTask` command via `Task::create()`. See
/// [`MissionCreationHandler`] for the shared implementation notes
/// (including the flagged gap: no outbox message registered for creation
/// events yet).
pub struct TaskCreationHandler {
    repo: Arc<dyn Repository>,
    unit_factory: Arc<dyn UnitOfWorkFactory>,
}

impl TaskCreationHandler {
    pub fn new(repo: Arc<dyn Repository>, unit_factory: Arc<dyn UnitOfWorkFactory>) -> Self {
        Self { repo, unit_factory }
    }
}

#[async_trait::async_trait]
impl CreationHandler for TaskCreationHandler {
    async fn handle_creation(
        &self,
        payload: serde_json::Value,
        actor: ActorContext,
        operation_id: OperationId,
        _correlation_id: CorrelationId,
        _vector_clock: VectorClock,
    ) -> Result<CommandResult, CreationError> {
        let command: work_domain::TaskCommand = serde_json::from_value(payload)?;
        let organization_id = actor.organization_id;
        let ctx = creation_decision_context(actor);

        let events = work_domain::Task::create(command, &ctx)
            .map_err(|e| CreationError::Domain(e.to_string()))?;
        let task = work_domain::Task::from_created_event(&events[0]);
        let aggregate_state = serde_json::to_value(&task)?;

        let mut unit = self
            .unit_factory
            .create(organization_id)
            .await
            .map_err(|e| CreationError::Persistence(e.to_string()))?;

        self.repo
            .commit(aggregate_state, &[], &mut *unit)
            .await
            .map_err(|e| CreationError::Persistence(e.to_string()))?;

        let result = serde_json::json!({
            "success": true,
            "operation_id": operation_id,
            "task_id": task.id().0,
        });
        unit.register_idempotency_result(operation_id, result.clone());
        unit.commit()
            .await
            .map_err(|e| CreationError::Persistence(e.to_string()))?;

        Ok(result)
    }
}
