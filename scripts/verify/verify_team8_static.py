#!/usr/bin/env python3
"""Static Team 8 contract verifier for tool-constrained environments."""
from __future__ import annotations

import argparse
import json
import re
import sqlite3
import subprocess
import sys
import tempfile
from pathlib import Path

try:
    import yaml
except ImportError:  # pragma: no cover - optional locally, present in CI image
    yaml = None

parser = argparse.ArgumentParser()
parser.add_argument("root", nargs="?", default=".")
parser.add_argument("--report")
args = parser.parse_args()
ROOT = Path(args.root).resolve()
checks = 0
failures: list[str] = []
warnings: list[str] = []


def check(condition: bool, message: str) -> None:
    global checks
    checks += 1
    if not condition:
        failures.append(message)


def warn(condition: bool, message: str) -> None:
    if not condition:
        warnings.append(message)


def text(path: str) -> str:
    p = ROOT / path
    check(p.is_file(), f"missing file: {path}")
    return p.read_text(encoding="utf-8") if p.is_file() else ""


required = [
    # CI was consolidated from eight increment workflows into the current
    # platform- and purpose-based workflow set.
    ".github/workflows/ci.yml", ".github/workflows/release.yml",
    ".github/workflows/Debug.yml", ".github/workflows/devcontainer-check.yml",
    ".github/workflows/fmt-fix.yml",
    "scripts/ci-pipeline.sh", "scripts/release.sh", "scripts/canary-rollout.sh",
    "scripts/verify/verify_crdt_determinism.sh",
    "scripts/verify/verify_migration_idempotency.sh",
    "scripts/verify/verify_tombstone_gc.sh",
    *[f"deploy/docker/{name}.Dockerfile" for name in [
        "api-server", "worker", "sync-agent", "migration-tool", "desktop-shell"]],
    "deploy/docker-compose.local.yml",
    "deploy/helm/namespace.yaml",
    "deploy/helm/onyx-api/Chart.yaml", "deploy/helm/onyx-api/values.yaml",
    "deploy/helm/onyx-api/values-staging.yaml", "deploy/helm/onyx-api/values-production.yaml",
    "deploy/helm/onyx-api/templates/deployment.yaml",
    "deploy/helm/onyx-api/templates/service.yaml",
    "deploy/helm/onyx-api/templates/ingress.yaml",
    "deploy/helm/onyx-api/templates/hpa.yaml",
    "deploy/helm/onyx-worker/Chart.yaml", "deploy/helm/onyx-worker/values.yaml",
    "deploy/helm/onyx-worker/templates/deployment.yaml",
    "deploy/helm/onyx-sync-agent/Chart.yaml", "deploy/helm/onyx-sync-agent/values.yaml",
    "deploy/helm/onyx-sync-agent/templates/deployment.yaml",
    "deploy/terraform/main.tf", "deploy/terraform/variables.tf", "deploy/terraform/outputs.tf",
    "crates/bins/migration-tool/src/main.rs", "crates/bins/migration-tool/src/migrations.rs",
    "crates/bins/sync-agent/src/main.rs",
    "crates/bins/api-server/src/main.rs",
    "crates/bins/api-server/src/query_handler.rs",
    "crates/bins/api-server/src/routes/mod.rs",
    "crates/bins/api-server/src/routes/query.rs",
    "crates/bins/api-server/src/routes/command.rs",
    *[f"tests/end-to-end/{name}.rs" for name in [
        "test_harness", "mission_lifecycle", "task_workflow", "conflict_resolution",
        "approval_workflow", "notification_sync", "p2p_sync", "background_sync"]],
    *[f"tests/chaos/{name}.rs" for name in [
        "chaos_runtime", "network_partition", "process_crash", "disk_full",
        "clock_skew", "database_failover"]],
    "tests/load/smoke-test.js", "tests/load/performance_benchmark.rs",
    *[f"docs/runbooks/{name}.md" for name in [
        "incident-response", "backup-restore", "performance-tuning", "on-call",
        "disaster-recovery", "outbox-backlog", "conflict-resolution"]],
    "docs/release/go-live-checklist.md", "docs/release/signoff-template.md",
    "docs/TEAM8_README.md", "docs/TEAM8_VERIFICATION.md", "docs/DECISIONS.md",
]
for item in required:
    check((ROOT / item).is_file(), f"required Team 8 artifact absent: {item}")

