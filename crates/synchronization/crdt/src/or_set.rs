//! OR-Set: Add-wins semantics.
//! Source: Team Prompt 3 §3.2. Concurrent add and remove of the same element
//! resolves as add-wins. Uses unique tags per add operation to distinguish
//! concurrent removes.
//!
//! DECISIONS.md B6: `T` bounds gain `Ord` (required for `BTreeMap<T, _>` keys,
//! which the frozen struct shape already requires but the frozen bound list
//! omitted).

use platform_kernel::{ReplicaId, VectorClock};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

use crate::Crdt;

/// An add-wins set: concurrent add and remove of the same element resolves
/// as add-wins. Uses unique tags per add operation to distinguish
/// concurrent removes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: DeserializeOwned"))]
pub struct OrSet<T: Ord + Hash + Eq + Clone + Debug + Serialize + DeserializeOwned> {
    /// Map from element to set of add tags.
    adds: BTreeMap<T, HashSet<Tag>>,
    /// Map from element to set of remove tags.
    removes: BTreeMap<T, HashSet<Tag>>,
    /// Causal context for GC.
    clock: VectorClock,
}

/// A unique, per-operation tag distinguishing one add (or remove) from
/// another, so concurrent adds/removes of the same element can be told
/// apart.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tag {
    /// The replica that performed this operation.
    pub replica_id: ReplicaId,
    /// A per-replica logical counter, for deterministic tie-breaking.
    pub lamport: u64,
}

impl<T> OrSet<T>
where
    T: Ord + Hash + Eq + Clone + Debug + Send + Sync + Serialize + DeserializeOwned,
{
    /// Create an empty OR-Set.
    pub fn new() -> Self {
        Self {
            adds: BTreeMap::new(),
            removes: BTreeMap::new(),
            clock: VectorClock::new(),
        }
    }

    /// Add an element with a unique tag and causal context.
    pub fn add(&mut self, element: T, tag: Tag, clock: VectorClock) {
        self.adds.entry(element).or_default().insert(tag);
        self.clock = self.clock.merge(&clock);
    }

    /// Remove an element with a tag and causal context.
    /// A remove tag must match an existing add tag to take effect.
    pub fn remove(&mut self, element: T, tag: Tag, clock: VectorClock) {
        self.removes.entry(element).or_default().insert(tag);
        self.clock = self.clock.merge(&clock);
    }

    /// Check if an element is present.
    /// An element is present if any add tag is NOT covered by a remove tag.
    pub fn contains(&self, element: &T) -> bool {
        let add_tags = self.adds.get(element).cloned().unwrap_or_default();
        let remove_tags = self.removes.get(element).cloned().unwrap_or_default();
        add_tags.difference(&remove_tags).count() > 0
    }

    /// Get all present elements.
    pub fn elements(&self) -> Vec<&T> {
        self.adds
            .iter()
            .filter(|(el, add_tags)| {
                let remove_tags = self.removes.get(*el).cloned().unwrap_or_default();
                add_tags.difference(&remove_tags).count() > 0
            })
            .map(|(el, _)| el)
            .collect()
    }

    /// Get the causal context for GC.
    pub fn causal_context(&self) -> &VectorClock {
        &self.clock
    }
}

impl<T> Default for OrSet<T>
where
    T: Ord + Hash + Eq + Clone + Debug + Send + Sync + Serialize + DeserializeOwned,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Crdt for OrSet<T>
where
    T: Ord + Hash + Eq + Clone + Debug + Send + Sync + Serialize + DeserializeOwned,
{
    fn merge(&mut self, other: &Self) -> bool {
        let mut changed = false;

        // Merge adds: union of add tags.
        for (el, tags) in &other.adds {
            let entry = self.adds.entry(el.clone()).or_default();
            let before_len = entry.len();
            entry.extend(tags.iter().cloned());
            if entry.len() != before_len {
                changed = true;
            }
        }

        // Merge removes: union of remove tags.
        for (el, tags) in &other.removes {
            let entry = self.removes.entry(el.clone()).or_default();
            let before_len = entry.len();
            entry.extend(tags.iter().cloned());
            if entry.len() != before_len {
                changed = true;
            }
        }

        // Merge clocks. (DECISIONS.md B1: VectorClock::merge is immutable.)
        let new_clock = self.clock.merge(&other.clock);
        if self.clock != new_clock {
            self.clock = new_clock;
            changed = true;
        }

        changed
    }

    fn causal_context(&self) -> &VectorClock {
        &self.clock
    }
}
