//! Shared test fixtures for persistence adapter integration tests.
//!
//! This module intentionally does NOT reimplement deterministic ID
//! generation or `DecisionContext` construction — `mission-domain::
//! test_support` (Team 1's real deliverable) already provides
//! `DeterministicIdGenerator` and `test_context()`. Persistence adapter
//! tests need lower-level fixtures instead: raw `ObjectId`/`OrganizationId`
//! generation and a couple of small helpers for constructing rows to
//! insert directly (bypassing the full command-handler pipeline, since
//! Increment 2's job is to test the persistence layer in isolation).
//!
//! Gated behind `#[cfg(feature = "test-helpers")]` rather than
//! `#[cfg(test)]`, since these fixtures are consumed by *other* crates'
//! integration tests (persistence-postgres, persistence-sqlite), not only
//! this crate's own unit tests.

use platform_kernel::{ObjectId, OrganizationId};
use uuid::Uuid;

/// A fresh, random `ObjectId` for use as an aggregate/entity ID in tests.
pub fn random_object_id() -> ObjectId {
    ObjectId(*Uuid::new_v4().as_bytes())
}

/// A fresh, random `OrganizationId` for use as a tenant boundary in tests.
/// Note: `OrganizationId` is a type alias for `ObjectId` in Team 1's real
/// `platform-kernel` (not a distinct newtype, unlike the pre-Team-1 stub),
/// so this constructs an `ObjectId` directly.
pub fn random_organization_id() -> OrganizationId {
    ObjectId(*Uuid::new_v4().as_bytes())
}