# Docker contract and reproducible-online-build workaround.
for dockerfile in sorted((ROOT / "deploy/docker").glob("*.Dockerfile")):
    body = dockerfile.read_text(encoding="utf-8")
    check("FROM rust:1.97-slim AS builder" in body, f"wrong builder base: {dockerfile}")
    check("FROM debian:bookworm-slim" in body, f"wrong runtime base: {dockerfile}")
    check("cargo generate-lockfile && cargo build --locked --release" in body,
          f"locked Docker build does not refresh inherited stale lockfile: {dockerfile}")
    check("USER 10001" in body, f"runtime is not non-root: {dockerfile}")

# Helm/Argo rollout contract.
helm_values = text("deploy/helm/onyx-api/values.yaml")
helm_text = "\n".join(
    p.read_text(encoding="utf-8") for p in (ROOT / "deploy/helm").rglob("*.yaml")
)
check("namespace: onyx" in helm_text or "namespace: {{ .Values.namespace }}" in helm_text,
      "namespace onyx not encoded")
api_rollout = text("deploy/helm/onyx-api/templates/deployment.yaml")
check("kind: Rollout" in api_rollout and "kind: AnalysisTemplate" in api_rollout,
      "Argo Rollout/AnalysisTemplate missing")
for weight in ["setWeight: 10", "setWeight: 50", "setWeight: 100"]:
    check(weight in api_rollout, f"canary step missing: {weight}")
check(api_rollout.count("duration: 30m") == 2,
      "canary must pause 30m at 10% and 50%")
check("<= 0.01" in api_rollout, "canary error rollback threshold is not 1%")
check('status=~"5.."' in helm_values and 'outcome=~"5.."' not in helm_values,
      "canary query does not match requests_total status label")
check('service="onyx_api_server"' in helm_values, "canary query lacks API service label")
check('path: /ready' in api_rollout and 'path: /health' in api_rollout,
      "API rollout must separate readiness from liveness")
check("DATABASE_URL" in helm_values and "ONYX_GOVERNANCE_DATABASE_URL" in helm_values,
      "API Helm values do not document required database secret keys")

# Scaled API storage and secret-safe readiness contract.
api_routes = text("crates/bins/api-server/src/routes/mod.rs")
api_query = text("crates/bins/api-server/src/query_handler.rs")
api_main = text("crates/bins/api-server/src/main.rs")
docker_compose = text("deploy/docker-compose.local.yml")
api_docker = text("deploy/docker/api-server.Dockerfile")
for token in [
    "ProjectionPool::Postgres", "PostgresRepository::new",
    "PostgresUnitOfWorkFactory::new", "PostgresIdempotencyStore",
    "production API storage must use PostgreSQL",
]:
    check(token in api_routes, f"production PostgreSQL API composition missing: {token}")
check("EXTRACT(EPOCH FROM updated_at)" in api_query,
      "PostgreSQL projection query path missing")
check('.route("/ready", get(readiness))' in api_routes and 'SELECT 1' in api_routes,
      "database-aware API readiness endpoint missing")
check('%database_url' not in api_main and '%storage_backend' in api_main,
      "API startup logging may expose a credential-bearing database URL")
check("postgres://onyx:onyx@postgres:5432/onyx" in docker_compose
      and "sqlite:///data/onyx.db" not in docker_compose,
      "local release composition does not match shared PostgreSQL topology")
