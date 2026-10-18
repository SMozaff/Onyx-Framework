> **Historical / superseded 2026-10-18.** This document records the pre-migration Google/Supabase All-Father flow. Do not use its OAuth, subject-mapping, provisioning, or deployment instructions. The current contract is ONYX username/password with the owner-controlled `npm run provision:allfather-password` command; see [the current migration report](06-onyx-credentials-migration.md) and [authentication contract](../../docs/SECURITY_AUTHENTICATION_MODEL.md).

# ONYX Authentication Repair — Handover (Phases 2–7)

Baseline: `54f7828` (merge of PR #174) — the tip of `main` when this handover started.
Scope: provisioning command contract, browser OAuth PKCE migration, Supabase JWT identity review,
D1 migration integrity, and CI verification. **No production resource was touched.**

---

## 1. Baseline findings (re-verified, not assumed)

| # | Audit lead | Verdict | Evidence |
|---|---|---|---|
| 1 | `package.json` lacks the documented `provision:allfather` script | **Confirmed** | `git show 54f7828:.../package.json` — scripts end at `test`. PR #174's merge resolution dropped it while the README kept referencing it. |
| 2 | Admin Shell still uses the OAuth implicit flow | **Confirmed** | `54f7828` `Login.tsx`: `flow_type=implicit`, token read from `window.location.hash`. |
| 3 | Production readiness unverified | **Still unverified / blocked** | No production credentials or access in this environment (§7). |
| 4 | Duplicate `0002` migration prefixes must be preserved | **Confirmed, already handled** | `0002_auth_hierarchy.sql` + `0002_onyx_identity_model.sql`; `test/migrations.test.ts` covers the reachable ledger histories. |

Two further problems were found during re-inspection and fixed:

- **README duplicated a whole section.** `## Trust boundary: external claims vs ONYX-assigned
  privileges` appeared twice (a merge artifact). Removed the duplicate.
- **Supabase JWT audience was never validated.** `verifySupabaseJwt` checked signature, algorithm,
  issuer, expiry and subject, but not `aud`. `docs/SECURITY_AUTHENTICATION_MODEL.md` already *required*
  "the approved audience/project claim policy" — this was a genuine gap against the normative contract.

Baseline CI was green before any change (all completed, no pending runs):
- Cloudflare Worker CI run 18 — https://github.com/SMozaff/Onyx-Framework/actions/runs/38134620431 — success
- Security run 606 — https://github.com/SMozaff/Onyx-Framework/actions/runs/38134620554 — success
- CI run 1065 — https://github.com/SMozaff/Onyx-Framework/actions/runs/38134620448 — success

---

## 2. Phase 2 — provisioning command contract

- `deploy/cloudflare/edge-gateway/package.json`: restored `"provision:allfather": "node scripts/provision-allfather.mjs"`.
- `README.md`: removed the duplicated section; documented that the npm alias runs from
  `deploy/cloudflare/edge-gateway` and that flags go after `--`
  (`npm run provision:allfather -- --environment production ...`). The direct `node
  scripts/provision-allfather.mjs` examples remain and are unchanged.
- `test/provision-allfather.test.ts`: **5 new tests** — every documented flag maps onto the option
  object, `parseArgs(argv)` round-trips through `normalizeOptions` exactly as the CLI's `main()` does,
  `--help`, trailing/next-token value flags, and a **contract test** that reads `package.json`, asserts
  the alias is exactly `node scripts/provision-allfather.mjs`, and asserts the file it invokes exists.

No safety property was weakened: environment selection, `--approve-production` approval,
`--expect-database` identity check, principal/role/active guards, double-bind refusal, `--rebind` and
`--activate` gating, guarded writes, postcondition verification and auditing are all unchanged and
still covered. **No production provisioning was run.**

## 3. Phase 3 — browser authentication → Authorization Code Flow with PKCE

New module `crates/bins/admin-shell/ui/src/auth/allfatherOAuth.ts`; `Login.tsx` rewritten to use it.

- Verifier: 32 random bytes, base64url (43 chars, RFC 7636 §4.1). Challenge:
  `BASE64URL(SHA256(verifier))`, method `s256` — proven against the **RFC 7636 Appendix B test vector**.
- Authorize URL mirrors Supabase's own `@supabase/auth-js` `_getUrlForProvider` (`provider`,
  `redirect_to`, `code_challenge`, `code_challenge_method=s256`, `apikey`). **`flow_type=implicit` is gone.**
