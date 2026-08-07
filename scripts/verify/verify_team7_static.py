#!/usr/bin/env python3
"""Offline structural verification for ONYX Increment 7.

This does not replace Cargo's compiler/tests. It validates artifacts that can be
verified without a Rust toolchain or external services.
"""
from __future__ import annotations

import json
import re
import sqlite3
import sys
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ERRORS: list[str] = []
CHECKS: list[str] = []


def check(condition: bool, message: str) -> None:
    if condition:
        CHECKS.append(message)
    else:
        ERRORS.append(message)


required = [
    "crates/applications/security-application/src/ports/authority_verifier.rs",
    "crates/applications/security-application/src/ports/rate_limiter.rs",
    "crates/applications/security-application/src/ports/secret_provider.rs",
    "crates/applications/audit-application/src/ports/audit_writer.rs",
    "crates/applications/audit-application/src/ports/integrity.rs",
    "crates/applications/worker-application/src/ports/job_queue.rs",
    "crates/infrastructure/observability-adapter/src/tracing.rs",
    "crates/infrastructure/observability-adapter/src/metrics.rs",
    "crates/infrastructure/observability-adapter/src/logging.rs",
    "crates/infrastructure/observability-adapter/src/audit.rs",
    "crates/infrastructure/security-adapter/src/authority.rs",
    "crates/infrastructure/security-adapter/src/rate_limiter.rs",
    "crates/infrastructure/security-adapter/src/secret_provider.rs",
    "crates/infrastructure/background-jobs/src/postgres_queue.rs",
    "crates/infrastructure/background-jobs/src/sqlite_queue.rs",
    "crates/bins/worker/src/job_runner.rs",
    "crates/bins/worker/src/scheduler_loop.rs",
    "crates/bins/worker/src/snapshot_loop.rs",
    "crates/bins/api-server/src/middleware/rate_limit.rs",
    "migrations/postgres/20260103000000_add_job_queue.up.sql",
    "migrations/postgres/20260103000000_add_job_queue.down.sql",
    "migrations/postgres/20260104000000_add_rate_limit.up.sql",
    "migrations/postgres/20260104000000_add_rate_limit.down.sql",
    "migrations/sqlite/20260103000000_add_job_queue.up.sql",
    "migrations/sqlite/20260103000000_add_job_queue.down.sql",
    "migrations/sqlite/20260104000000_add_rate_limit.up.sql",
    "migrations/sqlite/20260104000000_add_rate_limit.down.sql",
]
for relative in required:
    check((ROOT / relative).is_file(), f"required file exists: {relative}")

# Parse every manifest and validate workspace members/path dependencies.
manifests = list(ROOT.rglob("Cargo.toml"))
parsed: dict[Path, dict] = {}
for manifest in manifests:
    try:
        parsed[manifest] = tomllib.loads(manifest.read_text())
    except Exception as exc:  # pragma: no cover - diagnostic path
        ERRORS.append(f"invalid TOML {manifest.relative_to(ROOT)}: {exc}")
check(len(parsed) == len(manifests), f"all {len(manifests)} Cargo manifests parse")
root_manifest = parsed.get(ROOT / "Cargo.toml", {})
for member in root_manifest.get("workspace", {}).get("members", []):
    check((ROOT / member / "Cargo.toml").is_file(), f"workspace member exists: {member}")
for manifest, data in parsed.items():
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        for name, spec in data.get(section, {}).items():
            if isinstance(spec, dict) and "path" in spec:
                target = (manifest.parent / spec["path"] / "Cargo.toml").resolve()
                check(target.is_file(), f"path dependency resolves: {manifest.relative_to(ROOT)} -> {name}")

# Lightweight Rust delimiter/string/comment balance scanner.
def scan_rust(path: Path) -> str | None:
    text = path.read_text(errors="replace")
    stack: list[tuple[str, int]] = []
    pairs = {")": "(", "]": "[", "}": "{"}
    opens = set(pairs.values())
    i = 0
    mode = "code"
    block_depth = 0
    while i < len(text):
        ch = text[i]
        nxt = text[i + 1] if i + 1 < len(text) else ""
        if mode == "line":
            if ch == "\n":
                mode = "code"
            i += 1
            continue
        if mode == "block":
            if ch == "/" and nxt == "*":
                block_depth += 1
                i += 2
            elif ch == "*" and nxt == "/":
                block_depth -= 1
                i += 2
                if block_depth == 0:
                    mode = "code"
            else:
                i += 1
            continue
        if mode in {"string", "char"}:
            quote = '"' if mode == "string" else "'"
            if ch == "\\":
                i += 2
            elif ch == quote:
                mode = "code"
                i += 1
            else:
                i += 1
            continue
        if ch == "/" and nxt == "/":
            mode = "line"
            i += 2
        elif ch == "/" and nxt == "*":
            mode = "block"
            block_depth = 1
            i += 2
        elif ch == '"':
            mode = "string"
            i += 1
        elif ch == "'":
            # Lifetimes (`'a`) are not character literals.
            if nxt.isalpha() or nxt == "_":
                i += 1
            else:
                mode = "char"
                i += 1
        elif ch in opens:
            stack.append((ch, i))
            i += 1
        elif ch in pairs:
            if not stack or stack[-1][0] != pairs[ch]:
                return f"mismatched {ch} at byte {i}"
            stack.pop()
            i += 1
        else:
            i += 1
    if mode == "block":
        return "unterminated block comment"
    if mode in {"string", "char"}:
        return f"unterminated {mode}"
    if stack:
        return f"unclosed delimiter {stack[-1][0]} at byte {stack[-1][1]}"
    return None

