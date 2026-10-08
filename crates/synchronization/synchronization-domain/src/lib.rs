//! # synchronization-domain
//!
//! SynchronizationSession lifecycle, MergeStrategy dispatcher, ConflictRecord
//! aggregate, and event-sourced escalation (Team Prompt 3 §3.9, §3.10,
//! §3.11, §3.12, §3.13). Rebuilt against Team 1's real
//! `platform-kernel`/`platform-contracts` (Architectural Ruling Q1, Team 3
//! Initiation — Contract Reconciliation) plus the local
//! `platform-contracts-ext` extension (Ruling Q2). See `DECISIONS.md` at
//! the workspace root for every ruling and deviation.

#![deny(missing_docs)]

pub mod command_guard;
pub mod conflict;
pub mod escalation;
pub mod field_metadata;
pub mod merge_strategy;
pub mod session;

pub use command_guard::{check_conflict_pending, GuardError};
pub use conflict::{ConflictPendingMarker, ConflictRecord, ConflictResolution, ConflictStatus};
pub use escalation::{
    ConflictEscalationRequested, ConflictRepository, ConflictRepositoryError, EscalationError,
    EscalationService, PolicyConfig,
};
pub use field_metadata::{FieldMetadata, FieldShape};
pub use merge_strategy::{ConflictType, MergeOutcome, MergeStrategy, Value};
pub use session::{
    AggregateDelta, SessionPhase, SessionState, SyncCursor, SyncError, SyncResult,
    SynchronizationSession,
};
