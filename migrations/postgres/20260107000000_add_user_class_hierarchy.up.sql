-- ONYX Phase A (User Hierarchy & Chain of Authority), per
-- IMPLEMENTATION_PLAN_User_Hierarchy.md §2 and
-- DESIGN_User_Hierarchy_Chain_of_Authority.md §2-§2.3.
--
-- Supersedes the `is_manager` boolean (added in
-- 20260106000000_add_manager_role) with a real class hierarchy, per the
-- explicit product decision recorded in the design document §3: a flat
-- boolean cannot express distinct tiers with different add/remove
-- rights and observation scopes (Top-level Manager, Senior Manager,
-- Team Leader, Supervisor). `is_admin` is untouched — Admin is
-- confirmed as its own thing, not a `class` value (design doc §2's
-- table treats Admin as the top of the organizational hierarchy,
-- distinct from Allfather, which is a separate, not-yet-built,
-- platform-level concept per design doc §2.4 — deliberately not
-- represented in this table; see IMPLEMENTATION_PLAN §3/B.1).
--
-- Two-step migration by design (per IMPLEMENTATION_PLAN §2 A.3/A.4):
-- this step ADDS the new columns without removing `is_manager` yet.
-- `is_manager` is dropped in a later migration only after the
-- application-level backfill decision (which existing managers become
-- which class) has been made and executed — that decision is a real
-- operational judgment call flagged to the person, not something this
-- migration should silently decide by picking a default.
--
-- `class` is nullable: a NULL class means "no class assigned" (plain
-- Staff-equivalent, or not yet classified) — distinct from an explicit
-- `staff` value, so existing users are not silently reclassified as
-- Staff by this migration either. Every existing user (including
-- current is_manager=true ones) starts NULL here; assigning a real
-- class is a deliberate, separate, application-level action per user,
-- not automatic.
ALTER TABLE users ADD COLUMN class TEXT NULL
    CONSTRAINT chk_users_class CHECK (
        class IS NULL OR class IN (
            'top_level_manager',
            'senior_manager',
            'team_leader',
            'supervisor',
            'staff'
        )
    );

-- Self-referential reporting line (design doc §2.1: "every staff member
-- has exactly one true owning Manager"). NULL for users with no parent
-- (e.g. Admin, or a Top-level Manager who reports to no one within the
-- organizational tree). ON DELETE SET NULL rather than CASCADE or
-- RESTRICT: deleting a manager should not cascade-delete their reports,
-- nor should it be blocked by their existence — it should simply orphan
-- the reporting line, which the application layer must then handle
-- (reassign or leave unmanaged) rather than the database silently
-- deleting staff records or refusing the deletion outright.
ALTER TABLE users ADD COLUMN parent_user_id UUID NULL
    REFERENCES users(id) ON DELETE SET NULL;

-- Supports "list my direct reports" and verifier-resolution lookups
-- (IMPLEMENTATION_PLAN §5 D.4).
CREATE INDEX idx_users_parent_user_id ON users (parent_user_id);

-- Note: a self-referential FK does not by itself prevent a cycle
-- (A -> B -> A) or a user being their own ancestor. Postgres has no
-- built-in "no cycles" constraint. Per IMPLEMENTATION_PLAN §2 A.2, this
-- must be enforced in application code at assignment time, not assumed
-- to be caught here.
