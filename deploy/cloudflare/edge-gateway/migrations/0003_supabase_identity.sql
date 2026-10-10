-- Supabase Auth identity linkage.
-- Supabase authenticates the external identity; Cloudflare D1 remains the ONYX
-- authorization/system-of-record database.
--
-- Phase 3 repair (reports/auth-repair/03-d1-migrations.md): this file previously
-- created a second, differently named unique index (`idx_users_supabase_user_id`)
-- in addition to the column it added. Git history shows it is the OLDEST of the
-- conflicting identity migrations, which makes it the one file whose ledger
-- state and schema effect are consistent across every reachable history: either
-- it already shipped (this file is inert in that database) or it is pending and
-- adds the `supabase_user_id` column exactly once — `0002_onyx_identity_model.sql`
-- is deliberately column-free so it can never collide with it, and
-- `0004_auth_schema_repair.sql` only contains idempotent statements.
-- The canonical unique index name going forward is `idx_users_supabase_user`
-- (created below, and ensured by 0004 for databases where this migration
-- already shipped with its pre-repair index name).
ALTER TABLE users ADD COLUMN supabase_user_id TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_supabase_user
    ON users (supabase_user_id) WHERE supabase_user_id IS NOT NULL;
