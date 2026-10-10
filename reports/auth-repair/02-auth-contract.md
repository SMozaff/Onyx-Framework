# ONYX Authentication Repair — Phase 2 Contract and Implementation Reconciliation

**Assessment date:** 2026-10-10  
**Repository:** SMozaff/Onyx-Framework  
**Phase-start branch:** main  
**Phase-start commit:** 12b7568fa563e40d6bd974f0bdd0c681c54ef916 (docs(auth): record Phase 1 authentication repair baseline)  
**Working branch:** auth-repair/02-auth-contract  
**Baseline:** reports/auth-repair/01-baseline.md reviewed.

## Revision (2026-10-10): Phase 2 implementation at commit 2066216

The contract above was re-verified against `main` tip `2066216ee08a7222df6bc64f9088a87b3c4791ce`
and the two unambiguous contract violations were fixed with focused tests. Findings AUTH-02 and
AUTH-03 through AUTH-12 remain open and deferred exactly as recorded in §6–§8; migration files,
production configuration, secrets, redirect allowlists, and Worker variables were not touched.

**Code changes (narrow, evidence-tied):**

- AUTH-01 fixed in `deploy/cloudflare/edge-gateway/src/index.ts` (`authSupabaseAllFather`):
  the `UPDATE users SET supabase_user_id=...` login-time write is removed. The exchange now
  requires an already-provisioned `users.supabase_user_id` equal to the verified Supabase `sub`;
  unmapped or mismatched subjects are denied `403 ALLFATHER_NOT_PROVISIONED` with **no database
  write**, and a safe reason (`SUBJECT_UNMAPPED` / `SUBJECT_MISMATCH`) is logged server-side.
- AUTH-04 fixed in the same handler: verifier failures previously echoed `error.message` to the
  client; the public response is now the stable code `401 INVALID_SUPABASE_TOKEN`, with the
  verifier reason logged server-side as a structured `allfather_token_rejected` event that never
  includes the presented token.
- No other runtime handler changed. `authLogin`, refresh, logout, and all protected routes are
  byte-for-byte as reviewed above.

**Tests (new):** `deploy/cloudflare/edge-gateway/test/auth.test.ts` — 9 vitest cases (repo
convention: vitest, as in `web-ui`/`mobile-pwa`) covering: provisioned-subject success with zero
writes; AUTH-01 unmapped and mismatched subject denials with write assertions; AUTH-04 generic
401 with no token/verifier-detail leakage in body or logs; identity/`sub` mismatch; unconfirmed
email; wrong issuer; and password-path anti-enumeration (unknown identifier vs wrong password
responses are byte-identical). `package.json` gains `"test": "vitest run"` and a `vitest`
devDependency; `.github/workflows/cloudflare-worker-check.yml` now runs `npm test` after
`npm run check`, so the tests are covered by CI on every Worker PR/push.

**Local verification (Node 24, same as CI):** `npx vitest run` → 9/9 passed;
`npm run check` (`wrangler deploy --dry-run`) → passed.

**Documentation:** `deploy/cloudflare/edge-gateway/README.md` gained a "Trust boundary" section
separating externally authenticated claims (Supabase `sub`/`iss`/`exp`, resolved Auth user,
confirmed email) from ONYX-assigned privileges (`users.role`, `is_active`, `organization_id`,
the subject mapping), plus the stable-public-error-code policy.

**Intentional API contract change and client impact:**

| Change | Impact |
|---|---|
| `POST /api/auth/supabase` failure body: raw verifier message → stable `INVALID_SUPABASE_TOKEN` | `Login.tsx` shows generic i18n text for HTTP failures; no client code depends on the old strings (grep verified: no test or client matches them) |
| `POST /api/auth/supabase` on unmapped/mismatched subject: session issuance → `403 ALLFATHER_NOT_PROVISIONED` | **First login after any deploy now requires `users.supabase_user_id` to be provisioned out-of-band by the owner** (open decision §8.2). This is the contract's explicit requirement; deployment must not occur before that provisioning step exists |

**Remaining blocker note:** `scripts/migrate-worker-clerk-to-supabase.py` still carries the legacy
rewrite text (login-time subject binding and raw error passthrough). It aborts before writing
because its `clerkIssuer` anchor no longer exists in `index.ts`, so it cannot revert these fixes;
it should be retired by the owner in a later phase rather than re-run.

