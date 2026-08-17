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
        expected_authority_epoch: platform_kernel::AuthorityEpoch,
        vector_clock: VectorClock,
        correlation_id: CorrelationId,
    ) -> Result<CommandResult, api_server::CommandError> {
        let command: mission_domain::MissionCommand =
            serde_json::from_value(payload).map_err(api_server::CommandError::Serialization)?;

        api_server::handle_command::<mission_domain::Mission, _, _, _>(
            command,
            target_id,
            operation_id,
            actor,
            expected_version,
            expected_lifecycle_epoch,
            expected_authority_epoch,
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
        expected_authority_epoch: platform_kernel::AuthorityEpoch,
        vector_clock: VectorClock,
        correlation_id: CorrelationId,
    ) -> Result<CommandResult, api_server::CommandError> {
        let command: work_domain::TaskCommand =
            serde_json::from_value(payload).map_err(api_server::CommandError::Serialization)?;

        api_server::handle_command::<work_domain::Task, _, _, _>(
            command,
            target_id,
            operation_id,
            actor,
            expected_version,
            expected_lifecycle_epoch,
            expected_authority_epoch,
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

/// Wraps `api_server::handle_command` for `Conversation`, for every
/// `ConversationCommand` variant other than `CreateConversation`.
pub struct ConversationDecisionHandler {
    repo: Arc<dyn Repository>,
    unit_factory: Arc<dyn UnitOfWorkFactory>,
    idempotency_store: Arc<dyn IdempotencyStore>,
}

impl ConversationDecisionHandler {
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
impl DecisionHandler for ConversationDecisionHandler {
    async fn handle_decision(
        &self,
        payload: serde_json::Value,
        target_id: ObjectId,
        operation_id: OperationId,
        actor: ActorContext,
        expected_version: ObjectVersion,
        expected_lifecycle_epoch: LifecycleEpoch,
        expected_authority_epoch: platform_kernel::AuthorityEpoch,
        vector_clock: VectorClock,
        correlation_id: CorrelationId,
    ) -> Result<CommandResult, api_server::CommandError> {
        let command: communication_domain::ConversationCommand =
            serde_json::from_value(payload).map_err(api_server::CommandError::Serialization)?;

        api_server::handle_command::<communication_domain::Conversation, _, _, _>(
            command,
            target_id,
            operation_id,
            actor,
            expected_version,
            expected_lifecycle_epoch,
            expected_authority_epoch,
            vector_clock,
            correlation_id,
            "conversation",
            Arc::clone(&self.repo),
            Arc::clone(&self.unit_factory),
            Arc::clone(&self.idempotency_store),
        )
        .await
    }
}

/// Wraps `api_server::handle_command` for `FileAsset`, for every
/// `FileAssetCommand` variant other than `CreateFileAsset`.
pub struct FileAssetDecisionHandler {
    repo: Arc<dyn Repository>,
    unit_factory: Arc<dyn UnitOfWorkFactory>,
    idempotency_store: Arc<dyn IdempotencyStore>,
}

impl FileAssetDecisionHandler {
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
impl DecisionHandler for FileAssetDecisionHandler {
    async fn handle_decision(
        &self,
        payload: serde_json::Value,
        target_id: ObjectId,
        operation_id: OperationId,
        actor: ActorContext,
        expected_version: ObjectVersion,
        expected_lifecycle_epoch: LifecycleEpoch,
        expected_authority_epoch: platform_kernel::AuthorityEpoch,
        vector_clock: VectorClock,
        correlation_id: CorrelationId,
    ) -> Result<CommandResult, api_server::CommandError> {
        let command: file_domain::FileAssetCommand =
            serde_json::from_value(payload).map_err(api_server::CommandError::Serialization)?;

        api_server::handle_command::<file_domain::FileAsset, _, _, _>(
            command,
            target_id,
            operation_id,
            actor,
            expected_version,
            expected_lifecycle_epoch,
            expected_authority_epoch,
            vector_clock,
            correlation_id,
            "file_asset",
            Arc::clone(&self.repo),
            Arc::clone(&self.unit_factory),
            Arc::clone(&self.idempotency_store),
        )
        .await
    }
}

/// Wraps `api_server::handle_command` for `UploadSession`, for every
/// `UploadSessionCommand` variant other than `StartUpload`.
pub struct UploadSessionDecisionHandler {
    repo: Arc<dyn Repository>,
    unit_factory: Arc<dyn UnitOfWorkFactory>,
    idempotency_store: Arc<dyn IdempotencyStore>,
}

impl UploadSessionDecisionHandler {
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
impl DecisionHandler for UploadSessionDecisionHandler {
    async fn handle_decision(
        &self,
        payload: serde_json::Value,
        target_id: ObjectId,
        operation_id: OperationId,
        actor: ActorContext,
        expected_version: ObjectVersion,
        expected_lifecycle_epoch: LifecycleEpoch,
        expected_authority_epoch: platform_kernel::AuthorityEpoch,
        vector_clock: VectorClock,
        correlation_id: CorrelationId,
    ) -> Result<CommandResult, api_server::CommandError> {
        let command: file_domain::UploadSessionCommand =
            serde_json::from_value(payload).map_err(api_server::CommandError::Serialization)?;

        api_server::handle_command::<file_domain::UploadSession, _, _, _>(
            command,
            target_id,
            operation_id,
            actor,
            expected_version,
            expected_lifecycle_epoch,
            expected_authority_epoch,
            vector_clock,
            correlation_id,
            "upload_session",
            Arc::clone(&self.repo),
            Arc::clone(&self.unit_factory),
            Arc::clone(&self.idempotency_store),
        )
        .await
    }
}

/// Wraps `api_server::handle_command` for `Message`, for every
/// `MessageCommand` variant other than `PostMessage`.
pub struct MessageDecisionHandler {
    repo: Arc<dyn Repository>,
    unit_factory: Arc<dyn UnitOfWorkFactory>,
    idempotency_store: Arc<dyn IdempotencyStore>,
}

impl MessageDecisionHandler {
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
impl DecisionHandler for MessageDecisionHandler {
    async fn handle_decision(
        &self,
        payload: serde_json::Value,
        target_id: ObjectId,
        operation_id: OperationId,
        actor: ActorContext,
        expected_version: ObjectVersion,
        expected_lifecycle_epoch: LifecycleEpoch,
        expected_authority_epoch: platform_kernel::AuthorityEpoch,
        vector_clock: VectorClock,
        correlation_id: CorrelationId,
    ) -> Result<CommandResult, api_server::CommandError> {
        let command: communication_domain::MessageCommand =
            serde_json::from_value(payload).map_err(api_server::CommandError::Serialization)?;

        api_server::handle_command::<communication_domain::Message, _, _, _>(
            command,
            target_id,
            operation_id,
            actor,
            expected_version,
            expected_lifecycle_epoch,
            expected_authority_epoch,
            vector_clock,
            correlation_id,
            "message",
            Arc::clone(&self.repo),
            Arc::clone(&self.unit_factory),
            Arc::clone(&self.idempotency_store),
        )
        .await
    }
}

/// Wraps `api_server::handle_command` for `Policy`, for every
/// `PolicyCommand` variant other than `CreatePolicy`. Phase 1 addition.
pub struct PolicyDecisionHandler {
    repo: Arc<dyn Repository>,
    unit_factory: Arc<dyn UnitOfWorkFactory>,
    idempotency_store: Arc<dyn IdempotencyStore>,
}

impl PolicyDecisionHandler {
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
impl DecisionHandler for PolicyDecisionHandler {
    async fn handle_decision(
        &self,
        payload: serde_json::Value,
        target_id: ObjectId,
        operation_id: OperationId,
        actor: ActorContext,
        expected_version: ObjectVersion,
        expected_lifecycle_epoch: LifecycleEpoch,
        expected_authority_epoch: platform_kernel::AuthorityEpoch,
        vector_clock: VectorClock,
        correlation_id: CorrelationId,
    ) -> Result<CommandResult, api_server::CommandError> {
        let command: policy_domain::PolicyCommand =
            serde_json::from_value(payload).map_err(api_server::CommandError::Serialization)?;

        api_server::handle_command::<policy_domain::Policy, _, _, _>(
            command,
            target_id,
            operation_id,
            actor,
            expected_version,
            expected_lifecycle_epoch,
            expected_authority_epoch,
            vector_clock,
            correlation_id,
            "policy",
            Arc::clone(&self.repo),
            Arc::clone(&self.unit_factory),
            Arc::clone(&self.idempotency_store),
        )
        .await
    }
}

/// Wraps `api_server::handle_command` for `LegalHold`, for every
/// `LegalHoldCommand` variant other than `ApplyLegalHold`. Phase 1
/// addition.
pub struct LegalHoldDecisionHandler {
    repo: Arc<dyn Repository>,
    unit_factory: Arc<dyn UnitOfWorkFactory>,
    idempotency_store: Arc<dyn IdempotencyStore>,
}

impl LegalHoldDecisionHandler {
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
impl DecisionHandler for LegalHoldDecisionHandler {
    async fn handle_decision(
        &self,
        payload: serde_json::Value,
        target_id: ObjectId,
        operation_id: OperationId,
        actor: ActorContext,
        expected_version: ObjectVersion,
        expected_lifecycle_epoch: LifecycleEpoch,
        expected_authority_epoch: platform_kernel::AuthorityEpoch,
        vector_clock: VectorClock,
        correlation_id: CorrelationId,
    ) -> Result<CommandResult, api_server::CommandError> {
        let command: policy_domain::LegalHoldCommand =
            serde_json::from_value(payload).map_err(api_server::CommandError::Serialization)?;

        api_server::handle_command::<policy_domain::LegalHold, _, _, _>(
            command,
            target_id,
            operation_id,
            actor,
            expected_version,
            expected_lifecycle_epoch,
            expected_authority_epoch,
            vector_clock,
            correlation_id,
            "legal_hold",
            Arc::clone(&self.repo),
            Arc::clone(&self.unit_factory),
            Arc::clone(&self.idempotency_store),
        )
        .await
    }
}

/// Wraps `api_server::handle_command` for `ConnectionRequest`, for every
/// `ConnectionRequestCommand` variant other than `SendConnectionRequest`.
/// Phase 1 addition.
pub struct ConnectionRequestDecisionHandler {
    repo: Arc<dyn Repository>,
    unit_factory: Arc<dyn UnitOfWorkFactory>,
    idempotency_store: Arc<dyn IdempotencyStore>,
}

impl ConnectionRequestDecisionHandler {
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
impl DecisionHandler for ConnectionRequestDecisionHandler {
    async fn handle_decision(
        &self,
        payload: serde_json::Value,
        target_id: ObjectId,
        operation_id: OperationId,
        actor: ActorContext,
        expected_version: ObjectVersion,
        expected_lifecycle_epoch: LifecycleEpoch,
        expected_authority_epoch: platform_kernel::AuthorityEpoch,
        vector_clock: VectorClock,
        correlation_id: CorrelationId,
    ) -> Result<CommandResult, api_server::CommandError> {
        let command: communication_domain::ConnectionRequestCommand =
            serde_json::from_value(payload).map_err(api_server::CommandError::Serialization)?;

        api_server::handle_command::<communication_domain::ConnectionRequest, _, _, _>(
            command,
            target_id,
            operation_id,
            actor,
            expected_version,
            expected_lifecycle_epoch,
            expected_authority_epoch,
            vector_clock,
            correlation_id,
            "connection_request",
            Arc::clone(&self.repo),
            Arc::clone(&self.unit_factory),
            Arc::clone(&self.idempotency_store),
        )
        .await
    }
}

/// Wraps `api_server::handle_command` for `NotificationAggregate`.
/// Notifications are created by upstream workflow producers or synchronized
/// into a local replica; the desktop client only dispatches acknowledgement
/// decisions against the addressed notification.
pub struct NotificationDecisionHandler {
    repo: Arc<dyn Repository>,
    unit_factory: Arc<dyn UnitOfWorkFactory>,
    idempotency_store: Arc<dyn IdempotencyStore>,
}

impl NotificationDecisionHandler {
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
impl DecisionHandler for NotificationDecisionHandler {
    async fn handle_decision(
        &self,
        payload: serde_json::Value,
        target_id: ObjectId,
        operation_id: OperationId,
        actor: ActorContext,
        expected_version: ObjectVersion,
        expected_lifecycle_epoch: LifecycleEpoch,
        expected_authority_epoch: platform_kernel::AuthorityEpoch,
        vector_clock: VectorClock,
        correlation_id: CorrelationId,
    ) -> Result<CommandResult, api_server::CommandError> {
        let command: notification_domain::NotificationCommand =
            serde_json::from_value(payload).map_err(api_server::CommandError::Serialization)?;

        api_server::handle_command::<notification_domain::NotificationAggregate, _, _, _>(
            command,
            target_id,
            operation_id,
            actor,
            expected_version,
            expected_lifecycle_epoch,
            expected_authority_epoch,
            vector_clock,
            correlation_id,
            "notification",
            Arc::clone(&self.repo),
            Arc::clone(&self.unit_factory),
            Arc::clone(&self.idempotency_store),
        )
        .await
    }
}
