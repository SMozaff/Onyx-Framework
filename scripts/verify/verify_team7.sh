#!/usr/bin/env bash
set -euo pipefail

python scripts/verify/verify_team7_static.py
cargo fmt --all -- --check
cargo check --workspace
cargo test -p observability-adapter
cargo test -p security-adapter
cargo test -p background-jobs
cargo test -p team7-integration-tests --test integration
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
