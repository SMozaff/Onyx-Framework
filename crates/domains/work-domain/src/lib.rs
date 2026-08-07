//! # work-domain
//!
//! ONYX Task aggregate: pure domain logic for task lifecycle management.
//! Depends on `platform-kernel`, `platform-contracts`, and `mission-domain`
//! (for `MissionId`, since every task belongs to exactly one mission — a
//! Handover Document invariant). No I/O, no persistence, no async.

#![deny(warnings)]
#![deny(missing_docs)]

pub mod aggregate;
pub mod command;
pub mod error;
pub mod event;
pub mod state_machine;
pub mod value;

/// Deterministic test fixtures, exposed unconditionally for the same
/// reason as `mission_domain::test_support` (see its docs).
pub mod test_support;

pub use aggregate::Task;
pub use command::TaskCommand;
pub use error::TaskError;
pub use event::TaskEvent;
pub use state_machine::TaskStatus;
pub use value::{Dependency, DependencyType, TaskId, TaskMissionRef, TaskPriority};
