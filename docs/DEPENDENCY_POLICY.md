# ONYX Dependency & Software Supply-Chain Policy

Status: Active control-plane policy
Scope: Rust, Node/frontend, GitHub Actions, containers, deployment tooling, lockfiles, and release metadata.

## Purpose
ONYX treats dependency security as a lifecycle control, not a one-time upgrade exercise. Every dependency surface must have a reproducible version source, an owner, an update path, and an explicit vulnerability disposition.

## Repository-wide rules
- Dependency manifests and lockfiles are committed.
- CI uses reproducible installation/build commands and must not silently rewrite dependency state.
- High and critical known vulnerabilities fail CI unless an explicit, time-bounded exception is recorded.
- Dependency upgrades are validated by the complete relevant test/build surface.
- Exceptions require an owner, rationale, affected versions, mitigation, tracking issue, and expiry date.
- Security fixes are preferred over unrelated modernization; broad upgrades must be split into diagnosable batches.

## Rust
- Cargo.lock is authoritative for reproducible application builds.
- CI runs cargo metadata --locked, cargo audit, and cargo deny advisory checks.
- SQLx offline metadata under .sqlx/ is committed and checked with cargo sqlx prepare --check.
- RustSec high/critical findings require remediation or a documented exception.

## Frontend / Node
Frontend surfaces may intentionally use different React/Vite/TypeScript/Vitest generations, but version diversity must not become unmanaged drift.

### Repository baseline
- Node: 22.x LTS. CI must use Node 22 for every frontend surface.
- npm: 10.x. CI verifies the npm major before installing dependencies.
- Install: npm ci only in reproducible CI/dependency-audit jobs.
- Lockfiles: every frontend commits package-lock.json; lockfile drift must fail npm ci.
- Security: npm audit --omit=dev --audit-level=high is the enforced production-dependency gate.
- Updates: Dependabot proposes weekly npm updates for every frontend surface; major updates require the full relevant CI surface.
- Exceptions: any deviation from the Node/npm baseline or supported toolchain requires a recorded owner, rationale, affected surface/version, compensating validation, and expiry.

### Current intentional toolchain deviations
The fleet is not forcibly synchronized. The following major-version differences are intentional and are tracked here so they remain auditable:
- web-ui: React 18, Vite 5, TypeScript 5, Vitest 1, Playwright 1.x. This is the legacy web surface and is upgraded independently from the native shells.
- mobile-pwa: React 18, Vite 7, TypeScript 5, Vitest 4, Playwright 1.x. It shares the web platform model but has a newer build/test stack.
- desktop-shell/ui: React 19, Vite 8, TypeScript 6, oxlint 1.x; Tauri 2.x. This is the native Staff UI and deliberately follows the current Tauri toolchain.
- admin-shell/ui: React 19, Vite 8, TypeScript 6, oxlint 1.x; Tauri 2.x. This is the native Admin UI and follows the same native-shell baseline as desktop-shell/ui.

The repository security workflow identifies each frontend surface independently, runs the same Node/npm baseline check and high-severity audit, and the CI workflow runs the complete build/test/lint surface appropriate to that frontend.

## Dependency updates
Dependabot is the baseline automated update mechanism. Major updates require the full relevant CI surface. Security updates may be expedited, but still require reproducible CI validation.

## GitHub Actions
Third-party actions are dependencies and must be tracked. All GitHub Actions references in repository workflows are immutable commit-SHA pins, with the release tag retained as a comment for auditability. Dependabot tracks the github-actions ecosystem and proposes SHA/tag refreshes; every refresh must pass the complete relevant CI/security surface. Workflows use least privilege and must not execute untrusted PR code in privileged pull_request_target contexts with secrets.

## Containers and deployment tooling
Container base images, Helm dependencies, Terraform providers, and deployment actions are dependency surfaces. Scheduled scanning should cover container/base-image vulnerabilities, Rust and Node advisories, Actions versions, and deployment/toolchain versions.

## Exceptions
Every exception records the affected component/version, advisory or CVE, remediation constraint, compensating control, owner, tracking issue, and expiry date. Expired exceptions fail security review.

## Review cadence
Dependency posture is reviewed for every release candidate and by the automated security workflow on every push/PR plus a scheduled weekly run. Frontend build/release workflows use the repository-wide Node 22 baseline; legacy Node 20 references are prohibited.

## Container scanning and SBOM control

Container images under `deploy/docker/` are security-scanned in the Security workflow on pull requests, pushes to protected development branches, and the weekly scheduled scan. The scan builds each maintained ONYX image and fails on unfixed HIGH or CRITICAL vulnerabilities.

The same Security workflow generates an SPDX JSON software bill of materials for the repository at each verification run and retains it as a workflow artifact. Release processes should promote the SBOM associated with the exact candidate commit rather than regenerating it from a different source tree.

Container base-image changes are managed through the Docker Dependabot update stream and must pass the container-security gate before integration.
