#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

cargo test -p crdt --test tombstone_gc

SOURCE='crates/synchronization/crdt/src/tombstone_gc.rs'
# Garbage collection is gated by causal completeness, never elapsed time.
if grep -nE 'Duration::|Instant::|SystemTime::|\.elapsed\(' "$SOURCE"; then
  echo 'tombstone GC contains forbidden wall-clock eligibility logic' >&2
  exit 1
fi
grep -q 'summary_vector' "$SOURCE"
grep -q 'is_tombstone_eligible' "$SOURCE"
