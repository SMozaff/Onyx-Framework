# ONYX Team 8 — Release Engineering Handoff

Team 8 finalizes the backend release surface defined by the frozen Team 8 Execution Prompt v1.0. The repository now contains migration tooling, CI/CD workflows, hardened container definitions, Helm/Argo Rollouts deployment charts, an AWS Terraform baseline, backend E2E/chaos/load harnesses, operational runbooks, release signing, SPDX 2.3 SBOM generation, and formal go-live controls.

## Primary commands

```bash
# Complete local quality pipeline
scripts/ci-pipeline.sh

# Mandatory backend journeys
cargo test -p e2e --test all_journeys

# Deterministic chaos suite
cargo test -p chaos --test all

# Real-time 15-minute chaos drills (run in staging)
ONYX_CHAOS_REALTIME=1 cargo test -p chaos --test all

# Load acceptance gate
k6 run tests/load/smoke-test.js --vus 100 --duration 60s

# Deployment validation
helm lint deploy/helm/onyx-api
helm lint deploy/helm/onyx-worker
helm lint deploy/helm/onyx-sync-agent
terraform -chdir=deploy/terraform init -backend=false
terraform -chdir=deploy/terraform validate

# Release contract verification
scripts/verify/verify_team8.sh
```

## Migration CLI

```bash
export DATABASE_URL='postgres://onyx:onyx@localhost:5432/onyx'
export ONYX_DATABASE_KIND=postgres
cargo run -p migration-tool -- up
cargo run -p migration-tool -- status
cargo run -p migration-tool -- down --target 0
cargo run -p migration-tool -- create add_example
```

`up` executes the migrator twice as the R10 idempotency gate. `create` emits paired PostgreSQL and SQLite up/down files.

## Production database contract

The API rollout is horizontally scaled and therefore uses one shared PostgreSQL operational store. In production, the `onyx-api-secrets` Kubernetes Secret must contain:

- `DATABASE_URL` — PostgreSQL URL for aggregate projections, commands, events, outbox and idempotency.
- `ONYX_GOVERNANCE_DATABASE_URL` — PostgreSQL URL for rate governance and audit; it may point to the same cluster/database when approved.
- `ONYX_AUTHORITY_SIGNING_KEY` — current Ed25519 signing seed.

`ONYX_ENV=production` rejects SQLite. SQLite remains available only for isolated local/test composition. `/health` is process liveness; `/ready` verifies the operational database and is the Kubernetes readiness target. Connection URLs are not emitted in startup logs.

## Deployment prerequisites

Production installation requires:

- Kubernetes namespace `onyx`.
- Argo Rollouts and its kubectl plugin.
- NGINX ingress integration for weighted canary traffic.
- Prometheus reachable from the AnalysisTemplate.
- Jaeger/OpenTelemetry OTLP collector.
- PostgreSQL with PITR and backup retention configured.
- Cosign/OIDC permissions and a protected GPG release key.
- Syft for explicit SPDX 2.3 JSON output.

## Release sequence

1. Generate and review current dependency lockfiles.
2. Run the complete CI pipeline.
3. Execute the five staging chaos scenarios in real-time mode.
4. Run the 100-user load gate.
5. Validate restore and disaster-recovery procedures against the RTO/RPO targets.
6. Tag `v1.0.0` to start `.github/workflows/release.yml`.
7. Verify image digest signatures, GPG signatures, SBOMs, and attestations.
8. Deploy the API canary and observe the 10% → 50% → 100% progression.
9. Complete `docs/release/go-live-checklist.md` and `docs/release/signoff-template.md`.

## Known sign-off blockers in this delivered archive

The inherited `Cargo.lock` predates Team 7/8 dependencies and `web-ui/package-lock.json` is absent. This execution environment could not access the registries required to regenerate them. Online CI and Docker builders regenerate the Cargo lock graph before locked builds, and Web CI falls back to `npm install`; however, production sign-off requires reviewing and committing refreshed Rust and npm lockfiles.

Runtime acceptance status is recorded in `TEAM8_VERIFICATION.md`. No unchecked go-live item should be treated as passed.
