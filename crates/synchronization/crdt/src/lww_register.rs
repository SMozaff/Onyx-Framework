//! LWW-Register: Last-Write-Wins with deterministic tie-break.
//! Source: Team Prompt 3 §3.3. Appropriate only where silently discarding a
//! concurrent write is acceptable (e.g. non-authoritative annotations,
//! confidence scores).
//!
//! DECISIONS.md B6: `V` bounds gain `Eq` (required for the `self.value !=
//! other.value` comparison in `merge`, which the frozen bound list omitted).

use platform_kernel::{ReplicaId, Timestamp, VectorClock};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::Crdt;

/// A last-write-wins register: on a concurrent write, the value with the
/// later timestamp wins, with replica-id as a deterministic tie-break.
/// Appropriate only where silently discarding a concurrent write is
/// acceptable (e.g. non-authoritative annotations, confidence scores).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(serialize = "V: Serialize", deserialize = "V: DeserializeOwned"))]
pub struct LwwRegister<V: Clone + Debug + Eq + Serialize + DeserializeOwned> {
    value: V,
    timestamp: Timestamp,
    replica_id: ReplicaId,
    clock: VectorClock,
}

impl<V: Clone + Debug + Eq + Serialize + DeserializeOwned> LwwRegister<V> {
    /// Create a register with an initial value, timestamped now.
    pub fn new(value: V, replica_id: ReplicaId) -> Self {
        Self {
            value,
            timestamp: Timestamp::now(),
            replica_id,
            clock: VectorClock::new(),
        }
    }

    /// Set the value with a timestamp and replica ID.
    /// The clock is used for causal context.
    pub fn set(&mut self, value: V, ts: Timestamp, replica: ReplicaId, clock: VectorClock) {
        if ts > self.timestamp || (ts == self.timestamp && replica > self.replica_id) {
            self.value = value;
            self.timestamp = ts;
            self.replica_id = replica;
        }
        self.clock = self.clock.merge(&clock);
    }

    /// The current (winning) value.
    pub fn get(&self) -> &V {
        &self.value
    }

    /// The causal context for GC.
    pub fn causal_context(&self) -> &VectorClock {
        &self.clock
    }
}

impl<V: Clone + Debug + Eq + Send + Sync + Serialize + DeserializeOwned> Crdt for LwwRegister<V> {
    fn merge(&mut self, other: &Self) -> bool {
        let mut changed = false;

        if other.timestamp > self.timestamp
            || (other.timestamp == self.timestamp && other.replica_id > self.replica_id)
        {
            if self.value != other.value {
                self.value = other.value.clone();
                changed = true;
            }
            self.timestamp = other.timestamp;
            self.replica_id = other.replica_id;
        }

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