No production deployment, migration, secret, or identity change was made. Phase 2 stops here.

## 1. Decision and scope

The intended product contract is clear enough to document, but the current Worker implementation does not fully satisfy it. Runtime changes are deferred because the gaps affect privileged identity binding, OAuth callback architecture, JWT validation, session lifecycle, and authorization boundaries. These require focused tests and owner decisions about supported OAuth clients. No migration files, production configuration, secrets, redirect allowlists, deployed services, or production data were changed. Repository API inspection cannot establish the local working-tree state or production behavior.

The duplicate 0002 migration/schema conflict documented in Phase 1 remains a blocker for migration/deployment work, but does not prevent this documentation-only contract phase.

## 2. Canonical authentication contract

### A. ONYX-managed Admin and Staff login

1. Client submits username or email plus password to POST /api/auth/login.
2. Worker resolves the account from the authoritative D1 users record and verifies its Argon2id hash.
3. Reject invalid credentials, inactive accounts, and the All-Father principal. Credential failures must use the same generic public response.
4. Issue ONYX access and refresh tokens only after validation.
5. Derive role and organization from D1. Never authorize from client-supplied is_admin, role, organization_id, class, parent_user_id, or allfather fields.
6. All-Father provisions organizations and organization Admins. An organization Admin provisions/deactivates Staff only inside their own active organization. Staff cannot self-register or elevate privileges.

Current evidence: authLogin queries by username/email, verifies Argon2id, rejects inactive users and ALL_FATHER, and returns INVALID_CREDENTIALS for ordinary credential failures. The Admin Shell UI currently labels its input username, although the Worker accepts email too.

### B. All-Father Google/Supabase login

1. Google authenticates the designated identity through the configured Supabase Auth project.
2. Client obtains a Supabase access token using a callback flow supported by its actual platform.
3. Worker validates JWT signature, accepted algorithm, exact issuer, expiry, subject, and the required audience/project claim policy.
4. Worker resolves the token through Supabase Auth and requires returned user ID to equal JWT sub, a confirmed email, and the designated All-Father identity.
5. Supabase subject must match an explicitly provisioned supabase_user_id on the one canonical ONYX All-Father row. Login must not silently bind or replace this mapping.
6. Worker confirms that ONYX principal is active and has role ALL_FATHER, then issues an ONYX session.
7. Only server-side ONYX authorization grants All-Father access. A Google account alone, browser flag, user metadata, or unknown Supabase subject is insufficient.

Current evidence: src/supabase.ts verifies JWT signature, algorithm, issuer, expiry and subject, and resolves /auth/v1/user. The exchange checks subject equality, confirmed email, and expected email, but currently updates users.supabase_user_id when it differs. The verifier does not enforce an audience claim. The handler returns verifier error.message in its 401 response.

### C. Session and authorization contract

Existing behavior is a starting point, not proof of completeness:

- Access token lifetime: 1 hour.
- Refresh token lifetime: 7 days.
- Refresh: validates refresh token signature/expiry/revocation, rechecks account active state, revokes the presented refresh token hash, and issues a new pair.
- Revocation: token hashes are stored in D1 token_revocations and checked by JWT verification.
- Logout: POST /api/auth/logout revokes presented bearer and/or body refresh token hashes. The inspected Admin Shell auth store clears local state only; server revocation wiring was not found there.
- Protected request validation re-reads the user and compares active status, organization and role. It does not check organization active status and does not compare every privilege-bearing session field.
- Tenant isolation checks exist on several query, command, and user-management paths, but do not replace action-specific authorization for every command/resource.
- Public errors must be generic. Safe structured reasons may be logged server-side; never log or return tokens, passwords, OAuth codes, cookies or secrets.
- Provisioning/deactivation write audit records. Auth success/failure, refresh, logout/revocation, unknown Supabase subject, and privileged denials need a defined audit policy.

## 3. Canonical identity vocabulary

