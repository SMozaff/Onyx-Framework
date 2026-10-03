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
Every frontend surface must commit package-lock.json, use Node 22 in CI unless explicitly excepted, use npm ci, run npm audit at the high/critical threshold, validate its normal build/test/lint surface, document intentional major-version deviations, and participate in automated dependency updates.

## Dependency updates
Dependabot is the baseline automated update mechanism. Major updates require the full relevant CI surface. Security updates may be expedited, but still require reproducible CI validation.

## GitHub Actions
Third-party actions are dependencies and must be tracked. Security-sensitive actions should move toward immutable commit-SHA pinning where operationally practical. Workflows use least privilege and must not execute untrusted PR code in privileged pull_request_target contexts with secrets.

## Containers and deployment tooling
Container base images, Helm dependencies, Terraform providers, and deployment actions are dependency surfaces. Scheduled scanning should cover container/base-image vulnerabilities, Rust and Node advisories, Actions versions, and deployment/toolchain versions.

## Exceptions
Every exception records the affected component/version, advisory or CVE, remediation constraint, compensating control, owner, tracking issue, and expiry date. Expired exceptions fail security review.

## Review cadence
Dependency posture is reviewed for every release candidate and by the automated security workflow on every push/PR plus a scheduled weekly run.

## Container scanning and SBOM control

Container images under `deploy/docker/` are security-scanned in the Security workflow on pull requests, pushes to protected development branches, and the weekly scheduled scan. The scan builds each maintained ONYX image and fails on unfixed HIGH or CRITICAL vulnerabilities.

The same Security workflow generates an SPDX JSON software bill of materials for the repository at each verification run and retains it as a workflow artifact. Release processes should promote the SBOM associated with the exact candidate commit rather than regenerating it from a different source tree.

Container base-image changes are managed through the Docker Dependabot update stream and must pass the container-security gate before integration.
