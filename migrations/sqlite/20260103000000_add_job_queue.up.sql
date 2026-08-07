-- ONYX Increment 7: SQLite-compatible local/native job, audit and snapshot tables.
CREATE TABLE jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    organization_id BLOB NOT NULL,
    job_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'dead_letter')),
    claimed_by TEXT,
    lease_token TEXT,
    lease_expires_at_ms INTEGER,
    attempts INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 10,
    last_error TEXT,
    next_attempt_at_ms INTEGER NOT NULL,
    deduplication_key TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_jobs_runnable ON jobs (status, next_attempt_at_ms, id);
CREATE INDEX idx_jobs_lease ON jobs (status, lease_expires_at_ms);
CREATE INDEX idx_jobs_organization ON jobs (organization_id, status);
CREATE UNIQUE INDEX idx_jobs_deduplication ON jobs (deduplication_key) WHERE deduplication_key IS NOT NULL;

CREATE TABLE audit_entries (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    organization_id BLOB NOT NULL,
    previous_hash BLOB NOT NULL,
    current_hash BLOB NOT NULL,
    record TEXT NOT NULL,
    occurred_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_audit_entries_org_sequence ON audit_entries (organization_id, sequence);

CREATE TABLE aggregate_snapshots (
    snapshot_id INTEGER PRIMARY KEY AUTOINCREMENT,
    aggregate_id BLOB NOT NULL,
    organization_id BLOB NOT NULL,
    aggregate_type TEXT NOT NULL,
    aggregate_version INTEGER NOT NULL,
    event_count INTEGER NOT NULL,
    state TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    UNIQUE (aggregate_id, aggregate_version)
);
CREATE INDEX idx_aggregate_snapshots_latest ON aggregate_snapshots (aggregate_id, aggregate_version DESC);
