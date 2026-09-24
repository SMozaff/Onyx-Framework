# PWA Observer Contract

**Document:** `docs/mobile-migration/pwa-observer-contract.md`
**Date:** 2026-09-23
**Status:** PHASE 0 CONTRACT — PLANNED SURFACE, NOT AN EXISTING IMPLEMENTATION
**Normative basis:** ONYX-MOB-00 v1.1 §§3.2, 4–5, 13–22; ONYX-MOB-01 v1.1 §§6–9, 14–22

`mobile-pwa/` does not exist yet. This document freezes the intended client-service shape so Phase 1 backend work and Phase 2 PWA work do not invent incompatible meanings of “observer.”

## Client identity

The PWA must authenticate as:

```json
{
  "client_type": "mobile_observer"
}
```

Session refresh must preserve that classification. Logout/session revocation works normally. The backend remains authoritative; absence of mutation methods is defense in depth.

## Intended ObserverClient surface

```text
ObserverClient
  authenticate(...)
  refreshSession(...)
  logout(...)

  getDashboard(...)
  listMissions(...)
  getMission(...)
  listTasks(...)
  getTask(...)
  listApprovalRequirements(...)
  getApprovalRequirement(...)
  listNotifications(...)
  getEvidence(...)
  getAuditView(...)
  getHierarchyView(...)
  getFileMetadata(...)
  downloadFile(...)
  registerPushSubscription(...)
  unregisterPushSubscription(...)
```

The PWA must not expose:

```text
createMission
updateMission
createTask
updateTask
approve
reject
transitionLifecycle
resolveConflict
uploadFile
deleteFile
mutateUser
mutateOrganization
mutatePolicy
executeCommand
```

## Intended HTTP gateway

The gateway must be read-oriented by construction:

- Authentication/session:
  - `POST /api/auth/login`
  - `POST /api/auth/refresh`
  - `POST /api/auth/logout`
- Reads:
  - `GET /api/query?envelope=<base64url-query-envelope>`
  - `GET /api/users/hierarchy`
  - `GET /api/profiles`
  - `GET /api/profiles/:owner_id`
  - `GET /api/events`
- Delivered Phase 1 reads (MIGRATION_PLAN Phase 1.2):
  - `GET /api/files/:content_hash` — content-addressed download, gated by `can_download_files`.
  - `POST /api/push/subscriptions` — Web Push register/upsert, gated by `can_read_notifications`.
  - `DELETE /api/push/subscriptions/:subscription_id` — unregister own subscription only.

The gateway must not contain a generic mutation helper unless that helper is structurally restricted to approved client-control routes.

## Supported read-query inventory

The current backend query handler supports these `query_type` values:

```text
dashboard.summary
mission.list
mission.detail
task.list
task.detail
timeline.list
notification.list
approval.list
report.detail
policy.list
policy.detail
legal_hold.list
legal_hold.detail
todo_list.list
todo_list.detail
target_list.list
target_list.detail
staff_loan.list
staff_loan.detail
```

Unknown query types return an empty result rather than a domain object. The PWA must not infer a missing projection as authorization denial or as proof that the object does not exist.

Source: `crates/bins/api-server/src/query_handler.rs`.

## Delivered backend prerequisites (Phase 1.2)

These were missing from `api-server` when this contract was written; Phase 1.2
has since delivered them:

- `GET /api/files/:content_hash` — `crates/bins/api-server/src/routes/files.rs`.
  Reads through the `ApiState::blob_store` port (`LocalBlobStore` rooted at
  `ONYX_BLOB_STORE_ROOT`, else a host temp dir). Per-hash downloads are not yet
  scoped to a tenant because api-server has no `FileAsset` listing/query path
  (see the route's module doc); that lookup is the declared Phase 3.1 follow-up
  when the PWA `FileList` view lands.
- `POST/DELETE /api/push/subscriptions...` — `crates/bins/api-server/src/routes/push.rs`.
  Registration only stores the browser's endpoint + VAPID keys under the
  session's own `(user_id, organization_id)` key; there is **no push delivery
  worker yet** (MIGRATION_PLAN Phase 3.2). Unregistration is scoped to the
  owning user, so one user cannot remove another's subscription.
- Relay tickets (`POST /api/relay-ticket`) require `submit_domain_command`;
  observer sessions are therefore excluded from Cloud Relay / sync transport.
  See DECISIONS P2P-2.
