-- MIGRATION_PLAN Phase 1.2 / PWA push delivery: delivery ledger for the
-- push-delivery worker. The worker reads `push_subscriptions` (registered by
-- api-server) and forwards unacknowledged `notification` aggregates to the
-- matching push endpoint. Each successful send is recorded here so a
-- notification is only delivered once per subscription; the PK doubles as the
-- idempotency/dedup key. `push_deliveries` also flips worker-side state from
-- "attempt" to "delivered" (the notification aggregate's own status remains
-- authoritative for the UI's unread badge).
CREATE TABLE push_deliveries (
    subscription_id UUID NOT NULL,
    notification_id UUID NOT NULL,
    delivered_at BIGINT NOT NULL,
    PRIMARY KEY (subscription_id, notification_id)
);
CREATE INDEX idx_push_deliveries_delivered_at
    ON push_deliveries (subscription_id, delivered_at);