-- ======================================================================
-- ONYX Persistence Schema v1.0
-- Source: Handover Document §8, Team Prompt 2 §4.1
-- Copied verbatim per Team Prompt 2 §3 ("These definitions are copied
-- verbatim from the Handover Document. You MUST implement them exactly
-- as specified. No deviations are permitted.")
-- ======================================================================

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ----------------------------------------------------------------------
-- 1. AGGREGATES TABLE
-- Stores the current state (snapshot) of every aggregate.
-- Versioning and epochs enable optimistic concurrency control.
-- ----------------------------------------------------------------------
CREATE TABLE aggregates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    aggregate_type TEXT NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    lifecycle_epoch BIGINT NOT NULL DEFAULT 0,
    authority_epoch BIGINT NOT NULL DEFAULT 0,
    state JSONB NOT NULL,               -- Serialized aggregate
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    organization_id UUID NOT NULL
);

CREATE INDEX idx_aggregates_organization_type ON aggregates (organization_id, aggregate_type);
CREATE INDEX idx_aggregates_version ON aggregates (version);

-- ----------------------------------------------------------------------
-- 2. DOMAIN EVENTS TABLE
-- Append-only stream of all accepted events.
-- Used for replay, audit, and synchronization.
-- ----------------------------------------------------------------------
CREATE TABLE domain_events (
    event_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    aggregate_id UUID NOT NULL,
    aggregate_version BIGINT NOT NULL,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    vector_clock JSONB NOT NULL,
    operation_id UUID NOT NULL,
    correlation_id UUID NOT NULL,
    causation_id UUID,
    actor JSONB NOT NULL,
    organization_id UUID NOT NULL
);

CREATE INDEX idx_domain_events_aggregate_id ON domain_events (aggregate_id, aggregate_version);
CREATE INDEX idx_domain_events_operation_id ON domain_events (operation_id);
CREATE INDEX idx_domain_events_organization ON domain_events (organization_id);

-- ----------------------------------------------------------------------
-- 3. OUTBOX TABLE
-- Reliable message queue for integration events.
-- Never delete published rows immediately; retain per audit policy.
-- ----------------------------------------------------------------------
CREATE TABLE outbox (
    outbox_id BIGSERIAL PRIMARY KEY,
    event_id UUID NOT NULL,
    event_type TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    organization_id UUID NOT NULL,
    payload JSONB NOT NULL,             -- Serialized OutboxMessage
    vector_clock JSONB NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,

    published BOOLEAN NOT NULL DEFAULT FALSE,
    lease_token UUID,
    lease_expires_at TIMESTAMPTZ,
    retry_count INT NOT NULL DEFAULT 0,
    last_error TEXT,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_outbox_published ON outbox (published, next_attempt_at) WHERE published = FALSE;
CREATE INDEX idx_outbox_organization ON outbox (organization_id);
CREATE INDEX idx_outbox_event_id ON outbox (event_id);

-- ----------------------------------------------------------------------
-- 4. IDEMPOTENCY TABLE
-- Prevents duplicate command execution.
-- Stores the result of the first successful execution of an OperationId.
-- ----------------------------------------------------------------------
CREATE TABLE idempotency (
    operation_id UUID PRIMARY KEY,
    result JSONB NOT NULL,              -- Serialized CommandResult
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ----------------------------------------------------------------------
-- 5. INBOX TABLE
-- Consumer-side deduplication.
-- Prevents the same event from being applied twice to the same consumer.
-- ----------------------------------------------------------------------
CREATE TABLE inbox (
    consumer_id TEXT NOT NULL,
    event_id UUID NOT NULL,
    processed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (consumer_id, event_id)
);

CREATE INDEX idx_inbox_consumer ON inbox (consumer_id, processed_at);

-- ----------------------------------------------------------------------
-- 6. DEAD-LETTER TABLE
-- Messages that failed permanently after max retries.
-- Human review required.
-- ----------------------------------------------------------------------
CREATE TABLE dead_letter (
    dead_letter_id BIGSERIAL PRIMARY KEY,
    outbox_id BIGINT NOT NULL,
    event_id UUID NOT NULL,
    payload JSONB NOT NULL,
    last_error TEXT NOT NULL,
    failed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    organization_id UUID NOT NULL
);

CREATE INDEX idx_dead_letter_organization ON dead_letter (organization_id);
