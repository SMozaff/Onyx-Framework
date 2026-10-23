#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
python3 "$ROOT/scripts/verify/verify_mobile_static.py"
python3 "$ROOT/scripts/verify/verify_team8_static.py" "$ROOT" --report "$ROOT/docs/TEAM8_STATIC_REPORT.json"
if command -v cargo >/dev/null; then
  cargo metadata --manifest-path "$ROOT/Cargo.toml" --no-deps --format-version 1 >/dev/null
fi