- CSRF `state`: Supabase's `/authorize` owns the `state` parameter and rejects a caller-supplied value
  (supabase/auth#1548), so the nonce travels inside the allow-listed `redirect_to` — the approach
  Supabase documents for application state — and is validated on return exactly like a normal `state`.
- Exchange: `POST /auth/v1/token?grant_type=pkce` with `{auth_code, code_verifier}` using **only the
  publishable key**. No client secret exists in the browser code, and the Worker endpoint is unchanged
  (no invented endpoint; the Worker still verifies the bearer token and applies AUTH-01).
- Callback handling covers success, provider denial, malformed response, **missing / mismatched /
  expired / replayed** state, exchange failure and network failure — each as a stable
  `AllfatherOAuthError` code mapped to one of three generic localized strings (two new i18n keys,
  added to both canonical catalogs and regenerated).
- The code is scrubbed from the URL and history (`history.replaceState`) **before** any await; the
  stored transaction is cleared before the exchange, so a replayed code cannot re-use a verifier; the
  provider's error body (which echoes the code) is never surfaced or logged.
- A legacy implicit-flow fragment (`#access_token=`) is **refused**, not processed.

**Tests:** `tests/allfatherOAuth.test.ts`, **30 tests**, all dependencies injected (no browser needed).
Wired into CI in the `native-ui-evidence` job (`npm test` before the existing build step).
The suite immediately caught a real defect — `beginAllfatherSignIn` persisted the transaction before
validating configuration, so a misconfigured build left a half-finished PKCE transaction behind; fixed
by validating config first.

## 4. Phase 4 — identity / security review

- **Audience validated.** `src/supabase.ts` now requires `aud === "authenticated"` (string or array
  form) and rejects a future `nbf`, in addition to signature, RS256/ES256 allowlist, exact issuer,
  expiry and subject. The constant is hardcoded so the Worker cannot be deployed with the check
  silently off, and the deploy workflow needs no new variable.
- **Negative control.** With only the audience check neutralised, 2 of the new tests fail; with it
  restored all pass — the check is a genuine guard, not a tautology.
- **Verified already-correct behaviour** (no change needed): a valid Supabase identity alone never
  grants All-Father access; `roleForUser` takes privilege only from explicit `users.role`; login never
  writes `users.supabase_user_id`; missing/ambiguous mappings fail closed with no DB write; org/role
  boundaries revalidated on login, refresh and every protected request; no credentials in logs or
  the client bundle.

**Tests:** `test/auth.test.ts` **3 new** (wrong audience, absent audience, array-audience accepted +
future `nbf` rejected) → 13 in the file.

## 5. Phase 5 — D1 migration integrity

No schema change was required in this handover, so **no migration was added** (forward-only is
preserved by not needing one). Migrations were **not renamed, deleted, reordered or rewritten**.

Verified clean installation with the official runner against a disposable local D1:
- `wrangler d1 migrations apply --local` applied all 5 files → 9 commands, exit 0;
- re-run → `No migrations to apply!` (idempotent);
- ledger contains exactly the 5 names, including **both** `0002_*` files;
- schema has exactly one `idx_users_supabase_user` (no duplicate) and exactly one principal
  `__allfather__` with explicit `role=ALL_FATHER`, `is_active=1`, `is_admin=1`,
  `supabase_user_id = NULL` (inert — no privilege granted).

The disposable config and local state were removed afterwards.

## 6. Verification

Local (this VM, Node 24, matching CI's Node 24 for the Worker):

| Check | Result |
|---|---|
| Worker `npm test` | **45 passed** (auth 13, migrations 10, provisioning 22) |
| Worker `npm run check` (bundle dry-run) | exit 0 |
| Admin Shell `npm test` | **30 passed** |
| Admin Shell `npm ci` (lockfile sync, as CI runs it) | exit 0 |
| Admin Shell `npm run build` (`tsc -b && vite build`) | exit 0 |
| Admin Shell `npm run lint` (oxlint) | 0 errors, 3 warnings (all pre-existing in `I18nContext.tsx`) |
| `node scripts/sync-i18n.mjs --check` | in sync (544 keys) |
| Clean D1 install via official Wrangler runner | 5 migrations, idempotent re-run |

### GitHub Actions (branch `fix/auth-pkce-identity-review`, head `4e66602`)

| Workflow | Run | Status |
|---|---|---|
| CI (run 1066) | https://github.com/SMozaff/Onyx-Framework/actions/runs/38140377036 | **success** (all 9 jobs, incl. the new `native-ui-evidence` step) |
| Security (run 607) | https://github.com/SMozaff/Onyx-Framework/actions/runs/38140377049 | **success** |

`native-ui-evidence` job (`https://github.com/SMozaff/Onyx-Framework/actions/runs/38140377036/job/114472853604`)
ran every step successfully, including the new **"Run Admin UI authentication (PKCE) tests"** step
between the Admin UI `npm ci` and `npm run build`.

**Cloudflare Worker CI limitation:** its `push` trigger is filtered to
`[main, 'fix/**', 'migration/**']` *and* `deploy/cloudflare/edge-gateway/**`, and GitHub did not
evaluate the path filter for this push (which created the branch), so no run started for the branch
push. Its `pull_request` trigger has no such gap and will run on the PR. The exact Worker CI steps
were reproduced locally first: `npm install`, `npm run check` (bundle dry-run), `npm test` (45 tests),
and the disposable-D1 `wrangler d1 migrations apply --local` + ledger assertion.

## 7. Production status — all **NOT VERIFIED** (blocked on owner)

No production command, credential or console was available, so nothing below is claimed:

- **Production D1 migrations: NOT VERIFIED.** Not applied from here; the preflight in §8 is the
  read-only way to establish the real state.
- **All-Father provisioning: NOT VERIFIED.** No mapping was read or written in production.
- **End-to-end Google sign-in: NOT VERIFIED.** Requires the deployed Worker, the provisioned mapping,
  and the Supabase redirect allow-list to match the new PKCE `redirect_to`.

One deployment note the owner must act on: the Admin shell now sends `code_challenge` /
`code_challenge_method` to `/auth/v1/authorize`, so the Supabase project's redirect allow-list must
include the deployed Admin shell origin (it already needed to for `redirect_to`).

## 8. Owner-operated production steps (not executed here)

```bash
# 1. Read-only preflight (no writes, prints no hashes/tokens)
npx wrangler d1 execute onyx-free-db --remote --config wrangler.deploy.toml --json \
  --command "SELECT id, username, role, is_active, is_admin, supabase_user_id FROM users"

# 2. Read-only schema contract check
npx wrangler d1 execute onyx-free-db --remote --config wrangler.deploy.toml --json \
  --command "SELECT name FROM d1_migrations ORDER BY name"          # expect >= 5 names

# 3. Apply migrations (only after approving §5's evidence), then deploy the Worker.
# 4. Provision the subject (owner-only, requires explicit approval):
cd deploy/cloudflare/edge-gateway
npm run provision:allfather -- --environment production --config wrangler.deploy.toml \
  --database onyx-free-db --expect-database onyx-free-db \
  --supabase-user-id <SUPABASE_UUID> --dry-run --json
# ...then re-run without --dry-run, adding:
#   --approve-production <SUPABASE_UUID> --operator "<owner>"

# 5. Verify: Google sign-in returns an ONYX session instead of 403 ALLFATHER_NOT_PROVISIONED.
```

## 9. Known pre-existing issues (not introduced by this work)

- `crates/bins/admin-shell/ui` transitively pins `source-map-js@1.2.1` (via `vite`/`@tailwindcss/vite`);
  `npm audit` reports a high-severity DoS (GHSA-68fv-2mgg-jv7q), fixed upstream in 1.2.2. Confirmed
  present at baseline `54f7828`; left untouched to keep this diff scoped — `npm audit fix` would
  bump it.
- The root `.gitignore` contained a prose sentence instead of rules, so nothing (notably
  `node_modules/`) was actually ignored. Replaced with a single `node_modules/` rule; the frontends
  already had their own local `.gitignore`.
