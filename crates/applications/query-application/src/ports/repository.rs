//! Repository port.
//! Source: Team Prompt 2 §3.1, superseded by "Clarification — UnitOfWork
//! Port Trait" → "Updated Repository Trait (No Generic)", finalized by
//! "Ruling — Team 1/Team 2 Interface Reconciliation": the repository is
//! non-generic and works entirely in `serde_json::Value` for both
//! aggregate state and events. Domain decision logic
//! (`platform_contracts::AggregateRoot::decide`) runs upstream of
//! persistence, in the command handler pipeline (§6.1); the repository
//! never sees a typed aggregate or event.

use crate::ports::unit_of_work::UnitOfWork;
use async_trait::async_trait;
use platform_kernel::{AuthorityEpoch, LifecycleEpoch, ObjectId, ObjectVersion};

#[async_trait]
pub trait Repository: Send + Sync {
    /// Load an aggregate's serialized state by its ID. Returns None if not found.
    /// MUST NOT mutate state (Handover §8 Repository.load constraint).
    async fn load(&self, id: &ObjectId) -> Result<Option<Loaded>, RepositoryError>;

    /// Commit a set of changes atomically. The UnitOfWork is the transaction
    /// container; this call prepares/writes within `unit`'s transaction —
    /// the transaction is finalized by the caller invoking `unit.commit()`.
    ///
    /// `events` are serialized `platform_contracts::DomainEventEnvelope<E>`
    /// values (see `UnitOfWork::register_event` doc for rationale).
    async fn commit(
        &self,
        aggregate_state: serde_json::Value,
        events: &[serde_json::Value],
        unit: &mut dyn UnitOfWork,
    ) -> Result<CommitResult, RepositoryError>;
}

/// A loaded aggregate with its metadata.
#[derive(Debug, Clone)]
pub struct Loaded {
    /// Serialized aggregate state. Callers deserialize into the concrete
    /// aggregate type (e.g. `mission_domain::Mission`) using the domain
    /// crate's rehydration logic (`Aggregate::from_created_event` +
    /// replaying `apply()` — see Team 1 DECISIONS.md ruling C).
    pub aggregate: serde_json::Value,
    pub version: ObjectVersion,
    pub lifecycle_epoch: LifecycleEpoch,
    pub authority_epoch: AuthorityEpoch,
}

/// The result of a successful commit.
#[derive(Debug, Clone, Copy)]
pub struct CommitResult {
    pub new_version: ObjectVersion,
    pub new_lifecycle_epoch: Option<LifecycleEpoch>,
    pub new_authority_epoch: Option<AuthorityEpoch>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RepositoryError {
    #[error("database connection failed")]
    ConnectionFailure,
    #[error("version conflict")]
    VersionConflict,
    #[error("epoch conflict")]
    EpochConflict,
    #[error("constraint violation: {0}")]
    ConstraintViolation(String),
    #[error("serialization error: {0}")]
    SerializationError(String),
    #[error("object not found")]
    NotFound,
    #[error("unknown error: {0}")]
    Unknown(String),
}
