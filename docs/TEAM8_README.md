# ONYX Team 8 — Historical Release Engineering Handoff

**Revalidated against main:** `e0708eb5ebc2502c601039ee61087678d38bfe8f` (2026-10-23).

> This is a technical handoff record, not evidence of a completed release or production readiness. Older release claims and instructions have been reconciled against the current repository. ONYX's current objective is a demo; the checklist below describes existing source/verification entry points only.

## Current source inventory

The current repository contains Dockerfiles, Helm charts, Terraform files, a Rust migration tool, a native sync agent, Team 8 E2E/chaos test crates, and release/CI workflows. The workspace has 42 members. The current E2E sources are under `crates/team8-e2e-tests/`, `crates/team8-chaos-tests/`, and `tests/end-to-end/`.

The following files referenced by the older handoff are **not present** in this baseline:

- `docs/TEAM8_VERIFICATION.md`
- `docs/release/go-live-checklist.md`
- `docs/release/signoff-template.md`
- `docs/runbooks/` (the historical manifest lists seven runbooks, but the directory is absent)

Do not treat historical mentions of those files as proof that they exist or have been completed.

## Existing verification entry points

```bash
scripts/ci-pipeline.sh
python3 scripts/verify/verify_team8_static.py . --report docs/TEAM8_STATIC_REPORT.json
cargo test -p team8-e2e-tests --test all_journeys
cargo test -p team8-chaos-tests --test all
```

These are source-defined commands, not a claim that they have been executed successfully for the current commit. The static verifier must pass against current source before its result can be called green.

## Deployment assets and limitations

- Container definitions: `deploy/docker/`
- Helm charts: `deploy/helm/`
- Terraform baseline: `deploy/terraform/`
- CI/release workflows: `.github/workflows/ci.yml` and `.github/workflows/release.yml`

Presence of these files does not prove that a cloud environment, release credentials, monitoring, backup/restore, or live rollout is configured or working. Verify current workflow runs and deployment records before making runtime claims.

## Lockfile status

The current `Cargo.lock` includes `testcontainers-modules` and `opentelemetry-otlp`, and `web-ui/package-lock.json` exists. This corrects the older report's blanket statements that those dependencies were absent from the lockfile and that the web UI lockfile was missing. Their presence alone does not establish a fully successful locked build; use current CI evidence for that conclusion.

## Evidence status

The former 345/345 `TEAM8_STATIC_REPORT.json` result has been invalidated because the associated verifier included stale required paths and status assumptions. The current JSON records source inventory only until the verifier is reconciled and run. No production deployment or go-live acceptance is claimed.
