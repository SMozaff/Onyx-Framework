#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"
DB="$(mktemp -u /tmp/onyx-idempotency-XXXXXX.db)"
trap 'rm -f "$DB"' EXIT
export DATABASE_URL="sqlite://${DB}?mode=rwc"
export ONYX_DATABASE_KIND=sqlite
cargo run -p migration-tool -- up
cargo run -p migration-tool -- up
cargo run -p migration-tool -- down --target 0
cargo run -p migration-tool -- up
