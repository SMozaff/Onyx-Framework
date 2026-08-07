#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

# Fast deterministic smoke coverage runs on every commit. The full property
# suite remains ignored by default and is executed explicitly in release CI.
cargo test -p crdt --test determinism or_set_merge_deterministic_smoke -- --exact
cargo test -p crdt --test determinism --release -- --ignored

# The implementation must not introduce non-deterministic wall-clock or
# random winner selection into CRDT merge code.
if grep -RInE 'SystemTime|Instant::now|thread_rng\(|random::<|rand::' \
  crates/synchronization/crdt/src --include='*.rs'; then
  echo 'CRDT implementation contains a non-deterministic source' >&2
  exit 1
fi
