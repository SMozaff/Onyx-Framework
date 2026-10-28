-- Offline outbox ingest (offline-outbox-cloud-relay, step 5).
-- One row per device outbox row. The key (replica_id, outbox_id) makes retries
-- idempotent: a row uploaded twice is stored once. The replica must be owned by
-- the authenticated user and organization (replica_ownership), checked in the Worker.
CREATE TABLE IF NOT EXISTS outbox_ingest (
  replica_id TEXT NOT NULL,
  outbox_id INTEGER NOT NULL,
  organization_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  event_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  aggregate_id TEXT NOT NULL,
  occurred_at_millis INTEGER NOT NULL,
  payload TEXT NOT NULL,
  received_at INTEGER NOT NULL,
  PRIMARY KEY (replica_id, outbox_id)
);
CREATE INDEX IF NOT EXISTS idx_outbox_ingest_org
  ON outbox_ingest (organization_id, received_at);
