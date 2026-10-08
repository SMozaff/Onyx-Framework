# ONYX Team 8 Verification Record

**Increment:** 8 — Testing, Deployment, and Operational Handover  
**Specification:** Team 8 Execution Prompt v1.0, frozen 2026-08-05  
**Verification environment:** tool-constrained container without Rust, Docker, PostgreSQL, Helm, Terraform, k6, or public package-registry access

## Delivered acceptance surface

- Migration CLI: `up`, targeted `down`, `status`, and paired `create`.
- Five Rust service Dockerfiles using the frozen builder/runtime bases.
- Helm charts for API, worker, and sync-agent in namespace `onyx`.
- Argo Rollouts 10% → 50% → 100% canary over one hour with >1% error abort.
- AWS Terraform baseline for EKS, encrypted Multi-AZ RDS, ECR, KMS, backups, and logs.
- GitHub Actions full/increment/release workflows.
- Testcontainers-backed backend E2E Journeys 1–4.
- Client-dependent Journeys 5–7 present and ignored under R11.
- Five 15-minute logical chaos scenarios plus staging real-time mode.
- k6 100-user/60-second command load gate with p95 <500 ms.
- Seven operational runbooks, go-live checklist, and sign-off template.
- Cosign image-digest signing, GPG binary signing, SPDX 2.3 SBOMs, and provenance/SBOM attestations.
- PostgreSQL-backed production API composition with fail-closed SQLite rejection, database-aware `/ready`, and PostgreSQL-backed mandatory Approval E2E coverage.

## Checks executed in this environment

The final offline verifier completed with **345/345 checks passed** and zero static failures. The machine-readable result is `TEAM8_STATIC_REPORT.json`.

The Team 8 delta contains **91 files** relative to the delivered Team 7 repository: 81 added and 10 modified, with no deletions. See `TEAM8_CHANGED_FILES.txt`.

Validated evidence includes:

- Presence of every Team 8 CI/CD, deployment, migration, test, runbook, and handoff artifact.
- Docker base images, non-root runtime users, and online lockfile refresh before locked builds.
- Namespace `onyx`, Argo Rollout resources, 10/50/100 weights, two 30-minute pauses, and the 1% rollback expression.
- Prometheus error-rate query compatibility with the emitted `status` and `service` labels.
- Terraform release controls including EKS, encrypted RDS, Multi-AZ policy, backup retention, KMS rotation, ECR immutability/lifecycle, and private backup storage.
- Migration CLI surface and complete PostgreSQL/SQLite up/down pair coverage.
- Real SQLite execution of all migrations, a second SQLx-like idempotency pass, frozen Team 6 seed execution, and reverse rollback to the migration ledger only.
- Testcontainers harness/seed coupling, mandatory Journey 1–4 presence, and ignored Journey 5–7 annotations.
- Five chaos scenarios, 900-second duration contract, and real-time staging switch.
- k6 100-VU/60-second/p95 contract.
- Cosign digest signing, GPG signing, Syft `spdx-json@2.3`, image provenance/SBOM attestations, and safe file checksum enumeration.
- Workflow and Helm values YAML parsing.
- Bash syntax for all shell scripts and JavaScript syntax for the k6 test.
- JSON validation for 23 files and TOML validation for 34 manifests/configuration files.
- Delimiter integrity for Team 8-authored Rust sources.

Static warnings retained in the report:

1. `Cargo.lock` predates Team 7/8 dependencies.
2. `web-ui/package-lock.json` is absent.

These warnings are formal release blockers under T8-D12 and do not reduce the accuracy of the static source audit.

## Runtime gates not executable here

The following gates require an approved networked CI/staging environment and are **not represented as passed** in this archive:

- Rust compilation, formatting, Clippy, documentation, and Cargo tests.
- Testcontainers PostgreSQL execution of Journeys 1–4.
- Real PostgreSQL migration up/down/status runs.
- Docker builds and runtime health checks.
- Helm lint/template against the installed Argo Rollouts CRDs.
- Terraform provider initialization and validation.
- Real-time 15-minute chaos drills.
- k6 performance measurement.
- Syft, Cosign, GPG, and GitHub attestation execution.
- Canary progression and rollback observation.
- Backup/restore and disaster-recovery drills.

## Release blockers

1. Regenerate and commit `Cargo.lock` with Team 7/8 dependencies.
2. Generate and commit `web-ui/package-lock.json`.
3. Execute every unchecked item in `docs/release/go-live-checklist.md`.
4. Obtain the named stakeholder signatures; this package does not invent sign-off.
