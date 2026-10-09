-- ONYX Cloudflare D1 foundation.
-- D1 is SQLite-compatible. The worker keeps the ONYX aggregate/event shape
-- but stores identifiers as TEXT so the JavaScript runtime does not need
-- binary UUID adapters. Domain payloads remain JSON strings.

CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL,
    organization_id TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    is_admin INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 1,
    class TEXT NULL,
    parent_user_id TEXT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username_lower ON users (LOWER(username));
CREATE INDEX IF NOT EXISTS idx_users_organization ON users (organization_id);
CREATE INDEX IF NOT EXISTS idx_users_parent ON users (parent_user_id);

CREATE TABLE IF NOT EXISTS aggregates (
    id TEXT PRIMARY KEY,
    aggregate_type TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 0,
    lifecycle_epoch INTEGER NOT NULL DEFAULT 0,
    authority_epoch INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    organization_id TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_aggregates_org_type ON aggregates (organization_id, aggregate_type);
CREATE INDEX IF NOT EXISTS idx_aggregates_org_updated ON aggregates (organization_id, updated_at);

CREATE TABLE IF NOT EXISTS domain_events (
    event_id TEXT PRIMARY KEY,
    aggregate_id TEXT NOT NULL,
    aggregate_version INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    occurred_at INTEGER NOT NULL,
    vector_clock TEXT NOT NULL DEFAULT '{}',
    operation_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    causation_id TEXT,
    actor TEXT NOT NULL DEFAULT '{}',
    organization_id TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_org_time ON domain_events (organization_id, occurred_at);
CREATE INDEX IF NOT EXISTS idx_events_aggregate ON domain_events (aggregate_id, aggregate_version);

CREATE TABLE IF NOT EXISTS idempotency (
    operation_id TEXT PRIMARY KEY,
    result TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS token_revocations (
    token_hash TEXT PRIMARY KEY,
    revoked_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS push_subscriptions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    organization_id TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL,
    platform TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_push_owner_endpoint
    ON push_subscriptions (user_id, organization_id, endpoint);

CREATE TABLE IF NOT EXISTS file_assets (
    content_hash TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    blob_key TEXT NOT NULL,
    content_type TEXT,
    byte_size INTEGER,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_file_assets_org ON file_assets (organization_id);

CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    organization_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    action TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    details TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_audit_org_time ON audit_log (organization_id, created_at);
