-- ONYX Cloudflare D1 foundation.
--
-- D1 is SQLite-compatible. This table mirrors the existing ONYX SQLite users
-- migration exactly enough to preserve the identity storage contract while
-- the remaining projections are ported route-by-route.
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL,
    organization_id TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    is_admin INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username_lower
    ON users (LOWER(username));

CREATE INDEX IF NOT EXISTS idx_users_organization
    ON users (organization_id);
