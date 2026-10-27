# B-01 Blockers and Decisions — Wave 0

**Gate 0b: NOT APPROVED.** No production client implementation is authorized.

| ID | Blocker / decision | Owner(s) | Exit evidence |
|---|---|---|---|
| B01-B01 | Confirm Rust client owning crate and crate-relative module path; proposed `src/client/*.rs` paths have no confirmed anchor | Integration coordinator / repo owner | Explicit path ownership approval |
| B01-B02 | Reconcile stale Worker source stamp in existing API contract | B-01 / contract owner | Updated source baseline or documented approved rationale |
| B01-B03 | Resolve `POST /api/allfather/admins` DTO mismatch: Worker returns `is_admin`, `role`; UI expects timestamps absent from response | B-01 / B-10 | Canonical DTO decision plus test |
| B01-B04 | Approve C-03 error categories, safe parsing and localization keys | B-01 / B-04 / B-02 | Approved code-to-key map and auth behavior |
| B01-B05 | Approve C-09 fixture schema/version and validate candidates | B-13 / B-01 | Schema, validation report, route/error coverage |
| B01-B06 | Q-01: token persistence/session lifecycle and C-07/C-08 boundaries | B-02 / product owner | Written threat-model and lifecycle decision |
| B01-B07 | Q-05: profile membership control and wire round-trip | B-08 / product owner | Approved DTO/behavior and tests |
| B01-B08 | U-04: policy state diagram and safe command transitions/retry semantics | B-09 / product owner | Approved state model; blocks B-09 implementation |
| B01-B09 | Required root CI license/source-policy audit fails | Security/CI maintainers | Root required checks green on the target revision |
| B01-B10 | Gate 0b not approved | Integration coordinator / user/product owner | Formal gate decision and required approvals recorded |

## Cross-team dependencies

- **B-02 Session & Secrets:** token boundary, refresh/logout, session invalidation.
- **B-04 Localization:** stable error codes and English/Persian C-06 keys.
- **B-05 Login:** login response/error parity and safe diagnostics.
- **B-07 Users:** observed user operation parity and authorization.
- **B-08 Profiles:** DTO/import/export and Q-05.
- **B-09 Policy & Settings:** mobile-access concurrency, policy/legal-hold idempotency, U-04.
- **B-10 Organizations:** canonical organization-admin create DTO.
- **B-13 Verification & CI:** C-09 fixtures and required CI.
- **Repository owner/integration coordinator:** crate path, document ownership, Gate 0b.
- **Security/CI maintainers:** license audit failure.

## Prohibited until blockers close

Do not add production client modules, silently widen contracts, fabricate DTO fields, add unobserved endpoints, choose token storage, implement policy transitions without U-04, or claim tests/CI passed without exact evidence.