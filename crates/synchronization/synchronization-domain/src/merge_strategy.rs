//! MergeStrategy Dispatcher.
//! Source: Team Prompt 3 §3.9. The single dispatcher over CRDT taxonomy
//! shapes. MUST dispatch by FieldShape, NOT by aggregate type.
//!
//! `MergeOutcome::Conflict` is a struct variant (`Conflict { conflict_type
//! }`), matching how it's constructed in the `AuthorityControlled` arm, not
//! a tuple variant (a real defect in Team Prompt 3 v1.0's frozen contract,
//! which declared the enum one way and constructed it another).
//!
//! `Value` (referenced throughout §3.9 but never defined anywhere in Team
//! Prompt 3, and not present in the real Increment 1/2 kernel either) is an
//! opaque JSON encoding of the shape-specific CRDT state
//! (`serde_json::Value`) — the minimal thing that lets `merge_field` be
//! generic over field shape without knowing concrete Rust element types at
//! the dispatch site.

use platform_kernel::{CausalRelation, VectorClock};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::field_metadata::{FieldMetadata, FieldShape};
use crdt::{Crdt, LwwRegister, MvRegister, OrSet, PnCounter, Rga};

/// See module doc comment: opaque JSON encoding of shape-specific CRDT state.
pub type Value = JsonValue;

/// Pure dispatcher over the CRDT taxonomy: given a field's declared shape
/// and two concurrent values, decides how (or whether) to merge them.
/// Holds no state — dispatch is a pure function of its inputs.
#[derive(Clone, Default)]
pub struct MergeStrategy {
    // No state — pure dispatch.
}

impl MergeStrategy {
    /// Construct a new dispatcher.
    pub fn new() -> Self {
        Self {}
    }

    /// Dispatch by data shape (Blueprint §3.5 / §7.4).
    /// Returns a MergeOutcome.
    pub fn merge_field(
        &self,
        field_metadata: &FieldMetadata,
        local_value: Value,
        remote_value: Value,
        local_clock: &VectorClock,
        remote_clock: &VectorClock,
    ) -> MergeOutcome {
        use FieldShape::*;

        // Determine causal relation.
        let relation = local_clock.compare(remote_clock);

        // If operations are ordered (BEFORE/AFTER), no merge needed — apply the later one.
        match relation {
            CausalRelation::Before => {
                return MergeOutcome::Applied {
                    value: remote_value,
                }
            }
            CausalRelation::After => return MergeOutcome::Applied { value: local_value },
            CausalRelation::Equal => return MergeOutcome::Applied { value: local_value },
            CausalRelation::Concurrent => {}
        }

        // CONCURRENT — need to merge.
        match field_metadata.shape {
            ToggleSet => merge_shape::<OrSet<String>>(local_value, remote_value),
            LwwSingle => merge_shape::<LwwRegister<JsonValue>>(local_value, remote_value),
            MvSingle => merge_shape::<MvRegister<JsonValue>>(local_value, remote_value),
            Counter => merge_shape::<PnCounter>(local_value, remote_value),
            OrderedSequence => merge_shape::<Rga<JsonValue>>(local_value, remote_value),
            AppendOnlyLog => {
                // NOT dispatched via CRDT merge (Blueprint §7.5.5): there is
                // nothing to merge, only causally-ordered retention.
                MergeOutcome::BothRetained
            }
            AuthorityControlled => {
                // SHOULD NEVER BE DISPATCHED. §4.2: SynchronizationSession
                // must intercept authority-controlled fields before calling
                // merge_field at all. If we get here regardless, always
                // create a ConflictRecord rather than silently pick a value.
                MergeOutcome::Conflict {
                    conflict_type: ConflictType::AuthorityField,
                }
            }
        }
    }
}

/// Deserialize both sides into the concrete CRDT type `T` for this field
/// shape, merge via `Crdt::merge`, and re-serialize. Falls back to
/// `ConflictType::CustomMergeRequired` if either side fails to deserialize
/// as `T` — this is a real, surfaced failure mode, not a silently-swallowed
/// error, since a malformed value for a declared shape is itself a data
/// integrity problem the caller must see.
fn merge_shape<T>(local_value: Value, remote_value: Value) -> MergeOutcome
where
    T: Crdt,
{
    let local: Result<T, _> = serde_json::from_value(local_value);
    let remote: Result<T, _> = serde_json::from_value(remote_value);

    match (local, remote) {
        (Ok(mut l), Ok(r)) => {
            l.merge(&r);
            match serde_json::to_value(&l) {
                Ok(v) => MergeOutcome::Applied { value: v },
                Err(_) => MergeOutcome::Conflict {
                    conflict_type: ConflictType::CustomMergeRequired,
                },
            }
        }
        _ => MergeOutcome::Conflict {
            conflict_type: ConflictType::CustomMergeRequired,
        },
    }
}

/// The result of attempting to merge two concurrent values for one field.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MergeOutcome {
    /// A single merged value was produced automatically.
    Applied {
        /// The merged value.
        value: Value,
    },
    /// Both sides are retained as-is (e.g. append-only log entries).
    BothRetained,
    /// No automatic merge was possible; a `ConflictRecord` must be created.
    Conflict {
        /// Why this field could not be auto-merged.
        conflict_type: ConflictType,
    },
}

/// Why a field could not be auto-merged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictType {
    /// The field is authority-controlled; concurrent writes always
    /// conflict, by design.
    AuthorityField,
    /// The field's declared CRDT shape couldn't deserialize one or both
    /// sides — a data integrity problem, not a business conflict.
    CustomMergeRequired,
    /// The values are semantically incompatible even though they
    /// deserialized successfully.
    SemanticConflict,
}
