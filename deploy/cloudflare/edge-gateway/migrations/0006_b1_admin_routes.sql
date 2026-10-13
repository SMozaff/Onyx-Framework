-- B1 admin-route parity: manager compatibility and per-user session invalidation.
ALTER TABLE users ADD COLUMN is_manager INTEGER NOT NULL DEFAULT 0;
CREATE INDEX IF NOT EXISTS idx_users_org_manager ON users (organization_id, is_manager, is_active);

CREATE TABLE IF NOT EXISTS user_session_revocations (
  user_id TEXT PRIMARY KEY,
  revoked_before_ms INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

-- Durable relay ticket redemption and first-claim replica ownership.
CREATE TABLE IF NOT EXISTS replica_ownership (
  replica_id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  organization_id TEXT NOT NULL,
  claimed_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_replica_ownership_org ON replica_ownership (organization_id, replica_id);

CREATE TABLE IF NOT EXISTS relay_ticket_redemptions (
  jti TEXT PRIMARY KEY,
  expires_at INTEGER NOT NULL,
  redeemed_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_relay_ticket_expiry ON relay_ticket_redemptions (expires_at);
