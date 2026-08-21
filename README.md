# ONYX — Mission Operations Platform
https://onyxcase-bxl5ndbk.manus.space
A local‑first, authority‑aware mission operations system.

## Development environment

[![Open in GitHub Codespaces](https://github.com/codespaces/badge.svg)](https://codespaces.new/muzaff-beep/Onyx-Framwork)

Development happens in the devcontainer (`.devcontainer/`), not on a bare
local machine. Click the badge above, or **Code → Codespaces → Create
codespace** on this repo. The container provisions Rust (pinned to the exact
`rust-toolchain.toml` version), Flutter, the Android SDK, `cargo-ndk`, and
Node — every toolchain this workspace needs, versioned to match CI
(`.github/workflows/ci.yml`) rather than whatever happens to be on a given
laptop. First boot takes several minutes while `.devcontainer/setup-mobile.sh`
installs Flutter and the Android SDK; enabling
[Codespaces prebuilds](https://docs.github.com/en/codespaces/prebuilding-your-codespaces)
for this repo removes that wait for everyone after the first prebuild.

`docs/RUN_LOCALLY.md` covers running outside a Codespace for the rare case
that requires it (e.g. a physical device on a local network for P2P testing)
— read its banner first.

## Quick Start

Once inside the Codespace / devcontainer:

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
