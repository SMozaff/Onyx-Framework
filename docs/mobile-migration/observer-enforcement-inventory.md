# Observer Enforcement Inventory

**Document:** `docs/mobile-migration/observer-enforcement-inventory.md`
**Date:** 2026-09-23
**Status:** PHASE 0 WORKING INVENTORY — SERVER SOURCE REMAINS AUTHORITATIVE

This inventory distinguishes what the backend already enforces from what Phase 1 must add. It does not change enforcement.

## Already enforced

### Domain-command submission

- `POST /api/command`
  - Calls `require_capability(..., can_submit_domain_commands, "submit_domain_command")`.
  - Source: `crates/bins/api-server/src/routes/command.rs`.

### Todo/target/staff-loan creation

- `POST /api/todo/lists`
- `POST /api/todo/targets`
- `POST /api/todo/staff-loans`
  - Each calls `require_capability(..., can_submit_domain_commands, "submit_domain_command")`.
  - Source: `crates/bins/api-server/src/routes/todo_admin.rs`.

### Admin mutations through `require_admin_mutation`

`require_admin_mutation` authenticates the caller, requires an active admin account, then calls:

```rust
require_capability(&auth, |c| c.can_administer, "administer")
```

Therefore all routes using that helper already enforce the observer ceiling. Covered routes include:

- `POST /api/admin/users`
- `POST /api/admin/users/:id/deactivate`
- `POST /api/admin/users/:id/activate`
- `POST /api/admin/users/:id/password`
- `POST /api/admin/users/:id/manager`
- `POST /api/admin/users/:id/class`
- `POST /api/admin/users/:id/parent`
- `PUT /api/admin/mobile-access`
- `PUT /api/admin/profiles`
- `POST /api/admin/profiles/import`
- `POST /api/admin/policies`
- `POST /api/admin/legal-holds`

Source: `crates/bins/api-server/src/routes/admin.rs`, `profiles/mod.rs`, `profiles/batch.rs`, and `policy_admin.rs`.

### Observer denial and refresh tests

Existing coverage is in `crates/bins/api-server/tests/mobile_observer_capability.rs`:

- Observer reads still succeed.
- `/api/command`, relay-ticket minting, todo/target/staff-loan creation, and every admin mutation route return 403 `CLIENT_CAPABILITY_DENIED`. The admin-mutation sweep covers the full §28 matrix: user creation, activation/deactivation, password reset, manager/class/parent assignment, mobile-access replacement, profile upsert and batch import, policy creation, and legal-hold application.
- A refreshed observer access token remains observer-class.
- Tenant isolation (`TENANT_MISMATCH`) still applies independently of the client-class ceiling for a full-capability session.

## Reads that remain permitted with authentication

- `GET /api/query?envelope=<base64url-query-envelope>`
- `GET /api/users`
- `GET /api/users/hierarchy`
- `GET /api/profiles`
- `GET /api/profiles/:owner_id`
- `GET /api/events`
- Admin reads through `require_admin`, including `GET /api/admin/users`, `GET /api/admin/mobile-access`, and `GET /api/admin/profiles/export`

These routes perform authentication and their own tenant/role checks. They do not invoke observer capability denial because they are not mutation routes.

## Special or undecided server paths

- `POST /api/admin/bootstrap` is unauthenticated, bootstrap-token-gated, and self-closing once any user exists. It has no bearer session and therefore no `client_type` classification.
- `POST /api/relay-ticket` propagates the caller’s authenticated `client_type` into the ticket. **Relay ticket minting requires `submit_domain_command`; observers are excluded.** The code comment previously said relay/sync participation is outside ONYX-MOB-01 §9’s enumerated mutation list; Phase 1 has now ruled that observer participation is denied per the existing `can_submit_domain_commands` ceiling.
- WebSocket `/api/events` validates the caller’s access token and tenant boundary but does not apply a separate capability capability gate to the subscription itself. Message delivery remains tenant-scoped.

## Phase 1.2 additions (previously-confirmed gaps, now closed)

These were the Phase 1 read-route gaps; MIGRATION_PLAN Phase 1.2 closed them:

- `GET /api/files/:content_hash` — gated by `require_capability(..., can_download_files, "download_files")`. Errors: `400 INVALID_CONTENT_HASH`, `404 FILE_NOT_FOUND`, `500 BLOB_STORE_UNAVAILABLE`. Source: `crates/bins/api-server/src/routes/files.rs`. Blobs are served from `ApiState::blob_store` (`LocalBlobStore`). Per-hash downloads are **not** tenant-scoped to a `FileAsset` yet (no `FileAsset` query path exists in api-server); that lookup is the declared Phase 3.1 follow-up — see `docs/mobile-migration/MIGRATION_PLAN.md` and the route's module doc.
- `POST /api/push/subscriptions` / `DELETE /api/push/subscriptions/:subscription_id` — both gated by `require_capability(..., can_read_notifications, "read_notifications")`. Register is idempotent per `(user_id, organization_id, endpoint)`; unregister is scoped to the owning user. Errors: `400 INVALID_SUBSCRIPTION_ENDPOINT` / `INVALID_SUBSCRIPTION_KEY` / `INVALID_SUBSCRIPTION_ID`, `404 SUBSCRIPTION_NOT_FOUND`. Source: `crates/bins/api-server/src/routes/push.rs`. No push delivery worker exists yet (MIGRATION_PLAN Phase 3.2); these routes only store registrations.

Both were confirmed by `crates/bins/api-server/tests/observer_read_routes.rs`, which exercises them through a real `mobile_observer` session (the ceiling applies even to a highly privileged account).