| Field/concept | Canonical meaning |
|---|---|
| users.id | ONYX principal ID; ONYX session subject |
| users.username | ONYX login username |
| users.email | ONYX-managed email/login identifier |
| users.supabase_user_id | Explicitly provisioned external Supabase subject; never login-auto-bound |
| users.organization_id | ONYX tenant boundary for Admin/Staff; reserved system scope for All-Father only where schema requires it |
| users.role | Authoritative authorization role: ALL_FATHER, ORGANIZATION_ADMIN, STAFF |
| users.is_active | Authoritative ONYX account state |
| users.is_admin | Legacy/compatibility field; not an independent role model or client authority claim |
| users.class | Operational user class, distinct from authentication role |
| users.parent_user_id | Reporting-line relation, not identity or privilege proof |
| ONYX JWT sub | users.id |
| Supabase JWT sub | External Supabase Auth user ID; must match the explicitly provisioned mapping |

D1 is authoritative for ONYX role, active state, and organization. Supabase authenticates the external identity; it does not assign ONYX roles or tenant membership.

## 4. Route-to-handler matrix

Source reviewed: deploy/cloudflare/edge-gateway/src/index.ts and src/supabase.ts at the phase-start commit. This matrix does not prove deployed routes or production success.

| Route | Handler / boundary | Current behavior and gap |
|---|---|---|
| POST /api/auth/login | authLogin | Username/email + Argon2id; rejects inactive and All-Father; generic credential failure. Does not check organization active state. |
| POST /api/auth/supabase | authSupabaseAllFather | JWT and Supabase user checks; confirmed designated email and subject equality; auto-binds/replaces supabase_user_id; no audience check. |
| POST /api/auth/clerk | Alias to authSupabaseAllFather | Misleading legacy provider alias; compatibility inventory required before removal. |
| POST /api/auth/refresh | authRefresh | Validates refresh token and account active state, revokes old refresh token, issues new pair. Organization state and all privilege claims need revalidation. |
| POST /api/auth/logout | authLogout | Revokes supplied bearer and/or refresh token hashes; client wiring is not established. |
| Current-user/session introspection | No dedicated route found | No canonical /api/auth/me route found in inspected router. Client relies on stored login/session state. |
| POST /api/allfather/organizations | requireSession then createOrganization / requireRole(ALL_FATHER) | Server role gate present. Organization state and reserved scope consistency need review. |
| POST /api/allfather/admins | requireSession then createUser(ORGANIZATION_ADMIN) / requireRole(ALL_FATHER) | Server role gate and active target-organization lookup present. |
| POST /api/admin/staff | requireSession then createUser(STAFF) / requireRole(ORGANIZATION_ADMIN) | Staff organization derived from session rather than request body. |
| DELETE /api/admin/users/:id | requireSession then deactivateUser | Admin can deactivate Staff in same organization; All-Father can deactivate Admin. |
| GET /api/users/hierarchy | requireSession then hierarchy | Query scoped to session organization. |
| GET /api/query, GET /api/profiles, GET /api/profiles/:id, GET /api/events, GET /api/files/:hash | requireSession then handler | Several queries/download lookups are tenant-scoped; complete resource/action authorization and active-organization checks remain to be reviewed. |
| POST /api/command | requireSession then commandRoute | Observer restriction exists, but no complete role/action/aggregate authorization matrix is evident. Authentication must not authorize arbitrary domain mutations. |
| POST/DELETE /api/push/subscriptions | requireSession then push handlers | Subscription ownership scoped by user and organization. |
| GET /ready | D1 SELECT 1 | Basic D1 reachability only; auth schema readiness is deferred to a later phase. |

## 5. Request traces

### ONYX password path

Admin Shell form → POST /api/auth/login → authLogin → D1 username/email lookup → Argon2id and account-state checks → issuePair → ONYX access/refresh JWTs → client auth store.

Protected request: bearer ONYX access token → jwtVerify (HS256 signature, expiry, revocation) → requireSession (current D1 active/org/role checks) → route handler → route-specific role/tenant checks → D1 operation → response.

Gaps: login UI exposes only username; no current-user endpoint; store logout is local-only; organization active status and all privilege-bearing claims are not consistently revalidated; generic command authorization needs a dedicated matrix and tests.

### All-Father Google/Supabase path

Current client flow: Admin Shell All-Father mode → Supabase /authorize with provider=google and flow_type=implicit → redirect fragment containing access_token → Login.tsx reads fragment → POST /api/auth/supabase → verifySupabaseJwt (JWKS signature/issuer/expiry/sub) → Supabase /auth/v1/user → confirmed designated email and matching ID → D1 All-Father row → current code auto-binds/replaces supabase_user_id on mismatch → issue ONYX session.

