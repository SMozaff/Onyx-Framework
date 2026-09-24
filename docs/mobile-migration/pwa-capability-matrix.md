# PWA Capability Matrix

**Document:** `docs/mobile-migration/pwa-capability-matrix.md`
**Date:** 2026-09-23
**Status:** PHASE 0 CONTRACT — PWA-FACING RENDERING OF THE SERVER CEILING
**Normative basis:** ONYX-MOB-00 v1.1 §17; ONYX-MOB-01 v1.1 §8

This file is the PWA-facing view of `observer-capability-matrix.md`. It does not create a second security boundary. The backend remains authoritative.

## Observer capability rendering

| Capability | `mobile_observer` | Required PWA behavior |
|---|---:|---|
| Authenticate | YES | Implement login |
| Refresh/logout session | YES | Implement refresh and logout; refresh must not change client class |
| Query/read projections | YES | Implement reads; tenant/object checks still apply |
| Read authorized notifications | YES | Implement notification views |
| View audit/evidence where authorized | YES | Implement only authorized evidence views; no local authorization cache |
| Download authorized files | YES | Implement download/view only after Phase 1 adds an authorized route |
| Register/unregister Web Push | YES | Implement subscription management after Phase 1 adds the routes |
| Execute domain command | NO | Do not expose command submission |
| Create Mission/Task | NO | Do not expose creation |
| Edit Mission/Task | NO | Do not expose editing |
| Approve/reject | NO | Show requirement/status only |
| Lifecycle transition | NO | Show lifecycle/status only |
| Resolve conflict | NO | Show conflict existence/metadata only, or omit sensitive details |
| Upload/delete file | NO | No upload route in the PWA service |
| User/org/policy mutation | NO | No mutation surface |
| Administrative command | NO | No admin surface |

## Presentation rules

- For actionable states, use language such as: **“Action required in an ONYX Operational Client.”**
- Do not render disabled operational controls in a way that implies the PWA is malfunctioning.
- Approvals are requirement/status views, not decision controls.
- Conflicts are visibility-only; do not resolve them.
- Files are download/view-only; every download must be re-authorized by the backend.
- Previously rendered in-memory data may remain visible while offline, but it must be marked as potentially stale.

## Error semantics

The PWA must distinguish at least:

- Authentication failure
- Authorization/read denial
- Observer capability denial
- Not found
- Network unavailable
- Timeout
- Transient infrastructure failure
- Unsupported browser capability

An attempted observer mutation must surface the backend’s deterministic capability denial rather than treating it as a generic failure.
