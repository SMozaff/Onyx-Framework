# ONYX Repository Structure

**Inventory baseline:** `main` at `e0708eb5ebc2502c601039ee61087678d38bfe8f` (2026-10-23). This is a path-level inventory, not a build or deployment report.

## Root

- `Cargo.toml`, `Cargo.lock` — Rust workspace and dependency lockfile. The workspace currently declares **42 members**.
- `README.md` — commercial product overview.
- `DECISIONS.md` — project-wide decision record.
- `ONYX_PHASE_1_REPORT.md` — historical security/remediation report; consult its current-status addendum before using older instructions.
- `LICENSE.md`, `LEGAL/` — proprietary notice and ownership/provenance records.
- `.github/workflows/` — CI, security, release, Cloudflare Worker, and authentication smoke/deployment workflows.

## Main implementation areas

| Path | Current role |
|---|---|
| `crates/kernel/`, `crates/domains/`, `crates/applications/` | Rust kernel, domain, and application layers |
| `crates/infrastructure/` | Persistence, messaging, security, observability, jobs, and storage adapters |
| `crates/bins/api-server/` | Native Rust/Axum API |
| `crates/bins/worker/` | Native Rust background worker |
| `crates/bins/admin-shell/`, `crates/bins/desktop-shell/` | Desktop/Admin clients and native integration |
| `crates/mobile-core/`, `crates/mobile-android-jni/` | Rust mobile core and Android JNI bridge |
| `deploy/cloudflare/edge-gateway/` | Cloudflare Worker API, D1-backed auth/data routes, Durable Object hubs |
| `mobile-android/` | Native Android application (Kotlin + Rust bridge) |
| `mobile-pwa/` | Web-based mobile Observer PWA |
| `web-ui/` | Main web client |
| `scripts/verify/` | Static/source verification tools |
| `deploy/docker/`, `deploy/helm/`, `deploy/terraform/` | Container, Kubernetes/Helm, and Terraform deployment assets |
| `docs/` | Architecture, authentication, verification, diagnostics, and project records |

## Explicit inventory corrections

- The old Flutter tree `mobile/` is **not present** in the inventory baseline. Do not use old Flutter path checks as current mobile evidence.
- Current mobile application surfaces are `mobile-android/` and `mobile-pwa/`; the Rust mobile core is under `crates/mobile-core/`.
- `docs/runbooks/` is not present in this baseline. Historical manifests claiming seven runbooks must not be treated as a current inventory.
- `web-ui/package-lock.json` is present.
- `crates/applications/client-composition/src/handlers/mod.rs` is present; the old sibling `handlers.rs` and root `_DELETE_FILES.txt` are absent.
- The native Axum API and Cloudflare Worker are separate backends. See [Backend Architecture](BACKEND_ARCHITECTURE.md).

This file intentionally describes major current paths rather than duplicating a full generated tree. Regenerate detailed inventories from the checked-out repository instead of editing this file from memory.
