//! # platform-kernel
//!
//! ONYX Shared Kernel. Pure, infrastructure-free types shared by every
//! bounded context: identifiers, versioning, causality, authority, time, and
//! domain references.
//!
//! This crate has zero external dependencies beyond `serde` (serialization)
//! and `uuid` (offline ID generation). No databases, no networking, no async
//! runtimes, no logging.

#![deny(warnings)]
#![deny(missing_docs)]
#![allow(clippy::module_inception)]

pub mod authority;
pub mod causality;
pub mod identifiers;
pub mod refs;
pub mod time;
pub mod versioning;

pub use authority::{
    ActorContext, AuthorityProof, AuthorityScope, DeviceId, OrganizationId, PolicyDecisionSet,
    ProofType, UserId, VerifiedAuthority,
};
pub use causality::{CausalRelation, VectorClock};
pub use identifiers::{CommandId, ConflictId, CorrelationId, EventId, ObjectId, OperationId};
pub use refs::{DeviceRef, DomainObjectRef, OrganizationRef, UserRef};
pub use time::{Duration, Timestamp};
pub use versioning::{AuthorityEpoch, LifecycleEpoch, ObjectVersion, ReplicaId, SchemaVersion};