Required behavior differs at D1 mapping: subject must match an already provisioned mapping; mismatch must be denied without writing. The current inspected code uses window.location.origin and an implicit-style callback. Whether this Admin Shell is supported in browser, Tauri desktop, or both, and which callback scheme/allowlist applies, was not proven by repository API evidence.

## 6. Findings

| ID | Severity | Finding | Consequence |
|---|---|---|---|
| AUTH-01 | Critical | Supabase subject auto-bound/replaced during login | **Fixed in revision above:** unmapped/mismatched subject is now denied with no database write; mapping requires owner provisioning. |
| AUTH-02 | High | Supabase JWT audience claim is not checked | Signature, issuer, expiry and subject checks exist, but audience/project claim validation is absent. |
| AUTH-03 | High | OAuth uses implicit-style access-token fragment callback | PKCE/code exchange and Tauri callback behavior are not established; token reaches custom front-end callback code. |
| AUTH-04 | High | Supabase verifier error details are returned to clients | **Fixed in revision above:** public 401 now carries only `INVALID_SUPABASE_TOKEN`; reason logged server-side without token material. |
| AUTH-05 | High | No dedicated session introspection/current-user route | Client hydration relies on stored response until a protected request fails. |
| AUTH-06 | High | Server logout is not wired in the inspected auth store | Local logout may not revoke server tokens. |
| AUTH-07 | High | Generic command authorization is incomplete | /api/command does not demonstrate a complete server-side role/action/resource matrix. |
| AUTH-08 | Medium | Session validation omits organization active state and some privilege fields | Stale or inconsistent claims may survive until expiry or another check. |
| AUTH-09 | Medium | /api/auth/clerk aliases the Supabase handler | Provider boundary is obscured; remove only after compatibility review. |
| AUTH-10 | Medium | Supported OAuth client targets are unresolved | Redirect/callback correctness for browser and Tauri is unproven. |
| AUTH-11 | Medium | Authentication audit coverage is undefined/incomplete | Provisioning/deactivation are audited; auth lifecycle and privileged denial coverage need policy. |
| AUTH-12 | Medium | Duplicate 0002 migration/schema conflicts remain | Phase 1 identified competing All-Father seeds and overlapping columns; deployed impact is unknown. |

## 7. Tests and verification

Phase 1 found no dedicated Worker authentication test file in the inspected tree; Worker CI runs npm run check / wrangler deploy --dry-run. No Worker unit/integration tests were added here because runtime changes need a focused test harness and decisions on OAuth client flows. No test suite was executed through the available GitHub file operations. Production Worker, deployed D1 schema/history, Supabase project settings, redirects, and real login remain unverified.

## 8. Unresolved owner decisions

1. Is Admin Shell supported in browser, Tauri desktop, or both? Select and document callback mechanics for each supported runtime; prefer Authorization Code + PKCE where supported.
2. Which trusted owner-controlled provisioning process initially writes the single users.supabase_user_id mapping? Login must never create/replace it.
3. Confirm the expected Supabase JWT audience/project claim policy and enforce it.
4. Does logout revoke only the presented token pair or all sessions for the principal? Is per-token revocation sufficient, or is a session-family identifier required?
5. Define canonical GET /api/auth/me response and client hydration behavior.
6. Should a disabled organization immediately invalidate member sessions and block login/refresh? Recommended: yes.
7. Approve role × route/action × aggregate ownership/manager × client capability rules for generic commands and read routes.
8. Define audit coverage/retention for login, failures, refresh, logout/revocation, unknown Supabase subject, and privileged denial without storing secrets.
9. Define compatibility window for /api/auth/clerk before deprecation/removal.
10. Standardize generic public errors and safe server-side reason codes.

## 9. Conclusion

**Contract documented; current implementation is not yet compliant; no runtime code or migrations changed.** The most urgent issue is automatic Supabase subject binding. The next implementation phase should add focused Worker tests for negative authorization cases, then require an explicitly provisioned subject mapping and harden JWT claim validation. OAuth callback redesign and command authorization should follow the owner decisions above. Production deployment and D1 migration work remain out of scope.
