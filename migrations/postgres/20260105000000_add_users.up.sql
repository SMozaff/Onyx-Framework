-- ONYX production-readiness audit, finding H-01.
-- Replaces the compile-time DEFAULT_USERNAME/DEFAULT_PASSWORD constants in
-- api-server with a real identity store.
--
-- password_hash holds a PHC-format Argon2id string (e.g. "$argon2id$v=19$...").
-- It is deliberately TEXT and not a fixed-width digest column: the KDF
-- parameters are embedded in the value so they can be raised over time
-- without a schema change or a forced credential reset.
CREATE TABLE users (
    id UUID PRIMARY KEY,
    username TEXT NOT NULL,
    organization_id UUID NOT NULL,
    password_hash TEXT NOT NULL,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Usernames are globally unique and case-insensitive. Without LOWER(), an
-- attacker could register "Operator" alongside "operator" and rely on humans
-- confusing the two. The store lowercases on write; this index enforces it
-- even if a future writer forgets.
CREATE UNIQUE INDEX idx_users_username_lower ON users (LOWER(username));

-- Supports admin listing scoped by tenant.
CREATE INDEX idx_users_organization ON users (organization_id);
