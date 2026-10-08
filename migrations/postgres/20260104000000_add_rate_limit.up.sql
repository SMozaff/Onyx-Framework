-- ONYX Increment 7: exact sliding-window rate-limit ledger.
CREATE TABLE rate_limit_events (
    request_id TEXT PRIMARY KEY,
    organization_id UUID NOT NULL,
    resource_class TEXT NOT NULL CHECK (resource_class IN ('commands', 'sync_operations', 'automation_executions')),
    occurred_at_ms BIGINT NOT NULL
);
CREATE INDEX idx_rate_limit_window ON rate_limit_events (organization_id, resource_class, occurred_at_ms);
