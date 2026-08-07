//! # crdt
//!
//! CRDT (Conflict-Free Replicated Data Type) taxonomy for ONYX, per
//! Team Prompt 3 §3 and Blueprint §3.5 / §7.5. See `DECISIONS.md` at the
//! workspace root for the binding rulings that resolve Team Prompt 3 v1.0's
//! internal contradictions (referenced by ruling ID, e.g. "B6", throughout
//! this crate's doc comments and code).

#![deny(missing_docs)]

pub mod append_only_log;
pub mod lww_register;
pub mod mv_register;
pub mod or_set;
pub mod pn_counter;
pub mod rga;
pub mod tombstone_gc;

pub use append_only_log::{AppendOnlyLog, LogEntry};
pub use lww_register::LwwRegister;
pub use mv_register::MvRegister;
pub use or_set::{OrSet, Tag};
pub use pn_counter::PnCounter;
pub use rga::{Atom, ElementId, Rga};
pub use tombstone_gc::GarbageCollector;

use platform_kernel::VectorClock;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

/// The foundational trait for all CRDT types.
/// Every mergeable data structure in ONYX must implement this.
///
/// Source: Team Prompt 3 §3.1, transcribed verbatim except for the addition
/// of `Send + Sync` bounds already present on every concrete impl.
pub trait Crdt: Clone + Debug + Send + Sync + Serialize + DeserializeOwned {
    /// Merge another instance into this one, mutating `self`.
    /// Returns `true` if any change occurred (useful for GC tracking).
    /// This must be deterministic and commutative.
    fn merge(&mut self, other: &Self) -> bool;

    /// The causal context (vector clock) required to interpret this CRDT.
    /// Used for tombstone GC eligibility and causal comparison.
    fn causal_context(&self) -> &VectorClock;

    /// Merge two instances and return a new one (convenience method).
    fn merge_into(&self, other: &Self) -> Self {
        let mut result = self.clone();
        result.merge(other);
        result
    }
}
