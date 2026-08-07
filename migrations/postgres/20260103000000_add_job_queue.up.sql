-- ONYX Increment 7: durable jobs, audit hash chain, and aggregate snapshots.
CREATE TABLE jobs (
    id BIGSERIAL PRIMARY KEY,
    organization_id UUID NOT NULL,
    job_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'dead_letter')),
    claimed_by TEXT,
    lease_token TEXT,
    lease_expires_at_ms BIGINT,
    attempts INT NOT NULL DEFAULT 0,
    max_retries INT NOT NULL DEFAULT 10,
    last_error TEXT,
    next_attempt_at_ms BIGINT NOT NULL,
    deduplication_key TEXT,
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL
);
CREATE INDEX idx_jobs_runnable ON jobs (status, next_attempt_at_ms, id);
CREATE INDEX idx_jobs_lease ON jobs (status, lease_expires_at_ms);
CREATE INDEX idx_jobs_organization ON jobs (organization_id, status);
CREATE UNIQUE INDEX idx_jobs_deduplication ON jobs (deduplication_key) WHERE deduplication_key IS NOT NULL;

CREATE TABLE audit_entries (
    sequence BIGSERIAL PRIMARY KEY,
    organization_id UUID NOT NULL,
    previous_hash BYTEA NOT NULL,
    current_hash BYTEA NOT NULL,
    record JSONB NOT NULL,
    occurred_at_ms BIGINT NOT NULL
);
CREATE INDEX idx_audit_entries_org_sequence ON audit_entries (organization_id, sequence);

CREATE TABLE aggregate_snapshots (
    snapshot_id BIGSERIAL PRIMARY KEY,
    aggregate_id UUID NOT NULL,
    organization_id UUID NOT NULL,
    aggregate_type TEXT NOT NULL,
    aggregate_version BIGINT NOT NULL,
    event_count BIGINT NOT NULL,
    state JSONB NOT NULL,
    created_at_ms BIGINT NOT NULL,
    UNIQUE (aggregate_id, aggregate_version)
);
CREATE INDEX idx_aggregate_snapshots_latest ON aggregate_snapshots (aggregate_id, aggregate_version DESC);
