# C-03 Worker Error Mapping — Proposal

**State: DRAFT — requires B-04 localization and contract-owner approval.**  
**Gate 0b remains closed.**

## Observed error forms

The Worker does not expose one universal error envelope. Source includes string `error`, optional `code`, `category`, `retryability`, optional safe metadata, and structured command error objects. Client parsing must be defensive and retain distinctions without displaying raw server text by default.

## Proposed normalization layers

1. **Transport:** connection failure, timeout, abort, or no response.
2. **HTTP:** non-success status with parsed known error envelope or unknown/malformed body.
3. **Decode/contract:** invalid JSON, wrong content type, or response DTO validation failure.
4. **Application/command:** HTTP response received but command reports `success:false` or a nested structured error.
5. **Conflict/version:** explicit concurrency conflict, including mobile-access `expected_version` mismatch.
6. **Authentication/session:** unauthorized, invalid/expired/revoked session; coordinate refresh behavior with B-02/B-05.
7. **Unknown:** preserve status and safe correlation metadata only; do not expose raw body.

## Mapping proposal

| Condition | Internal category proposal | Handling |
|---|---|---|
| Network/timeout | `NETWORK` / `TIMEOUT` | User-safe retry guidance; retry only if operation is safe |
| 401/session invalid | `AUTH_REQUIRED` / `SESSION_INVALID` | Delegate lifecycle to approved B-02 behavior; never loop refresh indefinitely |
| 403 | `FORBIDDEN` | No automatic retry; no sensitive detail disclosure |
| 404 | `NOT_FOUND` | Map by operation context, not raw server message |
| 409/version conflict | `CONFLICT` | Surface stale state; reread canonical state before user-authorized retry |
| 422/validation | `VALIDATION` | Map known field/code errors to approved localized messages |
| 429 | `RATE_LIMITED` | Honor safe retry metadata if source provides it; no blind mutation retry |
| 5xx | `SERVER` | Safe generic message; mutation retry only with approved operation semantics |
| malformed/unknown envelope | `PROTOCOL` / `UNKNOWN` | Record safe diagnostics; never log raw secrets/body |

These are proposed client categories, not claims that every listed status is currently emitted by every handler. The implementation must map only statuses/codes verified in C-01.

## Localization and security constraints

- B-04 approves stable internal codes and C-06 translation keys (English/Persian).
- Never display arbitrary `error` strings without an approved safe allowlist.
- Never log passwords, access/refresh tokens, raw bodies or secret-bearing headers.
- Preserve safe status, known code and correlation metadata; redact user-controlled values.
- Distinguish HTTP success from command-level success.

## Required approvals

B-01 documents observed envelopes and implements only approved parser semantics. B-04 confirms key names/translations; B-02 confirms auth/refresh handling; B-13 adds fixture coverage for each approved error shape; integration owner approves final C-03 contract.