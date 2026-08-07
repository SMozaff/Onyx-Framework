//! # sync-test-utils
//!
//! Shared test utilities for `crdt` and `synchronization-domain`: mock
//! adapters (against the real `query-application` ports) and proptest
//! generators.

#![deny(missing_docs)]

pub mod generators;
pub mod mocks;

pub use generators::{arbitrary_or_set_operations, OrSetOperation};
pub use mocks::{MockConflictRepository, MockConnection, MockRepository, MockUnitOfWork};
