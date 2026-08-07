# ONYX — Mission Operations Platform

A local‑first, authority‑aware mission operations system.

## Quick Start

```bash
cargo build --workspace --release
cargo test --workspace --release -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
```

## Architecture

- 8 increments, 27 crates, 6 binaries
- Offline‑first sync engine (CRDTs)
- P2P transports (Wi‑Fi Direct, BLE, QUIC, Cloud Relay)
- Web UI (React/TS) + Desktop (Tauri) + Mobile (Flutter/Rust FFI)

## Deployment

- Helm charts: `deploy/helm/`
- Dockerfiles: `deploy/docker/`
- Terraform: `deploy/terraform/`
- Runbooks: `docs/runbooks/`

## Contributing

- `cargo fmt`
- `cargo clippy --workspace --all-targets -- -D warnings`
- All PRs must pass CI


## Mobile v1.1

The Flutter application lives in `mobile/` and embeds `crates/mobile-core`.

```bash
mobile/tool/ensure_platform_scaffold.sh
mobile/tool/build_rust_android.sh
cd mobile && flutter pub get && flutter analyze && flutter test && flutter build apk
```

See `MOBILE_STATUS.md` for iOS, background-sync, P2P device-lab, and signing gates.
