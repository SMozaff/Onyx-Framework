# Team 3 — Synchronization Engine & Conflict Resolution

Increment 3 of the ONYX build: the offline-first reconciliation engine.
Two replicas — laptops, phones, cloud nodes — exchange independently
accepted operation histories and converge to identical authoritative state
without a central coordinator or continuous connectivity.

Implements the frozen `Team Prompt 3: Synchronization Engine & Conflict
Resolution` specification, rebuilt against Team 1's real `platform-kernel`/
`platform-contracts` crates (see `DECISIONS.md` at the workspace root for
every ruling and deviation that shaped this).

## Crates

| Crate | What it is |
|---|---|
| [`crdt`](./crdt) | The CRDT taxonomy (OR-Set, LWW-Register, MV-Register, PN-Counter, RGA), an append-only causal log, and causally-complete tombstone GC. Pure domain logic — no I/O, no async runtime. |
| `synchronization-domain` | `SynchronizationSession` (the 4-phase Discovery → Exchange → Reconciliation → Completion lifecycle), `MergeStrategy` (dispatches merges by field shape), `ConflictRecord` (the conflict aggregate), `EscalationService` (durably records escalation events when a conflict has been open too long). |
| `sync-test-utils` | In-memory mocks (`MockRepository`, `MockUnitOfWork`, `MockConflictRepository`) built against the *real* `query-application` ports, plus proptest generators, shared by both crates' test suites. |

Also added: `../kernel/platform-contracts-ext` — a local, additive-only copy
of Team 1's `platform-contracts` with `AggregateRoot::conflict_pending()`
added (Architectural Ruling Q2). Not authoritative; see its own doc comment.

## Quick Start

```bash
# Run every Team 3 test (fast — the full property test is skipped by default)
cargo test --package crdt --package synchronization-domain --package sync-test-utils --package platform-contracts-ext

# Run the full CRDT determinism property test (slower; explicit --ignored)
cargo test --package crdt --test determinism --release -- --ignored --nocapture

# Run just the tombstone GC tests
cargo test --package crdt --test tombstone_gc -- --nocapture

# Clippy, strict
cargo clippy --package crdt --package synchronization-domain --package sync-test-utils --package platform-contracts-ext --all-targets -- -D warnings

# Doc coverage
RUSTDOCFLAGS="-D missing_docs" cargo doc --no-deps --package crdt --package synchronization-domain --package sync-test-utils --package platform-contracts-ext
```

## The core guarantee

Every CRDT merge in `crdt` is commutative, associative, and idempotent:
replaying the same operations in any order on any replica converges to
identical final state. Authority-controlled fields never auto-merge —
concurrent writes to them always produce a `ConflictRecord` for human
review, by design (see `SynchronizationSession::reconcile()`'s fail-safe
default for fields with unknown metadata). Tombstone garbage collection is
gated purely on causal completeness, never on wall-clock time.

See `DECISIONS.md` at the workspace root for the full record of
architectural rulings, contract defects found and resolved, and every
deviation from the frozen Team Prompt 3 specification.
