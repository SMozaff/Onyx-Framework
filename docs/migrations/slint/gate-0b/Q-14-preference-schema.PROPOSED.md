# Q-14 theme/language preference schema — PROPOSED draft (2026-11-23)

**Status: PROPOSED and BLOCKED by owner decision D-05.** The owner approved persistence in
direction (decision log: "Persist theme and language preferences across launches"), but the schema
content was never committed. G0B-06 stays OPEN. Do not implement from this draft without approval.

## Source evidence

- Decision log: "Remaining contract work: define storage location, schema/versioning, defaults,
  corruption fallback, and tests." Later entry: "the owner approved the schema, but it is not in
  the repo. Fields, defaults, and fallback must be provided before they can be implemented."
- Master record G0B-06 exit evidence: "Commit exact schema, defaults, storage location, versioning,
  corruption fallback and tests."
- BOUND brief B-04: use separate non-secret preference storage; never store theme/language inside
  authentication tokens or the auth-session record by default; define restore timing and
  corrupt/unavailable-storage behavior, and whether All-Father is included.
- `docs/slint/CONTRACT_FREEZE_V1.md`: C-06 Localization "Not frozen. Q-14 persistence not implemented."

## PROPOSED schema (not approved)

| Field | Proposed default | Notes |
|---|---|---|
| `language` | `"en"` | One of the registered locale tags; unknown values fall back to `en` |
| `theme` | `"light"` | One of `light` / `dark`; unknown values fall back to `light` |
| `schema_version` | `1` | Integer, migrated on read; future versions must be additive |

Storage boundary (proposed): a separate non-secret preference file per application, outside the
auth session and keyring. On missing/corrupt/unreadable storage, start from defaults and continue;
do not treat the failure as a logout. Apply the same rule to All-Father sessions unless the owner
decides otherwise. Restore timing: at startup, before the first frame, after catalog load.

## Open decisions (owner, D-05)

- Exact storage location/name per app and per OS.
- Whether All-Father is excluded.
- Version migration policy and tester/date ownership.

## Acceptance (not met)

Approved schema artifact plus tests for missing, corrupt, unknown-value and version-migration
cases. Registration Admin Slint currently starts from English/light each launch; no persistence
implementation is claimed.
