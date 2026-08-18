# ONYX — Mission Operations Platform

[![Rust](https://img.shields.io/badge/Rust-2021-purple?logo=rust)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-purple?logo=tauri)](https://tauri.app/)
[![Web](https://img.shields.io/badge/web-react-61DAFB.svg?logo=react&logoColor=white)](https://reactjs.org)
[![TypeScript](https://img.shields.io/badge/typescript-5.x-3178C6.svg?logo=typescript&logoColor=white)](https://typescriptlang.org)
[![React](https://img.shields.io/badge/react-19.x-61DAFB.svg?logo=react&logoColor=white)](https://reactjs.org)
[![Flutter](https://img.shields.io/badge/flutter-3.x-02569B.svg?logo=flutter&logoColor=white)](https://flutter.dev)
[![Vite](https://img.shields.io/badge/vite-6.x-646CFF.svg?logo=vite&logoColor=white)](https://vitejs.dev)
[![Tailwind CSS](https://img.shields.io/badge/tailwindcss-4.x-38B2AC.svg?logo=tailwind-css&logoColor=white)](https://tailwindcss.com)
[![License: Proprietary](https://img.shields.io/badge/License-Proprietary-red.svg)](LICENSE.md)
[![Platforms](https://img.shields.io/badge/Platforms-Android%20%7C%20Windows%20%7C%20Linux-brightgreen)]()
[![Build Status](https://img.shields.io/badge/Build-passing-brightgreen)]()
[![Version](https://img.shields.io/badge/Version-1.3.1-yellow)]()
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Rust](https://img.shields.io/badge/rust-1.97.1+-orange.svg?logo=rust&logoColor=white)](https://rust-lang.org)
[![Android](https://img.shields.io/badge/Android-8.0%2B-green?logo=android)](https://developer.android.com/)
[![CI](https://github.com/So-Muzaff/Onyx-Framwork/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/So-Muzaff/Onyx-Framwork/actions/workflows/ci.yml)


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
