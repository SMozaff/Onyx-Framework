//! # kernel-contracts-stub
//!
//! **TEMPORARY STUB.** This crate is a stand-in for the real Increment 1
//! ("Team 1") kernel/contracts crates, which are being built in parallel and
//! were not available at the start of Increment 2.
//!
//! Per Architectural Ruling (Team 2 Kickoff §1): this stub matches the
//! Handover Document (`ONYX_Handover_Document__Machine-Readable_Contract_.md`)
//! exactly for every type it defines, contains no business logic, and exists
//! only so that Increment 2 (persistence, events, messaging) can compile and
//! be tested independently.
//!
//! **This crate MUST be replaced by the real Team 1 crates when delivered.**
//! In the final handover package it is retained only as a `dev-dependency`
//! for tests, per ruling. See `DECISIONS.md` at the workspace root for the
//! full list of architectural rulings that shaped this stub, including
//! several types (`CommandResult`, `OrganizationId`, `UserId`, `DeviceId`,
//! `ConflictId`, `VerifiedAuthority`, `PolicyDecisionSet`, `DataClassification`,
//! `IdGenerator`, `AuthError`, `SecretRef`) that are referenced throughout the
//! Handover Document but never defined in it, and were specified by explicit
//! ruling rather than invented silently.

pub mod aggregate_root;
pub mod authority;
pub mod causality;
pub mod domain_references;
pub mod envelopes;
pub mod identifiers;
pub mod time;
pub mod versioning;

// Flat re-export surface so downstream crates can `use kernel_contracts_stub::*;`
// matching how Team Prompt 2 refers to these types unqualified.
pub use aggregate_root::{AggregateRoot, DecisionContext, IdGenerator};
pub use authority::{
    ActorContext, AuthError, AuthorityProof, AuthorityScope, DataClassification, PolicyDecisionSet,
    ProofType, SecretRef, VerifiedAuthority,
};
pub use causality::{CausalRelation, VectorClock};
pub use domain_references::{DeviceRef, DomainObjectRef, OrganizationRef, UserRef};
pub use envelopes::{
    AuditMetadata, CommandEnvelope, CommandResult, DomainError, DomainEventEnvelope,
};
pub use identifiers::{
    CommandId, ConflictId, CorrelationId, DeviceId, EventId, ObjectId, OperationId, OrganizationId,
    UserId,
};
pub use time::{Duration, Timestamp};
pub use versioning::{AuthorityEpoch, LifecycleEpoch, ObjectVersion, ReplicaId, SchemaVersion};
