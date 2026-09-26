# Observer Capability Matrix

**Document:** `docs/mobile-migration/observer-capability-matrix.md`
**Date:** 2026-09-23
**Status:** PHASE 0 CONTRACT — BACKEND-FACING
**Normative basis:** ONYX-MOB-00 v1.1 §§4, 17–19; ONYX-MOB-01 v1.1 §§6–9

## Rule

An observer session’s effective capability is the intersection of user permissions and the server-owned observer-class ceiling. A highly privileged administrator using `mobile_observer` still receives only observer capabilities through that client class.

The client must send:

```json
{
  "client_type": "mobile_observer"
}
```

An absent `client_type` is **not** observer access. The backend resolves an absent value to `Web`, preserving legacy compatibility. An explicitly supplied unknown value is rejected during login deserialization.

## Server-owned capability mapping

Implemented in `crates/bins/api-server/src/routes/client_type.rs`.

| Capability | `mobile` | `mobile_observer` | `desktop` | `admin` | `web` |
|---|---:|:---:|---:|---:|---:|
| `can_read_projections` | true | true | true | true | true |
| `can_read_notifications` | true | true | true | true | true |
| `can_read_evidence` | true | true | true | true | true |
| `can_download_files` | true | true | true | true | true |
| `can_submit_domain_commands` | true | **false** | true | true | true |
| `can_approve` | true | **false** | true | true | true |
| `can_transition_lifecycle` | true | **false** | true | true | true |
| `can_resolve_conflicts` | true | **false** | true | true | true |
| `can_upload_files` | true | **false** | true | true | true |
| `can_administer` | true | **false** | true | true | true |

`can_read_evidence` and `can_download_files` are class-level ceilings only. They do not authorize a particular file, evidence object, or download. Object-level tenant, ownership, role, and policy checks still apply.

## Observer-permitted operation classes

- Session control: login, refresh, logout/session revocation.
- Reads through `/api/query`, subject to authentication, tenant isolation, and the query handler’s own validation.
- Profile reads through `/api/profiles` and `/api/profiles/:owner_id`, subject to their existing visibility rules.
- Authenticated user/hierarchy reads through `/api/users` and `/api/users/hierarchy`.
- Authenticated event subscription through `/api/events`, subject to tenant scoping.

## Observer-forbidden operation classes

The backend must deny these with HTTP 403 and the deterministic error below:

- Any `/api/command` submission, including notification acknowledgement, approvals, policy evaluation/registration/retirement, legal-hold release, todo/target/staff-loan lifecycle commands, and every other domain command.
- Todo/target/staff-loan creation through `/api/todo/lists`, `/api/todo/targets`, and `/api/todo/staff-loans`.
- Every admin mutation reachable through `require_admin_mutation`, including user creation, activation/deactivation, password reset, manager/class/parent assignment, mobile-access replacement, profile upsert/import, policy creation, and legal-hold application.
- File upload or deletion, user/org/policy mutation, and administrative command execution.

## Deterministic denial shape

`require_capability` uses the project’s `ApiError` envelope:

```json
{
  "error": {
    "code": "CLIENT_CAPABILITY_DENIED",
    "category": "AUTHORITY",
    "retryability": "NON_RETRYABLE",
    "correlation_id": "<uuid>",
    "safe_details": {
      "client_type": "mobile_observer",
      "required_capability": "submit_domain_command"
    }
  }
}
```

Representative `required_capability` values are `submit_domain_command` and `administer`.

## Refresh behavior

Refresh preserves the classification embedded in the presented refresh token. An observer session cannot be upgraded to an unrestricted class merely by rotating its token.

## Explicitly unresolved for Phase 1

- Whether an observer session may mint a Cloud Relay ticket or otherwise participate in relay/sync transport.
- Whether bootstrap’s unauthenticated, token-gated, one-time account creation needs a client-class rule.
