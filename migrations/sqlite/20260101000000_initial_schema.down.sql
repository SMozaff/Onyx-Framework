-- Down migration for SQLite 20260101000000_initial_schema.
-- Not specified by Team Prompt 2; added so `sqlx migrate` has a reversible
-- pair, per standard sqlx practice (same rationale as the Postgres down
-- migration). Documented in DECISIONS.md.

DROP INDEX IF EXISTS idx_dead_letter_organization;
DROP TABLE IF EXISTS dead_letter;

DROP INDEX IF EXISTS idx_inbox_consumer;
DROP TABLE IF EXISTS inbox;

DROP TABLE IF EXISTS idempotency;

DROP INDEX IF EXISTS idx_outbox_event_id;
DROP INDEX IF EXISTS idx_outbox_organization;
DROP INDEX IF EXISTS idx_outbox_published;
DROP TABLE IF EXISTS outbox;

DROP INDEX IF EXISTS idx_domain_events_organization;
DROP INDEX IF EXISTS idx_domain_events_operation_id;
DROP INDEX IF EXISTS idx_domain_events_aggregate_id;
DROP TABLE IF EXISTS domain_events;

DROP INDEX IF EXISTS idx_aggregates_version;
DROP INDEX IF EXISTS idx_aggregates_organization_type;
DROP TABLE IF EXISTS aggregates;
