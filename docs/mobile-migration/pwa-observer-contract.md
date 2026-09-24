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
- Proposed Phase 1 reads:
  - Authorized file download endpoint
  - Push-subscription register/unregister endpoints

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

## Missing backend prerequisites

These Phase 1 additions are required before the planned PWA surface is implementable:

1. An HTTP blob/file-download route with per-download authorization. There is presently no `/api/files/...` route. A `BlobStore` port exists, but it is not exposed over HTTP.
2. Push-subscription register/unregister routes. There are presently no `/api/push/...` routes.
3. A decision on whether observer sessions may obtain relay tickets or participate in sync transport.
