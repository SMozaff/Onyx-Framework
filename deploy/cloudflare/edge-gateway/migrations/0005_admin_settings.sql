-- Organization-scoped mobile access grants and optimistic concurrency state.
-- Additive migration: preserves all existing settings and tenant data.
CREATE TABLE IF NOT EXISTS mobile_access_settings (
  organization_id TEXT PRIMARY KEY,
  allowed_classes TEXT NOT NULL CHECK (json_valid(allowed_classes)),
  version INTEGER NOT NULL CHECK (version > 0),
  updated_by TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  last_operation_id TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_mobile_access_settings_updated_at
  ON mobile_access_settings (updated_at);
