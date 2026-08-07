#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

run() { printf '\n==> %s\n' "$*"; "$@"; }
require() { command -v "$1" >/dev/null || { echo "missing required tool: $1" >&2; exit 2; }; }

require cargo
run cargo fmt --all -- --check
run cargo check --workspace --all-targets
run cargo test --workspace --exclude e2e --exclude chaos
run cargo test -p e2e --test all_journeys
run cargo test -p chaos --test all
run cargo clippy --workspace --all-targets -- -D warnings
run cargo doc --workspace --no-deps

DB_FILE="$(mktemp -u /tmp/onyx-migrations-XXXXXX.db)"
export DATABASE_URL="sqlite://${DB_FILE}?mode=rwc"
export ONYX_DATABASE_KIND=sqlite
run cargo run -p migration-tool -- up
run cargo run -p migration-tool -- up
run cargo run -p migration-tool -- status
run cargo run -p migration-tool -- down --target 0
run cargo run -p migration-tool -- up
rm -f "$DB_FILE"

if command -v npm >/dev/null; then
  (cd web-ui && if [[ -f package-lock.json ]]; then npm ci; else npm install; fi && npm run type-check && npm test && npm run test:a11y && npm run feature-audit && npm run build && npm run bundle-check)
fi
if command -v helm >/dev/null; then
  run helm lint deploy/helm/onyx-api
  run helm lint deploy/helm/onyx-worker
  run helm lint deploy/helm/onyx-sync-agent
  run helm template onyx-api deploy/helm/onyx-api --namespace onyx
fi
if command -v terraform >/dev/null; then
  (cd deploy/terraform && terraform fmt -check && terraform init -backend=false && terraform validate)
fi
run scripts/verify/verify_team8.sh
