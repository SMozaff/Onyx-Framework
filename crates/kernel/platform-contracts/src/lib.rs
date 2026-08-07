//! # platform-contracts
//!
//! ONYX envelope and trait contracts: `CommandEnvelope`, `DomainEventEnvelope`,
//! the `AggregateRoot` trait, `DecisionContext`, and the `DomainError`
//! protocol. Depends only on `platform-kernel`.

#![deny(warnings)]
#![deny(missing_docs)]

pub mod command;
pub mod error;
pub mod event;
pub mod traits;

pub use command::CommandEnvelope;
pub use error::{DomainError, DomainErrorResponse, RepositoryError};
pub use event::{AuditMetadata, DataClassification, DomainEventEnvelope};
pub use traits::{AggregateRoot, DecisionContext, IdGenerator};
