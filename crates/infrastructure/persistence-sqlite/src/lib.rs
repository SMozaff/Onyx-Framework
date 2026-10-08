//! # persistence-sqlite
//!
//! SQLite adapter implementing `query-application`'s `Repository` and
//! `UnitOfWork` ports, and `worker-application`'s `OutboxStore`,
//! `InboxStore`, and `DeadLetterStore` ports.
//!
//! Schema per Team Prompt 2 §4.2 and Architectural Ruling "SQLite Schema
//! Mapping" (DECISIONS.md S1). Column conventions: UUID->BLOB, JSONB->TEXT,
//! TIMESTAMPTZ->INTEGER (Unix milliseconds, per T1).

pub mod dead_letter_store;
pub mod inbox_store;
pub mod outbox_store;
pub mod repository;
pub mod unit_of_work;

pub use dead_letter_store::SqliteDeadLetterStore;
pub use inbox_store::SqliteInboxStore;
pub use outbox_store::SqliteOutboxStore;
pub use repository::SqliteRepository;
pub use unit_of_work::{SqliteUnitOfWork, SqliteUnitOfWorkFactory};
