# ONYX — Mission Operations Platform
https://onyxcase-bxl5ndbk.manus.space


Overview
ONYX is a local-first, authority-aware mission operations system designed for offline-capable team collaboration with robust synchronization capabilities. It's built as a production-grade Rust workspace with multi-platform client support.
Key Features
Offline-First Architecture: Uses CRDTs (Conflict-free Replicated Data Types) for seamless data synchronization across devices
Multi-Platform Support:
Web UI (React/TypeScript with Vite)
Desktop (Tauri-based)
Mobile (Flutter with Rust FFI via mobile-core)
P2P Transports: Supports Wi-Fi Direct, Bluetooth LE, QUIC, and Cloud Relay for device-to-device communication
Authority-Aware: Implements hierarchical command structures with proper authorization controls
Architecture
The codebase follows Clean Architecture principles organized into 8 increments with 41 crates and 6 binaries:
Core Layers:
Kernel (crates/kernel/): Platform primitives (IDs, versioning, causality, authority)
Domains (crates/domains/): Business logic for missions, work, communications, files, policies, profiles, todos, notifications
Applications (crates/applications/): Query, worker, security, audit, and client composition services
Infrastructure (crates/infrastructure/): Persistence (PostgreSQL/SQLite), messaging, observability, security adapters
Synchronization (crates/synchronization/): CRDT implementations and sync domain logic
Transports (crates/transports/): Sync transport protocols including mobile-specific implementations
Binaries (crates/bins/): API server, worker, sync-agent, migration tool, desktop shell, admin shell
Development Setup
Development occurs in a devcontainer/GitHub Codespaces environment that provisions all required toolchains (Rust, Flutter, Android SDK, Node.js) matching CI configurations. The project uses a pinned Rust version (1.97.1) defined in rust-toolchain.toml.
Testing Strategy
Comprehensive test coverage including:
Unit tests
Property-based tests (using proptest)
Integration tests
End-to-end tests (crates/team8-e2e-tests)
Chaos tests (crates/team8-chaos-tests)
Contract verification scripts
Deployment
Production-ready deployment infrastructure:
Helm charts for Kubernetes deployments
Dockerfiles for containerized services
Terraform configurations for infrastructure-as-code
Runbooks for operations (incident response, backup/restore, disaster recovery)
Documentation
Extensive documentation including:
Architectural Decision Records (ADRs) in DECISIONS.md
API specifications in docs/api/
Team implementation reports (Teams 6-8)
Mobile development status and verification reports
Design documents for user hierarchy and escalation systems
This is a sophisticated, enterprise-grade platform designed for mission-critical operations requiring reliable offline functionality and secure multi-party collaboration.