check("/ready" in api_docker, "API image health check does not validate database readiness")
api_command = text("crates/bins/api-server/src/routes/command.rs")
check("CommandError::EpochConflict { .. }" in api_command
      and "CommandError::NotFound(_)" in api_command
      and "CommandError::Idempotency(_)" in api_command
      and "LifecycleEpochConflict" not in api_command,
      "web command audit classification does not match delivered CommandError enum")

# Terraform release and recovery baseline.
terraform = text("deploy/terraform/main.tf")
for token in [
    'module "eks"', 'module "database"', "multi_az", "backup_retention_period",
    "aws_ecr_repository", "aws_ecr_lifecycle_policy", "aws_s3_bucket",
    "aws_s3_bucket_public_access_block", "storage_encrypted", "enable_key_rotation",
    "create_monitoring_role",
]:
    check(token in terraform, f"Terraform release resource/control missing: {token}")
check("transition { days" not in terraform and "encryption_configuration { encryption" not in terraform,
      "Terraform contains comma-separated inline block arguments")

# Migration CLI and reversible pairs.
migration_main = text("crates/bins/migration-tool/src/main.rs")
for command in ['"up"', '"down"', '"status"', '"create"']:
    check(command in migration_main, f"migration command missing: {command}")
migration_runner = text("crates/bins/migration-tool/src/migrations.rs")
check(migration_runner.count(".run(&pool)") >= 4, "up must execute each migrator twice")
check(".undo(&pool, target)" in migration_runner, "targeted down migration absent")
check('for database in ["postgres", "sqlite"]' in migration_runner,
      "migration create does not emit both database families")

pairs: dict[tuple[str, str], set[str]] = {}
for database in ["postgres", "sqlite"]:
    for path in (ROOT / "migrations" / database).glob("*.sql"):
        base = path.name.replace(".up.sql", "").replace(".down.sql", "")
        pairs.setdefault((database, base), set()).add("up" if ".up.sql" in path.name else "down")
for key, directions in pairs.items():
    check(directions == {"up", "down"}, f"migration pair incomplete: {key}")

# Execute SQLite migrations with SQLx-like tracking, seed fixture, then rollback.
try:
    with tempfile.TemporaryDirectory() as temp:
        db = sqlite3.connect(Path(temp) / "team8.db")
        db.execute("CREATE TABLE _sqlx_migrations (version INTEGER PRIMARY KEY, description TEXT, success BOOLEAN, installed_on TEXT)")
        applied: set[int] = set()
        ups = sorted((ROOT / "migrations/sqlite").glob("*.up.sql"))
        for _pass in range(2):
            for path in ups:
                version = int(path.name.split("_", 1)[0])
                if version in applied:
                    continue
                db.executescript(path.read_text(encoding="utf-8"))
                db.execute(
                    "INSERT INTO _sqlx_migrations VALUES (?, ?, 1, datetime('now'))",
                    (version, path.stem),
                )
                applied.add(version)
        db.executescript(text("tests/fixtures/seed.sql"))
        aggregate_count = db.execute("SELECT COUNT(*) FROM aggregates").fetchone()[0]
        check(aggregate_count >= 9, "seed fixture did not populate expected aggregate projections")
        for up in reversed(ups):
            version = int(up.name.split("_", 1)[0])
            down = up.with_name(up.name.replace(".up.sql", ".down.sql"))
            db.executescript(down.read_text(encoding="utf-8"))
            db.execute("DELETE FROM _sqlx_migrations WHERE version = ?", (version,))
        tables = {
            row[0] for row in db.execute(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
            )
        }
        check(tables == {"_sqlx_migrations"}, f"SQLite rollback left application tables: {sorted(tables)}")
        db.close()
except Exception as exc:
    failures.append(f"SQLite migration/seed/rollback execution failed: {exc}")

# E2E contracts.
harness = text("tests/end-to-end/test_harness.rs")
check("testcontainers_modules" in harness and "Postgres::default().start()" in harness,
      "Testcontainers PostgreSQL harness missing")
check('include_str!("../fixtures/seed.sql")' in harness,
      "E2E harness must consume seed.sql")
