//! # persistence-postgres
//!
//! PostgreSQL adapter implementing `query-application`'s `Repository` and
//! `UnitOfWork` ports, and `worker-application`'s `OutboxStore`,
//! `InboxStore`, and `DeadLetterStore` ports.
//!
//! Per Team Prompt 2 §2 File Manifest. Schema per §4.1 (verbatim frozen
//! DDL). Trait implementations reflect the Architectural Rulings recorded
//! in `DECISIONS.md` at the workspace root.

pub mod dead_letter_store;
pub mod inbox_store;
pub mod outbox_store;
pub mod repository;
pub mod unit_of_work;

pub use dead_letter_store::PostgresDeadLetterStore;
pub use inbox_store::PostgresInboxStore;
pub use outbox_store::PostgresOutboxStore;
pub use repository::PostgresRepository;
pub use unit_of_work::{PostgresUnitOfWork, PostgresUnitOfWorkFactory};
