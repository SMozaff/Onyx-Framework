-- ONYX security model: Supabase/Google is exclusively for All-Father.
-- Admin and Staff credentials, roles, status, and organization membership live in D1.
ALTER TABLE users ADD COLUMN email TEXT;
ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'STAFF';
ALTER TABLE users ADD COLUMN supabase_user_id TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email_lower
    ON users (LOWER(email)) WHERE email IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_supabase_user
    ON users (supabase_user_id) WHERE supabase_user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_users_org_role_active
    ON users (organization_id, role, is_active);

CREATE TABLE IF NOT EXISTS organizations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Stable root scope for the single All-Father principal. The password hash is
-- intentionally unusable; this principal is accepted only through Supabase.
INSERT OR IGNORE INTO organizations(id, name, is_active, created_at, updated_at)
VALUES ('__onyx_root__', 'ONYX Root Authority', 1, 0, 0);

INSERT OR IGNORE INTO users(
    id, username, email, supabase_user_id, organization_id, password_hash,
    is_admin, is_active, class, role, parent_user_id, created_at, updated_at
) VALUES (
    '__onyx_allfather__', 'allfather', 'so.muzaff@gmail.com', NULL,
    '__onyx_root__',
    '$argon2id$v=19$m=65536,t=3,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
    1, 1, 'ALL_FATHER', 'ALL_FATHER', NULL, 0, 0
);
