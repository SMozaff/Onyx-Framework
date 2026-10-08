-- MIGRATION_PLAN Phase 1.2: Web Push subscription registration for the
-- PWA ObserverClient. Endpoint data only -- no push delivery backend is
-- built yet; this table is what a future push-delivery worker would read
-- to fan out notifications. Ownership is (user, organization) so a
-- subscription can only be listed/deleted by its own session.
CREATE TABLE push_subscriptions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    organization_id UUID NOT NULL,
    endpoint TEXT NOT NULL,
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL,
    platform TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX idx_push_subscriptions_owner_endpoint
    ON push_subscriptions (user_id, organization_id, endpoint);