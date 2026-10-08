-- ======================================================================
-- ONYX Persistence Schema — SQLite (local/desktop/mobile)
-- Source: Team Prompt 2 §4.2 (aggregates, outbox verbatim) + Architectural
-- Ruling "SQLite Schema Mapping" (S1) for the derived remainder
-- (domain_events, idempotency, inbox, dead_letter).
--
-- Type mapping (ruling S1): UUID -> BLOB(16), JSONB -> TEXT, TIMESTAMPTZ ->
-- INTEGER (Unix MILLISECONDS — see note below), BIGSERIAL/BIGINT ->
-- INTEGER PRIMARY KEY AUTOINCREMENT, BOOLEAN -> INTEGER (0/1).
--
-- NOTE ON TIMESTAMP UNITS: ruling S1 standardizes on Unix milliseconds for
-- BOTH Postgres and SQLite, for representational consistency. This differs
-- from platform_kernel::Timestamp, which is Unix NANOSECONDS. Adapter code
-- MUST convert nanoseconds -> milliseconds when writing a Timestamp into
-- any *_at column, and milliseconds -> nanoseconds when reading one back
-- out. See DECISIONS.md item S2 for the conversion contract and precision-
-- loss disclosure (sub-millisecond precision is not preserved in storage).
-- ======================================================================

-- 1. AGGREGATES TABLE
CREATE TABLE aggregates (
    id BLOB PRIMARY KEY,                  -- UUID (16 bytes)
    aggregate_type TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 0,
    lifecycle_epoch INTEGER NOT NULL DEFAULT 0,
    authority_epoch INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL,                  -- JSON
    updated_at INTEGER NOT NULL,          -- Unix milliseconds
    organization_id BLOB NOT NULL
);

CREATE INDEX idx_aggregates_organization_type ON aggregates (organization_id, aggregate_type);
CREATE INDEX idx_aggregates_version ON aggregates (version);

-- 2. DOMAIN EVENTS TABLE
CREATE TABLE domain_events (
    event_id BLOB PRIMARY KEY,
    aggregate_id BLOB NOT NULL,
    aggregate_version INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,                -- JSON
    occurred_at INTEGER NOT NULL,         -- Unix milliseconds
    vector_clock TEXT NOT NULL,           -- JSON
    operation_id BLOB NOT NULL,
    correlation_id BLOB NOT NULL,
    causation_id BLOB,
    actor TEXT NOT NULL,                  -- JSON
    organization_id BLOB NOT NULL
);

CREATE INDEX idx_domain_events_aggregate_id ON domain_events (aggregate_id, aggregate_version);
CREATE INDEX idx_domain_events_operation_id ON domain_events (operation_id);
CREATE INDEX idx_domain_events_organization ON domain_events (organization_id);

-- 3. OUTBOX TABLE
CREATE TABLE outbox (
    outbox_id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id BLOB NOT NULL,
    event_type TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    organization_id BLOB NOT NULL,
    payload TEXT NOT NULL,                -- JSON
    vector_clock TEXT NOT NULL,           -- JSON
    occurred_at INTEGER NOT NULL,         -- Unix milliseconds
    published INTEGER NOT NULL DEFAULT 0, -- BOOLEAN
    lease_token BLOB,
    lease_expires_at INTEGER,
    retry_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    next_attempt_at INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT 0 -- Unix milliseconds
);

CREATE INDEX idx_outbox_published ON outbox (published, next_attempt_at) WHERE published = 0;
CREATE INDEX idx_outbox_organization ON outbox (organization_id);
CREATE INDEX idx_outbox_event_id ON outbox (event_id);

-- 4. IDEMPOTENCY TABLE
CREATE TABLE idempotency (
    operation_id BLOB PRIMARY KEY,
    result TEXT NOT NULL,                 -- JSON
    created_at INTEGER NOT NULL DEFAULT 0 -- Unix milliseconds
);

-- 5. INBOX TABLE
CREATE TABLE inbox (
    consumer_id TEXT NOT NULL,
    event_id BLOB NOT NULL,
    processed_at INTEGER NOT NULL DEFAULT 0, -- Unix milliseconds
    PRIMARY KEY (consumer_id, event_id)
);

CREATE INDEX idx_inbox_consumer ON inbox (consumer_id, processed_at);

-- 6. DEAD-LETTER TABLE
CREATE TABLE dead_letter (
    dead_letter_id INTEGER PRIMARY KEY AUTOINCREMENT,
    outbox_id INTEGER NOT NULL,
    event_id BLOB NOT NULL,
    payload TEXT NOT NULL,                -- JSON
    last_error TEXT NOT NULL,
    failed_at INTEGER NOT NULL DEFAULT 0, -- Unix milliseconds
    organization_id BLOB NOT NULL
);

CREATE INDEX idx_dead_letter_organization ON dead_letter (organization_id);
