-- ONYX auth schema repair and contract baseline (forward-only).
--
-- Phase 3 (reports/auth-repair/03-d1-migrations.md). This migration contains
-- ONLY idempotent statements (IF NOT EXISTS / IF EXISTS / INSERT OR IGNORE),
-- so it converges every reachable ledger state to one canonical auth schema
-- and can never fail because an earlier migration already ran. It:
--   1. ensures the canonical auth indexes exist;
--   2. removes the redundant duplicate Supabase-mapping index left by the
--      pre-repair `0003_supabase_identity.sql` (index drop only, no data loss;
--      the canonical index is ensured immediately before);
--   3. ensures the canonical system scope and All-Father principal row exist.
--
-- The seeded All-Father row is INERT privilege infrastructure, not a grant:
-- its `password_hash` is a documented non-usable placeholder (the Worker
-- rejects password login for ALL_FATHER), and `supabase_user_id` stays NULL
-- until the owner explicitly provisions the Supabase subject mapping. The
-- Worker denies Supabase login for unmapped subjects (reports/auth-repair/
-- 02-auth-contract.md, AUTH-01). No email, email domain, username, or Google
-- identity alone grants ALL_FATHER access. No existing user's email,
-- supabase_user_id, role, or organization is inferred or backfilled here.

-- 1. Canonical auth indexes.
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username_lower ON users (LOWER(username));
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email_lower ON users (LOWER(email)) WHERE email IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_users_role ON users (role);
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_supabase_user ON users (supabase_user_id) WHERE supabase_user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_users_org_role_active ON users (organization_id, role, is_active);

-- 2. Drop the redundant duplicate mapping index from pre-repair 0003
--    (canonical index above is already ensured).
DROP INDEX IF EXISTS idx_users_supabase_user_id;

-- 3. Canonical system scope and inert All-Father principal (same values as
--    `0002_auth_hierarchy.sql`; INSERT OR IGNORE makes this a no-op when the
--    seed already exists, including when a different allfather username row
--    already occupies the unique username index).
INSERT OR IGNORE INTO organizations (id, name, is_active, created_at, updated_at)
VALUES ('__allfather__', 'ONYX System Authority', 1, unixepoch(), unixepoch());

INSERT OR IGNORE INTO users (
    id, username, email, organization_id, password_hash, is_admin, is_active, class, role,
    parent_user_id, created_at, updated_at
) VALUES (
    '__allfather__', 'allfather', 'so.muzaff@gmail.com', '__allfather__',
    '$argon2id$v=19$m=65536,t=3,p=4$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
    1, 1, 'ALL_FATHER', 'ALL_FATHER', NULL, unixepoch(), unixepoch()
);
