-- ONYX security model: Supabase/Google is exclusively for All-Father.
-- Admin and Staff credentials, roles, status, and organization membership live in D1.
--
-- Phase 3 repair (reports/auth-repair/03-d1-migrations.md). Git history shows
-- this file was introduced (f397553, 2026-10-10) AFTER `0003_supabase_identity.sql`
-- (7e56462, 2026-10-09) had already shipped, and as committed it duplicated
-- `0002_auth_hierarchy.sql`'s `ALTER TABLE users ADD COLUMN email/role`, its
-- email unique index, the organizations table, and seeded a second competing
-- All-Father principal (`__onyx_allfather__` / `__onyx_root__`). It therefore
-- could never apply on any database where 0002_auth_hierarchy ran, while on
-- databases where 0003 already added `supabase_user_id` any column it added
-- would collide with recorded state.
--
-- The filename is preserved as migration identity (never renamed). The content
-- below is the forward-only correction: deliberately column-free and idempotent
-- so it is safe under EVERY reachable Wrangler ledger state (already recorded →
-- inert; pending → cannot collide with any column another file owns). The
-- `users.supabase_user_id` column and its canonical unique index are owned
-- exclusively by `0003_supabase_identity.sql`; the canonical All-Father
-- principal is the `__allfather__` row seeded by `0002_auth_hierarchy.sql`
-- (matching the Worker's `parent_user_id` convention).
CREATE TABLE IF NOT EXISTS organizations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_users_org_role_active
    ON users (organization_id, role, is_active);
