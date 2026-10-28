-- Offline outbox -> cloud relay acknowledgement (BOUND: offline-outbox-cloud-relay).
-- bus_delivered: 1 once the row has been published to the local event bus, so
--   retries after a failed cloud upload do not publish the UI event again.
-- cloud_acked_at: Unix milliseconds when the Worker confirmed storage.
--   A row is only "published" (done) after this is set.
ALTER TABLE outbox ADD COLUMN bus_delivered INTEGER NOT NULL DEFAULT 0;
ALTER TABLE outbox ADD COLUMN cloud_acked_at INTEGER;
