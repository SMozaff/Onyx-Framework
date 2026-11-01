-- Staff accounts (backend accounts lane; decisions O-3 and O-4).
-- onyx_id: the staff ID (public_ids.public_id) of the account. Login identifier (O-3).
--   Unique per organization. Stored lowercase.
-- must_change_password: set on every account created by import or single creation.
--   requireSession allows only POST /api/auth/password (and logout) while it is set.
--   Cleared by a successful password change (O-4).
ALTER TABLE users ADD COLUMN onyx_id TEXT CHECK (onyx_id IS NULL OR onyx_id = LOWER(onyx_id));
ALTER TABLE users ADD COLUMN must_change_password INTEGER NOT NULL DEFAULT 0;
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_org_onyx_id ON users (organization_id, onyx_id) WHERE onyx_id IS NOT NULL;
