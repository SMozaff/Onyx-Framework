-- Explicit per-user file grants for Staff downloads. Legacy assets remain inaccessible
-- to Staff until an Admin grants access; no implicit tenant-wide access is added.
CREATE TABLE IF NOT EXISTS file_asset_acl (
  content_hash TEXT NOT NULL,
  organization_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  granted_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (content_hash, user_id)
);
CREATE INDEX IF NOT EXISTS idx_file_asset_acl_user_org
  ON file_asset_acl (organization_id, user_id, content_hash);
