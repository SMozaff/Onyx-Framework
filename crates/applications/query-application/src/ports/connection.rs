//! Connection port.
//!
//! `UnitOfWork::connection()` (Team Prompt 2 §3.2, restored per Clarification —
//! UnitOfWork Port Trait) returns `&dyn Connection`, but `Connection` is never
//! defined anywhere in the Handover Document or Team Prompt 2. Per Architectural
//! Ruling ("Connection Trait Shape"), this is a type-erased handle each adapter
//! downcasts to its own concrete transaction type via `as_any()`.
//! See DECISIONS.md item C1.

use std::any::Any;

/// A type-erased database connection handle. Each adapter (Postgres, SQLite)
/// provides its own concrete implementation. The repository uses this to
/// downcast and execute queries within the same transaction.
pub trait Connection: Send + Sync {
    /// Downcast to a concrete connection type. Adapters implement this by
    /// returning `self` as `&dyn Any`.
    fn as_any(&self) -> &dyn Any;
}
