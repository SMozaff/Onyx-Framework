-- Class-based mobile access control.
--
-- Per explicit product decision (asked of and answered by the project
-- owner directly, not inferred): mobile login is **restrictive by
-- default**. An organization with no rows in this table for a given
-- `UserClass` denies mobile login for users of that class entirely,
-- until an admin explicitly adds a row here. This is the opposite of
-- an allow-everything-until-restricted model — the empty table is the
-- safe state, not a bug.
--
-- Each row is a positive grant: "this class, in this organization, may
-- log in from a mobile client." Admin (`users.is_admin`) is a separate
-- field, not a `UserClass` value (see `user_store.rs`'s module doc),
-- and is intentionally NOT gated by this table — an org's Admin(s)
-- always retain mobile access, mirroring the existing precedent in
-- `api-server::routes::admin::require_class`, where Admin bypasses
-- every class-based allow-list check in this codebase.
--
-- A user with `class IS NULL` (unclassified) can never match a row
-- here, since `user_class` below is NOT NULL -- unclassified users are
-- denied mobile access by construction until given a real class, which
-- is consistent with the restrictive-default decision above.
CREATE TABLE mobile_class_access (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL,
    user_class TEXT NOT NULL CHECK (
        user_class IN (
            'top_level_manager',
            'senior_manager',
            'team_leader',
            'supervisor',
            'staff'
        )
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (organization_id, user_class)
);

CREATE INDEX idx_mobile_class_access_organization_id ON mobile_class_access (organization_id);
