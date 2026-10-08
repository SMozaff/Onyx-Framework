# User Class Migration: Manual Reclassification of Existing Managers

## Context

Phase A of `IMPLEMENTATION_PLAN_User_Hierarchy.md` introduces a real
class hierarchy (`UserClass`: Top-level Manager, Senior Manager, Team
Leader, Supervisor, Staff) superseding the flat `is_manager: bool`
column. The migration
(`20260107000000_add_user_class_hierarchy`) adds `class` as **nullable**
and does **not** backfill it from `is_manager` — this was a deliberate
decision, not an oversight (see `DECISIONS.md`, Phase A section).

**Explicit decision (2026-08-13):** existing `is_manager = true` users
are **not** auto-promoted to any class on deploy. An Admin must
manually assign each one the correct class. This is safer than
guessing (an auto-promoted `TopLevelManager` might actually belong at
`SeniorManager` or `TeamLeader`, and nobody would have actually decided
that), and this system has no live production user base yet, so the
cost of doing it manually is low.

## Effect on existing managers immediately after deploy

- `is_manager = true` users **keep** whatever `is_manager` already
  granted them (Policy/Settings administration) — `is_manager` is not
  removed by this migration, only supplemented.
- They do **not** automatically gain any `UserClass`-gated ability
  (e.g. anything built against `require_class` in
  `api-server::routes::admin`) until an Admin explicitly assigns them a
  class.
- No user loses access they previously had. This is additive from the
  affected users' point of view — new capabilities require a new,
  deliberate grant.

## Runbook: reclassifying existing managers

**Prerequisite:** an Admin account (`is_admin = true`) and its access
token.

1. **List every user currently `is_manager = true` with no class
   assigned.**

   ```bash
   curl -s -H "Authorization: Bearer $ADMIN_TOKEN" \
     "$API_BASE/api/admin/users" \
     | jq '[.[] | select(.is_manager == true and .class == null)]'
   ```

2. **For each one, decide the correct class** based on their actual
   organizational role — not a default. Consult with whoever owns HR/
   org-chart decisions for this deployment if the correct class isn't
   obvious from the account alone. See
   `DESIGN_User_Hierarchy_Chain_of_Authority.md` §2 for what each class
   means:
   - **Top-level Manager** — one level below Admin, can add/remove
     staff, observes everything beneath their chain.
   - **Senior Manager** — work-related authority only, cannot add/
     remove staff, observes their own supergroups/subgroups.
   - **Team Leader** — supergroup leader, real standing pre-check
     authority on Todo/Target items, no add/remove.
   - **Supervisor** — observes their own subgroup only, no
     verification or escalation authority.
   - (Staff is the base tier — a user who was never really a manager
     in the first place, if any such account has `is_manager = true`
     for another reason, may belong here or may indicate `is_manager`
     was set for a reason unrelated to this hierarchy and needs
     separate investigation before assigning any class.)

3. **Assign the class:**

   ```bash
   curl -s -X POST -H "Authorization: Bearer $ADMIN_TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"class": "top_level_manager"}' \
     "$API_BASE/api/admin/users/$USER_ID/class"
   ```

   Valid `class` values: `top_level_manager`, `senior_manager`,
   `team_leader`, `supervisor`, `staff`.

4. **Assign each user's parent in the reporting line**, if known. This
   is a separate action from class assignment — a user can have a class
   without a parent (e.g. a Top-level Manager may have no parent within
   the organizational tree), but Todo/Target verification (design doc
   §4) and staff loans (design doc §2.1) both depend on
   `parent_user_id` once those features are built.

   ```bash
   curl -s -X POST -H "Authorization: Bearer $ADMIN_TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"parent_user_id": "'"$MANAGER_USER_ID"'"}' \
     "$API_BASE/api/admin/users/$USER_ID/parent"
   ```

5. **Verify** by re-listing users and confirming `class`/
   `parent_user_id` are set as expected:

   ```bash
   curl -s -H "Authorization: Bearer $ADMIN_TOKEN" \
     "$API_BASE/api/admin/users" | jq '.[] | {id, username, is_manager, class, parent_user_id}'
   ```

## Notes

- `set_class`/`set_parent` are both admin-only
  (`api-server::routes::admin::require_admin`) — the same account doing
  the reclassification must itself be `is_admin = true`.
- `set_parent` will reject a self-reference or a cycle
  (`UserStoreError::ParentCycle`, surfaced as HTTP 400
  `PARENT_CYCLE`) and a nonexistent parent id
  (`UserStoreError::ParentNotFound`, HTTP 400 `PARENT_NOT_FOUND`) — see
  `DECISIONS.md`'s Phase A section for how this is enforced on both
  Postgres and SQLite.
- This runbook does not itself decide **when** to remove the
  now-deprecated `is_manager` column — that is a separate, later
  migration once every `is_manager = true` user has been reclassified
  and the team is confident nothing still reads `is_manager` directly.
  Do not drop the column as part of this reclassification pass.