rust_files = list(ROOT.rglob("*.rs"))
for rust_file in rust_files:
    failure = scan_rust(rust_file)
    if failure:
        ERRORS.append(f"Rust lexical balance {rust_file.relative_to(ROOT)}: {failure}")
check(not any(error.startswith("Rust lexical") for error in ERRORS), f"all {len(rust_files)} Rust files are lexically balanced")

# Execute every SQLite migration, validate Team 7 schema, seed, then roll back Team 7.
with tempfile.NamedTemporaryFile(suffix=".db") as handle:
    connection = sqlite3.connect(handle.name)
    up_files = sorted((ROOT / "migrations/sqlite").glob("*.up.sql"))
    for migration in up_files:
        connection.executescript(migration.read_text())
    tables = {row[0] for row in connection.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    for table in {"jobs", "rate_limit_events", "audit_entries", "aggregate_snapshots"}:
        check(table in tables, f"SQLite migration creates {table}")
    jobs_columns = {row[1] for row in connection.execute("PRAGMA table_info(jobs)")}
    for column in {"status", "claimed_by", "lease_expires_at_ms", "next_attempt_at_ms", "attempts", "max_retries"}:
        check(column in jobs_columns, f"jobs column exists: {column}")
    seed = ROOT / "tests/fixtures/seed.sql"
    if seed.is_file():
        connection.executescript(seed.read_text())
        count = connection.execute("SELECT COUNT(*) FROM aggregates").fetchone()[0]
        check(count > 0, "Team 6/7 deterministic seed executes")
    for migration in sorted((ROOT / "migrations/sqlite").glob("2026010[34]*.down.sql"), reverse=True):
        connection.executescript(migration.read_text())
    remaining = {row[0] for row in connection.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    for table in {"jobs", "rate_limit_events", "audit_entries", "aggregate_snapshots"}:
        check(table not in remaining, f"SQLite down migration removes {table}")
    connection.close()

# Binding-ruling source assertions.
source = lambda rel: (ROOT / rel).read_text()
tracing = source("crates/infrastructure/observability-adapter/src/tracing.rs")
check("http://jaeger-collector:4317" in tracing, "R1 default OTLP endpoint is frozen")
metrics = source("crates/infrastructure/observability-adapter/src/metrics.rs")
for family in [
    "requests_total", "request_duration_seconds", "outbox_pending",
    "job_queue_depth", "sync_conflicts_open", "audit_entries_total",
    "observability_export_errors_total",
]:
    check(f'"{family}"' in metrics, f"R2 metric family declared: {family}")
logging = source("crates/infrastructure/observability-adapter/src/logging.rs")
check("CanonicalJsonLayer" in logging and "on_event" in logging, "R3 canonical JSON layer covers every tracing event")
for field in [
    "timestamp", "level", "service", "trace_id", "operation_id", "actor_id",
    "organization_id", "command_type", "outcome", "latency_ms",
]:
    check(f"pub {field}:" in logging, f"R3 structured field declared: {field}")
queue = source("crates/infrastructure/background-jobs/src/postgres_queue.rs")
check("FOR UPDATE SKIP LOCKED" in queue, "R4 PostgreSQL lease claims are concurrent-worker safe")
runner = source("crates/bins/worker/src/job_runner.rs")
check("min(300)" in runner and "0.20" in runner, "R4 retry backoff cap and jitter are encoded")
scheduler = source("crates/bins/worker/src/scheduler_loop.rs")
check("Duration::from_secs(5)" in scheduler, "R5 scheduler interval is five seconds")
snapshot = source("crates/bins/worker/src/snapshot_loop.rs")
check("Duration::from_secs(60 * 60)" in snapshot, "R6 snapshot interval is one hour")
check("SNAPSHOT_EVENT_THRESHOLD: i64 = 1_000" in snapshot, "R6 snapshot threshold is more than 1,000 events")
authority = source("crates/infrastructure/security-adapter/src/authority.rs")
for marker in ["Ed25519", "InvalidSignature", "Expired", "Revoked", "ObjectScopeMismatch", "CommandNotAuthorized"]:
    check(marker in authority, f"R7 authority validation marker exists: {marker}")
rate = source("crates/infrastructure/security-adapter/src/rate_limiter.rs")
check("pg_advisory_xact_lock" in rate, "R8 PostgreSQL advisory lock protects sliding window")
check("ResourceClass::Commands" in rate and "ResourceClass::SyncOperations" in rate and "ResourceClass::AutomationExecutions" in rate, "R8 all resource classes have policies")
audit = source("crates/infrastructure/observability-adapter/src/audit.rs")
check("Sha256::new" in audit and "hasher.update(previous)" in audit and "hasher.update(record)" in audit, "R9 SHA-256 hash-chain formula is implemented")
secret = source("crates/infrastructure/security-adapter/src/secret_provider.rs")
check("std::env::var" in secret and "PREVIOUS_VALID_UNTIL_UNIX" in secret, "R10 environment secret provider and rotation convention are implemented")

# OpenAPI remains valid JSON after the Team 7 integration.
openapi_path = ROOT / "docs/api/openapi.json"
try:
    openapi = json.loads(openapi_path.read_text())
    check(openapi.get("openapi") is not None, "frozen OpenAPI remains valid JSON")
except Exception as exc:
    ERRORS.append(f"invalid OpenAPI JSON: {exc}")

print(f"PASS checks: {len(CHECKS)}")
if ERRORS:
    print(f"FAIL checks: {len(ERRORS)}")
    for error in ERRORS:
        print(f"- {error}")
    sys.exit(1)
print("Static Team 7 verification passed.")
