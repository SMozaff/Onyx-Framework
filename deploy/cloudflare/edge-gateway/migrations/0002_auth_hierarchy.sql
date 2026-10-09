-- ONYX authentication hierarchy.
-- All-Father authenticates through Clerk; Admin/Staff authenticate with ONYX passwords.

CREATE TABLE IF NOT EXISTS organizations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_organizations_name_lower ON organizations (LOWER(name));

ALTER TABLE users ADD COLUMN email TEXT;
ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'STAFF';
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email_lower ON users (LOWER(email)) WHERE email IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_users_role ON users (role);

INSERT OR IGNORE INTO organizations (id, name, is_active, created_at, updated_at)
VALUES ('__allfather__', 'ONYX System Authority', 1, unixepoch(), unixepoch());

-- No ONYX password is assigned to All-Father. The placeholder hash is intentionally
-- non-usable and exists only because the legacy users.password_hash column is NOT NULL.
INSERT OR IGNORE INTO users (
    id, username, email, organization_id, password_hash, is_admin, is_active, class, role,
    parent_user_id, created_at, updated_at
) VALUES (
    '__allfather__', 'allfather', 'so.muzaff@gmail.com', '__allfather__',
    '$argon2id$v=19$m=65536,t=3,p=4$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
    1, 1, 'ALL_FATHER', 'ALL_FATHER', NULL, unixepoch(), unixepoch()
);