check("pub database_url: String" in harness,
      "Testcontainers harness does not expose the production PostgreSQL URL")
approval_journey = text("tests/end-to-end/approval_workflow.rs")
check("ApiState::new(&postgres.database_url)" in approval_journey,
      "mandatory Approval journey does not exercise PostgreSQL API composition")
for journey in range(1, 5):
    check(any(f"journey_{journey}" in p.read_text(encoding="utf-8")
              for p in (ROOT / "tests/end-to-end").glob("*.rs")),
          f"mandatory journey {journey} absent")
for name in ["notification_sync.rs", "p2p_sync.rs", "background_sync.rs"]:
    check("#[ignore" in text(f"tests/end-to-end/{name}"),
          f"client journey not ignored: {name}")

# Chaos and load contracts.
chaos = text("tests/chaos/chaos_runtime.rs")
check("15 * 60" in chaos, "chaos duration is not 15 minutes")
check("ONYX_CHAOS_REALTIME" in chaos, "real-time chaos mode absent")
check((ROOT / "tests/chaos/database_failover.rs").is_file(), "fifth chaos scenario absent")
for scenario in ["network_partition", "process_crash", "disk_full", "clock_skew", "database_failover"]:
    check((ROOT / f"tests/chaos/{scenario}.rs").is_file(), f"chaos scenario absent: {scenario}")
load = text("tests/load/smoke-test.js")
for token in ["vus: 100", "duration: '60s'", "p(95)<500"]:
    check(token in load, f"load contract missing: {token}")

# Release security and integrity controls.
release_workflow = text(".github/workflows/release.yml")
release_script = text("scripts/release.sh")
release = release_workflow + release_script
for token in ["cosign", "gpg", "spdx-json@2.3", "actions/attest@v4"]:
    check(token.lower() in release.lower(), f"release security control missing: {token}")
check("sigstore/cosign-installer@v4" in release_workflow, "current Cosign installer major not used")
check("crazy-max/ghaction-import-gpg@v7" in release_workflow, "current GPG action major not used")
check('"${IMAGE}@${DIGEST}"' in release_workflow,
      "container signature is not bound to immutable digest")
check("subject-digest: ${{ steps.build.outputs.digest }}" in release_workflow,
      "image attestation is not bound to build digest")
check("sbom-path:" in release_workflow, "SBOM attestation missing")
check("find \"release/${VERSION}\" -type f" in release_script,
      "release checksums do not safely enumerate files")
check("cargo metadata --locked --no-deps" in release_workflow
      and "cargo generate-lockfile" in release_script,
      "release workflow lockfile validation or release-script regeneration missing")

# Runbook and formal signoff contracts.
runbooks = "\n".join(p.read_text(encoding="utf-8") for p in (ROOT / "docs/runbooks").glob("*.md"))
check(re.search(r"RTO.{0,40}(one|1) hour", runbooks, re.I | re.S) is not None,
      "RTO target absent from runbooks")
check(re.search(r"RPO.{0,40}(five|5) minutes", runbooks, re.I | re.S) is not None,
      "RPO target absent from runbooks")
check("NO-GO" in text("docs/release/go-live-checklist.md"), "go-live checklist lacks NO-GO guard")

# Decision record.
decisions = text("docs/DECISIONS.md")
for ruling in [f"R{i}" for i in range(1, 12)]:
    check(ruling in decisions, f"Team 8 ruling not recorded: {ruling}")
for decision in [f"T8-D{i}" for i in range(1, 18)]:
    check(decision in decisions, f"Team 8 additional ruling absent: {decision}")

# OpenAPI remains parseable after Team 8 integration.
try:
    json.loads(text("docs/api/openapi.json"))
except Exception as exc:
    failures.append(f"OpenAPI JSON invalid: {exc}")

