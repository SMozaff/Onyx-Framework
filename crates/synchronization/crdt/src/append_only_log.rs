//! Append-only causal log.
//! Source: Team Prompt 3 §3.7. NOT a CRDT — entries are ordered by vector
//! clock, concurrent entries retained. Used for Message revisions, audit
//! entries, and domain events (Blueprint §7.5.5). Intentionally does not
//! implement the `Crdt` trait: it has no merge conflict to resolve, only
//! deduplication by clock, matching the frozen contract's own note that
//! this type is deliberately excluded from the CRDT taxonomy.

use platform_kernel::VectorClock;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// An append-only, causally-ordered log. NOT a CRDT: entries are ordered
/// by vector clock, with concurrent entries retained rather than merged —
/// there is no conflict to resolve, only deduplication by clock.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "E: Serialize", deserialize = "E: DeserializeOwned"))]
pub struct AppendOnlyLog<E: Clone + Debug + Serialize + DeserializeOwned> {
    entries: Vec<LogEntry<E>>,
    clock: VectorClock,
}

/// A single entry in an `AppendOnlyLog`, tagged with the causal clock it
/// was appended under.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(
    serialize = "E: Serialize",
    deserialize = "E: serde::de::DeserializeOwned"
))]
pub struct LogEntry<E> {
    /// The logged value.
    pub entry: E,
    /// The causal clock at the time this entry was appended.
    pub clock: VectorClock,
}

impl<E: Clone + Debug + Send + Sync + Serialize + DeserializeOwned> AppendOnlyLog<E> {
    /// Create an empty log.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            clock: VectorClock::new(),
        }
    }

    /// Append an entry with its causal clock.
    pub fn append(&mut self, entry: E, clock: VectorClock) {
        self.entries.push(LogEntry {
            entry,
            clock: clock.clone(),
        });
        self.clock = self.clock.merge(&clock);
    }

    /// Get all entries.
    pub fn entries(&self) -> &[LogEntry<E>] {
        &self.entries
    }

    /// Merge with another log.
    /// Adds entries not already present (by clock comparison).
    pub fn merge_entries(&mut self, other: &Self) -> bool {
        let mut changed = false;

        for entry in &other.entries {
            if !self.entries.iter().any(|e| e.clock == entry.clock) {
                self.entries.push(entry.clone());
                changed = true;
            }
        }

        let new_clock = self.clock.merge(&other.clock);
        if self.clock != new_clock {
            self.clock = new_clock;
            changed = true;
        }

        changed
    }

    /// The causal context for GC.
    pub fn causal_context(&self) -> &VectorClock {
        &self.clock
    }
}

impl<E: Clone + Debug + Send + Sync + Serialize + DeserializeOwned> Default for AppendOnlyLog<E> {
    fn default() -> Self {
        Self::new()
    }
}
