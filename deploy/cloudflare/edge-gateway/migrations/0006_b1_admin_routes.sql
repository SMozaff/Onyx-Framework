-- B1 admin-route parity: manager compatibility and per-user session invalidation.
ALTER TABLE users ADD COLUMN is_manager INTEGER NOT NULL DEFAULT 0;
CREATE INDEX IF NOT EXISTS idx_users_org_manager ON users (organization_id, is_manager, is_active);

CREATE TABLE IF NOT EXISTS user_session_revocations (
  user_id TEXT PRIMARY KEY,
  revoked_before_ms INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
