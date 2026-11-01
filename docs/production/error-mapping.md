# Typed error mapping (C-03)

**Status:** Recorded 2026-10-31. Owner approved the mapping on 2026-10-29. Source at main `98a2258`.

## Sources

- Rust protocol: `crates/kernel/platform-contracts/src/error.rs` (`DomainError::category`, `retryability`). Guarded by tests, including `retryability_mapping_matches_ruling_table`.
- Worker: `deploy/cloudflare/edge-gateway/src/index.ts`. Each error response carries `error`, `category`, and `retryability`.

## Protocol values

| Field | Rust protocol | Worker also emits |
|---|---|---|
| `category` | `DOMAIN`, `AUTHORITY`, `CONCURRENCY` | `VALIDATION`, `INFRASTRUCTURE` |
| `retryability` | `RETRYABLE`, `NON_RETRYABLE` | `TRANSIENT` |

## Rust mapping

| Code | Category | Retryability |
|---|---|---|
| `INVALID_TRANSITION` | DOMAIN | NON_RETRYABLE |
| `UNAUTHORIZED` | AUTHORITY | NON_RETRYABLE |
| `VERSION_CONFLICT` | CONCURRENCY | RETRYABLE |
| `EPOCH_CONFLICT` | CONCURRENCY | RETRYABLE |
| `NOT_FOUND` | DOMAIN | NON_RETRYABLE |
| `ALREADY_EXISTS` | DOMAIN | NON_RETRYABLE |
| `INVALID_ARGUMENT` | DOMAIN | NON_RETRYABLE |
| `CONFLICT_PENDING` | DOMAIN | RETRYABLE |

## Worker mapping (observed in source)

- 88 distinct error codes. 76 carry a category.
- Categories: `AUTHORITY` (most access and ownership codes), `DOMAIN`, `VALIDATION` (`INVALID_LOGIN_REQUEST`), `CONCURRENCY` (`USER_CHANGED`, `ORGANIZATION_CHANGED`), `INFRASTRUCTURE` (storage and relay unavailable).
- Retryability: `NON_RETRYABLE` for most codes; `TRANSIENT` for `INFRASTRUCTURE` codes; `RETRYABLE` for `CONCURRENCY` and `OUTBOX_STORE_UNAVAILABLE`.

### Backend accounts codes (2026-11-01)

| Code | HTTP | Category | Retryability | Where |
|---|---|---|---|---|
| `PASSWORD_CHANGE_REQUIRED` | 409 | AUTHORITY | NON_RETRYABLE | Any gated route while `must_change_password` is set |
| `CURRENT_PASSWORD_INCORRECT` | 403 | AUTHORITY | NON_RETRYABLE | `POST /api/auth/password` |
| `PASSWORD_UNCHANGED` | 400 | DOMAIN | NON_RETRYABLE | `POST /api/auth/password` |
| `INVALID_PASSWORD_CHANGE` | 400 | DOMAIN | NON_RETRYABLE | `POST /api/auth/password` |
| `PASSWORD_CHANGE_UNAVAILABLE` | 503 | INFRASTRUCTURE | TRANSIENT | `POST /api/auth/password` |
| `INVALID_STAFF_ROW` | 400 | DOMAIN | NON_RETRYABLE | Single creation (`messages` lists the column problems) |
| `EMAIL_EXISTS`, `ACCOUNT_ID_CONFLICT` | 409 (`USER_EXISTS`) | DOMAIN | NON_RETRYABLE | Single creation |
| `PARENT_NOT_FOUND` | 404 | DOMAIN | NON_RETRYABLE | Single creation |
| `ID_SEQUENCE_EXHAUSTED` | 409 | DOMAIN | NON_RETRYABLE | Single creation (more than 9999 staff IDs in one organization and year) |
| `STAFF_CREATE_UNAVAILABLE` | 503 | INFRASTRUCTURE | TRANSIENT | Single creation |
| `IMPORT_MISSING_COLUMN` | 400 | DOMAIN | NON_RETRYABLE | Import (`column` names the missing column) |

Import row results are not HTTP errors. They appear in the creation report's `account_status` as `created` or `failed: <CODE>`, using the same codes as above (for example `ACCOUNT_ID_CONFLICT`, `ID_SEQUENCE_EXHAUSTED`, `INVALID_STAFF_ROW`, `PARENT_NOT_FOUND`, `STAFF_CREATE_UNAVAILABLE`).

## Decision (2026-10-31)

1. **Keep the Worker's `VALIDATION` and `INFRASTRUCTURE` categories, and its `TRANSIENT` retryability.** The Worker already emits them on dozens of codes. Clients depend on them, so removing them would break responses.
2. **Extend the Rust protocol to match.** Add the two categories and the one retry value, plus tests. This is a follow-up change, not part of this document.
3. **Specific policy codes** (`DRAFT_ALREADY_PENDING`, `NO_DRAFT_VERSION`, `NO_PUBLISHED_VERSION`) follow the U-04 decision.

## Reason

The Rust mapping is the design source, but the Worker is what clients see today. Matching the Worker to Rust would change production responses. Extending Rust to the Worker's values avoids that.
