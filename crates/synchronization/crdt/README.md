# crdt

CRDT (Conflict-Free Replicated Data Type) taxonomy for ONYX's offline-first
synchronization engine, plus a causally-complete tombstone garbage collector.

Built for Increment 3 (Team Prompt 3: Synchronization Engine & Conflict
Resolution) against Team 1's real `platform-kernel` crate. See
`DECISIONS.md` at the workspace root for the architectural rulings that
shaped this crate, and `ONYX_Increment_3_Synchronization_Engine_and_Conflict_Resolution.md`
for the frozen specification it implements.

## What's in this crate

| Type | File | Use it when... |
|---|---|---|
| `OrSet<T>` | `or_set.rs` | You need a set with add-wins semantics under concurrent add/remove (e.g. tags, watchers, participant lists). |
| `LwwRegister<V>` | `lww_register.rs` | Losing a concurrent write is acceptable — last writer (by timestamp, replica-id tie-break) wins. Good for non-authoritative annotations. |
| `MvRegister<V>` | `mv_register.rs` | Losing a concurrent write is **not** acceptable — all concurrent values are preserved for human/`MergeStrategy` review. |
| `PnCounter` | `pn_counter.rs` | A counter that can go up and down under concurrent increments/decrements from multiple replicas. |
| `Rga<E>` | `rga.rs` | An ordered sequence (list) with concurrent insert/delete support. |
| `AppendOnlyLog<E>` | `append_only_log.rs` | An append-only causal log (not technically a CRDT — no merge conflict, just causally-ordered retention). |
| `GarbageCollector` | `tombstone_gc.rs` | Reclaiming tombstone space once all known replicas have causally advanced past a deletion. |

All CRDT types (except `AppendOnlyLog`, deliberately) implement the `Crdt`
trait:

```rust
pub trait Crdt: Clone + Debug + Send + Sync + Serialize + DeserializeOwned {
    fn merge(&mut self, other: &Self) -> bool;
    fn causal_context(&self) -> &VectorClock;
    fn merge_into(&self, other: &Self) -> Self { /* ... */ }
}
```

## Determinism guarantee

Every CRDT merge in this crate is **commutative, associative, and
idempotent**. Applying the same set of operations in any order, on any
replica, converges to identical final state. This is proven by the property
test in `../../../tests/property/crdt_determinism.rs`, which replays a
random operation set in 10 different shuffled orders and asserts identical
results — run it explicitly (see Quick Start below), since it's marked
`#[ignore]` by default (it's slower than the rest of the suite).

## Tombstone GC: causal completeness, never wall-clock

`GarbageCollector` gates collection **only** on whether all known replicas
have causally advanced past a tombstone — never on elapsed time. There is no
`Duration`, `Instant`, or `SystemTime` anywhere in `tombstone_gc.rs`'s actual
code (only in a doc comment describing the rule). A tombstone can be safely
collected the instant every replica has seen it, whether that's a
millisecond or a year later; conversely it is *never* collected just because
"enough time has passed," since a still-offline replica could resurrect a
causal dependency on it.

## A note on `MvRegister`'s storage

Team Prompt 3 v1.0's frozen contract stored `MvRegister`'s concurrent
versions in a `BTreeMap<VectorClock, V>`, which requires `VectorClock: Ord`.
The real `platform_kernel::VectorClock` (Team 1's authoritative type) has no
`Ord` impl, and — since vector clocks are only ever *partially* ordered by
design — adding one ourselves risked it leaking into causal-comparison code
where it doesn't belong. `MvRegister` instead stores concurrent versions as
a small `Vec<(VectorClock, V)>`. Lookups are O(n), but n is the number of
genuinely concurrent writers to one field, which is expected to be tiny.

## Quick Start

```bash
# Run all crdt tests (fast — the property test is skipped by default)
cargo test --package crdt

# Run the full determinism property test (slower; explicit --ignored)
cargo test --package crdt --test determinism --release -- --ignored --nocapture

# Run just the tombstone GC tests
cargo test --package crdt --test tombstone_gc -- --nocapture

# Confirm no wall-clock code exists in the GC engine
grep -n "Duration::\|Instant::\|SystemTime::\|\.elapsed(" src/tombstone_gc.rs
```
