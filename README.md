# ONYX — Mission Operations Platform

[![Open in GitHub Codespaces](https://github.com/codespaces/badge.svg)](https://codespaces.new/muzaff-beep/Onyx-Framwork)
[![CI](https://github.com/muzaff-beep/Onyx-Framwork/actions/workflows/ci.yml/badge.svg)](https://github.com/muzaff-beep/Onyx-Framwork/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**A local-first, authority-aware mission operations platform built for offline collaboration and secure multi-device synchronization.**

🌐 **Live Demo**: https://onyxcase-bxl5ndbk.manus.space

---

## 🚀 Overview

ONYX is a production-grade mission operations system designed for environments where connectivity cannot be assumed. Built with Rust at its core, it enables seamless collaboration across web, desktop, and mobile platforms with automatic conflict resolution through CRDT-based synchronization.

### Key Features

- **offline-first Architecture**: Full functionality without internet connectivity
- **🔄 Smart Synchronization**: CRDT-based sync engine with automatic conflict resolution
- **📡 Multi-Transport P2P**: Wi-Fi Direct, Bluetooth LE, QUIC, and Cloud Relay
- **🔐 Authority-Aware**: Role-based permissions and secure collaboration
- **📱 Cross-Platform**: Web (React/TypeScript), Desktop (Tauri), Mobile (Flutter + Rust FFI)
- **🏗️ Clean Architecture**: 27 crates across 8 domain-driven increments

---

## 🛠️ Development Environment

**Development happens exclusively in the devcontainer**, not on bare metal. This ensures consistent tooling across all contributors.

### Getting Started

1. **Create a Codespace**: Click the badge above or go to **Code → Codespaces → Create codespace**
2. **Wait for provisioning**: First boot takes ~5-10 minutes while Flutter, Android SDK, and `cargo-ndk` are installed
3. **Start developing**: All toolchains are pre-configured to match CI

> 💡 **Pro Tip**: Enable [Codespaces prebuilds](https://docs.github.com/en/codespaces/prebuilding-your-codespaces) to reduce wait time after the first build.

### Local Development (Advanced)

For rare cases requiring local execution (e.g., physical device P2P testing), see [`docs/RUN_LOCALLY.md`](docs/RUN_LOCALLY.md).

---

## ⚡ Quick Start

Once inside the Codespace/devcontainer:

```bash
# Build all workspace crates
cargo build --workspace --release

# Run full test suite
cargo test --workspace --release -- --test-threads=1

# Lint with strict warnings
cargo clippy --workspace --all-targets -- -D warnings

# Format code
cargo fmt --all
```

---

## 🏛️ Architecture

ONYX follows Clean Architecture principles with Domain-Driven Design:

### Structure
- **8 Increments**: Progressive delivery milestones
- **27 Crates**: Modular, reusable components
- **6 Binaries**: Deployable applications
- **8 Domains**: Mission, Work, Communication, File, Policy, Profile, Todo, Notification

### Core Components

| Component | Technology | Location |
|-----------|------------|----------|
| Sync Engine | Rust + CRDTs | `crates/sync-*` |
| P2P Transports | Wi-Fi Direct, BLE, QUIC | `crates/transport-*` |
| Web UI | React/TypeScript | `web/` |
| Desktop App | Tauri | `desktop/` |
| Mobile App | Flutter + Rust FFI | `mobile/` |
| Infrastructure | Docker, Helm, Terraform | `deploy/` |

---

## 📦 Deployment

Production-ready deployment configurations:

- **Kubernetes**: [`deploy/helm/`](deploy/helm/) - Helm charts for K8s clusters
- **Containers**: [`deploy/docker/`](deploy/docker/) - Optimized Dockerfiles
- **Infrastructure as Code**: [`deploy/terraform/`](deploy/terraform/) - Cloud provisioning
- **Operations**: [`docs/runbooks/`](docs/runbooks/) - Incident response & maintenance

---

## 📱 Mobile Development

The Flutter application (`mobile/`) embeds `crates/mobile-core` for native performance.

### Build Pipeline

```bash
# Ensure platform scaffolds
mobile/tool/ensure_platform_scaffold.sh

# Build Rust libraries for Android
mobile/tool/build_rust_android.sh

# Full mobile build & test
cd mobile && flutter pub get && flutter analyze && flutter test && flutter build apk
```

### Platform Status

See [`MOBILE_STATUS.md`](MOBILE_STATUS.md) for:
- ✅ iOS build status
- 🔄 Background sync implementation
- 📱 P2P device lab setup
- 🔐 Code signing requirements

---

## 🤝 Contributing

We welcome contributions! Please follow these guidelines:

### Code Quality Requirements

```bash
# All PRs must pass:
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release
```

### Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/your-feature`)
3. Make changes with tests
4. Ensure CI passes
5. Submit PR with clear description

### Documentation

- Update relevant docs in `docs/`
- Add ADRs for architectural decisions
- Update runbooks for operational changes

---

## 📚 Documentation

| Document | Description |
|----------|-------------|
| [`docs/RUN_LOCALLY.md`](docs/RUN_LOCALLY.md) | Local development setup |
| [`docs/runbooks/`](docs/runbooks/) | Operational procedures |
| [`MOBILE_STATUS.md`](MOBILE_STATUS.md) | Mobile platform status |
| `docs/adr/` | Architectural Decision Records |

---

## 🧪 Testing Strategy

- **Unit Tests**: Per-crate functionality
- **Property-Based Tests**: Edge case coverage
- **Integration Tests**: Cross-crate interactions
- **E2E Tests**: Full workflow validation
- **Chaos Tests**: Resilience under failure conditions

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

Built with ❤️ using Rust, Flutter, React, and modern web technologies.

Special thanks to the CRDT, P2P, and offline-first communities for inspiration and research.
