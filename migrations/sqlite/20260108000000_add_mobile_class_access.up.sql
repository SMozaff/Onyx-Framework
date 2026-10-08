-- Class-based mobile access control -- SQLite counterpart of the
-- Postgres migration of the same name; see that file for full
-- rationale (restrictive-by-default: an empty table denies mobile
-- login for every class until an admin adds a grant row).
--
-- As with `20260107000000_add_user_class_hierarchy` on this backend,
-- SQLite has no native CHECK-against-list enforcement as strict as
-- Postgres's here (SQLite does support CHECK, so it is included below,
-- but SQLite's type affinity is looser and this is not double-enforced
-- by a separate application-side validator the way `users.class` is
-- documented to be -- `parse_class_field` / `UserClass::parse` is the
-- single source of truth for valid values on both backends).
CREATE TABLE mobile_class_access (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    user_class TEXT NOT NULL CHECK (
        user_class IN (
            'top_level_manager',
            'senior_manager',
            'team_leader',
            'supervisor',
            'staff'
        )
    ),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (organization_id, user_class)
);

CREATE INDEX idx_mobile_class_access_organization_id ON mobile_class_access (organization_id);
