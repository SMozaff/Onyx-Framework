//! DeadLetterStore port.
//! Per Architectural Ruling (Team 2 Kickoff, item 4): referenced in §5.1
//! relay algorithm but never defined. This is our own port, defined here
//! in `worker-application` per the ruling's placement table.
//!
//! Distinct from `OutboxStore::dead_letter()` (§3.3): that method moves an
//! outbox row's *status* to dead-letter within the same outbox table.
//! `DeadLetterStore` is the separate durable sink the relay loop (§5.1,
//! per Architectural Ruling 3b) writes to when a claimed message is
//! permanently failed — giving operators a dedicated place to inspect/
//! replay dead-lettered messages without scanning the primary outbox table.

use crate::ports::outbox_store::ClaimedMessage;
use async_trait::async_trait;

#[async_trait]
pub trait DeadLetterStore: Send + Sync {
    /// Persist a permanently-failed claimed message to the dead-letter sink.
    ///
    /// Per Architectural Ruling ("Dead Letter Table Single Writer" —
    /// DECISIONS.md D1): `DeadLetterStore` is the SOLE writer of the
    /// `dead_letter` table. `OutboxStore::dead_letter()` delegates to this
    /// method rather than writing to the table itself, so a caller that
    /// invokes both (as a relay loop naturally would — see §5.1) does not
    /// produce a duplicate row.
    async fn send(&self, claimed: &ClaimedMessage, error: &str) -> Result<(), DeadLetterError>;
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DeadLetterError {
    #[error("database error: {0}")]
    Database(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}