# Parse workflow and values YAML where no Helm template syntax exists.
if yaml is not None:
    for path in sorted((ROOT / ".github/workflows").glob("*.yml")):
        try:
            parsed = yaml.safe_load(path.read_text(encoding="utf-8"))
            check(isinstance(parsed, dict), f"workflow does not parse as mapping: {path}")
        except Exception as exc:
            failures.append(f"workflow YAML invalid ({path.relative_to(ROOT)}): {exc}")
    for path in sorted((ROOT / "deploy/helm").rglob("Chart.yaml")) + sorted((ROOT / "deploy/helm").rglob("values*.yaml")):
        try:
            parsed = yaml.safe_load(path.read_text(encoding="utf-8"))
            check(isinstance(parsed, dict), f"Helm metadata/values not mapping: {path}")
        except Exception as exc:
            failures.append(f"Helm YAML invalid ({path.relative_to(ROOT)}): {exc}")
else:
    warnings.append("PyYAML unavailable; workflow/value parsing skipped")

# Bash and JavaScript syntax checks when local interpreters are present.
for path in sorted((ROOT / "scripts").rglob("*.sh")):
    result = subprocess.run(["bash", "-n", str(path)], capture_output=True, text=True)
    check(result.returncode == 0, f"bash syntax invalid: {path.relative_to(ROOT)}: {result.stderr.strip()}")
if subprocess.run(["bash", "-lc", "command -v node"], capture_output=True).returncode == 0:
    result = subprocess.run(["node", "--check", str(ROOT / "tests/load/smoke-test.js")], capture_output=True, text=True)
    check(result.returncode == 0, f"k6 JavaScript syntax invalid: {result.stderr.strip()}")

# Basic delimiter integrity for Team 8-authored Rust sources.
team8_rust = [
    ROOT / "crates/bins/migration-tool/src/main.rs",
    ROOT / "crates/bins/migration-tool/src/migrations.rs",
    ROOT / "crates/bins/sync-agent/src/main.rs",
    ROOT / "crates/infrastructure/security-adapter/src/rate_limiter.rs",
    ROOT / "crates/bins/api-server/src/main.rs",
    ROOT / "crates/bins/api-server/src/query_handler.rs",
    ROOT / "crates/bins/api-server/src/routes/mod.rs",
    ROOT / "crates/bins/api-server/src/routes/query.rs",
    ROOT / "crates/bins/api-server/src/routes/command.rs",
    *list((ROOT / "crates/team8-e2e-tests").rglob("*.rs")),
    *list((ROOT / "crates/team8-chaos-tests").rglob("*.rs")),
    *list((ROOT / "tests/end-to-end").rglob("*.rs")),
    *list((ROOT / "tests/chaos").rglob("*.rs")),
    ROOT / "tests/load/performance_benchmark.rs",
]
for path in team8_rust:
    body = path.read_text(encoding="utf-8")
    check(body.count("{") == body.count("}"), f"unbalanced Rust braces: {path.relative_to(ROOT)}")
    check(body.count("(") == body.count(")"), f"unbalanced Rust parentheses: {path.relative_to(ROOT)}")
    check(body.count("[") == body.count("]"), f"unbalanced Rust brackets: {path.relative_to(ROOT)}")

# Expected environment limitations are reported rather than hidden.
lock = text("Cargo.lock")
warn('name = "testcontainers-modules"' in lock and 'name = "opentelemetry-otlp"' in lock,
     "Cargo.lock predates Team 7/8 dependencies; regenerate and commit before sign-off")
warn((ROOT / "web-ui/package-lock.json").is_file(),
     "web-ui/package-lock.json is absent; generate and commit before sign-off")

report = {
    "checks": checks,
    "passed": checks - len(failures),
    "failures": failures,
    "warnings": warnings,
}
if args.report:
    Path(args.report).write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

print(f"Team 8 static verification: {report['passed']}/{checks} checks passed")
for warning in warnings:
    print(f"WARN: {warning}", file=sys.stderr)
if failures:
    for failure in failures:
        print(f"FAIL: {failure}", file=sys.stderr)
    raise SystemExit(1)
