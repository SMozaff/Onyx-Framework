# ONYX — Remaining Work: Technical Source for Coding Agents

**Snapshot date:** 2026-11-22
**Repository:** `SMozaff/Onyx-Framework` (GitHub)
**Purpose:** one authoritative list of everything not yet done in the Slint migration and its supporting Worker/backend work, plus a division of that work into stages that a single agent can complete from one prompt.
**Rule for this document:** every statement is tagged. `VERIFIED` = observed in the repo, CI or a tool result on the snapshot date. `DECIDED` = recorded owner decision. `VERIFY FIRST` = not established; the agent must check before acting. Nothing marked `VERIFY FIRST` may be assumed.

---

## 0. Common Preamble (prepend to every stage prompt)

1. **Never assume, never guess.** If a fact is `VERIFY FIRST`, check it in the repo. If it cannot be established, stop and report; do not invent.
2. **Read first:** `docs/migrations/slint/MASTER-MIGRATION-RECORD.md`, then `docs/migrations/slint/decision-log.md`. Do not ask the owner a question those files already answer. Decision log is append-only.
3. **Gate 0b is BLOCKED / NOT PASSED** (`MASTER-MIGRATION-RECORD.md`). Production Admin/Desktop client modules are not authorized until the owner signs Contract Freeze v1. Work in this document is labelled *feasibility* (may be built and reviewed) or *Gate-0b-dependent* (must not start before the gate is signed).
4. **Never** deploy, run D1 migrations, touch production data, merge to `main` outside a reviewed PR, or weaken a CI/security policy unless the stage says the owner approved it.
5. **Never** read, print, copy or use credentials. The project has a doc named "GitHub token"; do not use it. Use only the credentials your environment already provides.
6. **Small reviewed branches.** One stage = one or a few PRs. Branch from current `origin/main`. Push the branch, open a PR, wait for CI, report. Do not stack branches unless the stage says so.
7. **Tests are part of the work.** Run the commands in Appendix A for every crate/package you touch. Report real results, not expectations.
8. **Record decisions and changes**: append to `docs/migrations/slint/decision-log.md` (dated, with who decided). Do not rewrite history.
9. **Owner-only decisions** (section 4) are never made by an agent. Use the stated default only where the stage says so, label it `RECORDED`, and list it in the PR body.
10. **Stop conditions:** a failing check you cannot explain, a missing file the stage depends on, a conflict with a `DECIDED` item, or any step needing credentials. Stop and report with evidence.
11. **Report format:** what changed (files), what was run (commands + results), what is unverified, what needs the owner.

---

## 1. Verified state snapshot (2026-11-22)

| Item | State |
|---|---|
| `origin/main` | `d2dba18` ("Merge pull request #473 …"). `VERIFIED` |
| Main CI on `d2dba18` | Failing: `merge-gate`, `Security audits / rust-advisories`, `check`, `Test and build (ubuntu-latest)`, `Test and build (windows-latest)`. `VERIFIED` Known causes: Ubuntu build error E0063 (missing `must_change_password` in a `StoredSession` initializer in `crates/bins/desktop-slint/src/composition.rs`); `rust-advisories` was already failing before this work. Causes of `check`, Windows test and `merge-gate` after #473 were **not diagnosed**. |
| PR #474 `fix/stored-session-field` | Open, `mergeable_state=unstable`, +1 commit. Adds `must_change_password: false` to the fixture. Local `cargo test -p desktop-slint -p desktop-session` passed (39 + 3). CI was still running when last checked. `VERIFIED` |
| Open Dependabot PRs | 17: #475–#492 excluding #474. `VERIFIED` Their branches, plus branches of earlier merged Dependabot PRs, still exist on the remote. |
| Worker work | Branches `feat/allfather-audit-worker`, `feat/staff-org-login`, `feat/staff-org-login-desktop` are in `main`. `VERIFIED` Worker is **not deployed** by this work. `VERIFY FIRST` whether a Cloudflare Workers Build redeployed on merge. |
| Admin Slint on `main` | `crates/bins/admin-slint/`: `Cargo.toml`, `build.rs`, `src/{auth,i18n,i18n_catalog,main,users}.rs`, `ui/app.slint` (feat/admin-slint-phase1). `VERIFIED` |
| Admin Slint not on `main` | Stack of unmerged branches, each based on the previous (commit counts ahead of `main`): `feat/admin-users-writes` +1 → `feat/admin-organizations` +2 → `feat/admin-settings` +3 → `feat/admin-profiles` +5 → `feat/admin-policies` +6 → `feat/admin-legal-holds` +10. `VERIFIED` These were built against an older `main`; expect conflicts (notably `Cargo.lock`). |
| Gate 0b | Not signed. Blockers G0B-01…G0B-14 and others in `MASTER-MIGRATION-RECORD.md` §5. `VERIFIED` (document read 2026-11-22) |
| Live testing | None of the Admin Slint features or the Tauri desktop login/password-change screen has been exercised against a deployed Worker or run as a GUI. `VERIFIED` |

---

## 2. Reference facts (from this work; use instead of re-deriving)

### 2.1 Layout

- Worker: `deploy/cloudflare/edge-gateway/` — `src/index.ts`, `src/command-semantics.ts`, `src/staff-rules.ts`, `migrations/` (latest `0011_organization_server_url.sql`), tests under `test/` (vitest; 264 tests in 17 files on 2026-11-16 run). Migration filenames are pinned in `test/migrations.test.ts`. Config: `wrangler.toml`, `wrangler.dev.toml`. Deploy workflow: `.github/workflows/deploy-cloudflare-edge.yml` (generates `wrangler.deploy.toml` at run time; Worker name `onyx-framework`, D1 `onyx-free-db`, workers.dev enabled, no custom domain — from reading the workflow; `VERIFY FIRST` before acting).
- Admin Slint: `crates/bins/admin-slint/` — modules `auth`, `users`, `orgs`, `settings`, `profiles`, `policy`, `legal_hold` (the last four on unmerged branches), `i18n`, generated `i18n_catalog.rs` (do not edit); UI in `ui/app.slint` (globals `Theme`, `Strings`).
- Desktop (Tauri): `crates/bins/desktop-shell/` (Rust `src/lib.rs`; React UI `ui/src/` with `pages/Login.tsx`, `pages/ChangePassword.tsx`, `hooks/useSession.tsx`, `App.tsx`).
- Desktop Slint host: `crates/bins/desktop-slint/` (`src/main.rs`, `src/composition.rs`). `src/main.rs:40` calls `composition.login(&username, &password)` — two fields, no organization. `VERIFIED`
- Shared session: `crates/applications/desktop-session/` — `session.rs` (`SessionApi`, `HttpSessionApi`, `StoredSession`, `LoginIdentity`), `session_authority.rs` (`SessionAuthority`, `ApiFactory`, `http_api_factory()`, `with_api_factory`, `sign_in_at`).
- Tauri admin (reference only, do not modify for Slint work): `crates/bins/admin-shell/ui/src/pages/{Users,Settings}.tsx`, `settingsHelpers.ts`, `api/query.ts`, `api/command.ts`.
- i18n: canonical `shared/i18n/{en,fa}.json` (flat keys, namespace order enforced). Commands: `node scripts/sync-i18n.mjs --sort-catalogs`, `node scripts/sync-i18n.mjs`, `node scripts/sync-i18n.mjs --check`. Generated outputs are committed.
- Decision history: `docs/migrations/slint/decision-log.md`. Master status: `docs/migrations/slint/MASTER-MIGRATION-RECORD.md`.

### 2.2 Worker contracts relevant to the remaining work

| Topic | Contract (`VERIFIED` from code or tests unless noted) |
|---|---|
| Login | `POST /api/auth/login` body `{username, password, client_type, organization?}`. `username` accepts ONYX ID, username or email (ID compared lowercase). `organization` (ID or name) is checked against the account; mismatch is rejected and audited. Response carries `must_change_password`. |
| Org registry | `GET /api/organizations/lookup?identifier=` (public) → `{server_url: string|null}`; 404 for unknown. `PUT /api/allfather/organizations/:id/server` (All-Father; audited with before/after). Column `organizations.server_url` (migration 0011). |
| Forced password change | While `must_change_password` is set, session routes return `PASSWORD_CHANGE_REQUIRED` (409) except password change and logout. `POST /api/auth/password` body `{current_password, new_password}`; error codes include `CURRENT_PASSWORD_INCORRECT`, `WEAK_PASSWORD`, `PASSWORD_UNCHANGED`; success revokes all sessions for the account. |
| Concurrency | `POST /api/command` envelope (snake_case): `command_id, operation_id, command_type, schema_version "1.0", target{id,type,organization_id}, expected_version, expected_lifecycle_epoch, expected_authority_epoch, issued_at, vector_clock{entries}, correlation_id, causation_id, payload`. On an existing record the Worker returns, in this order: `400 LIFECYCLE_EPOCH_REQUIRED`, `400 AUTHORITY_EPOCH_REQUIRED`, `409 EPOCH_MISMATCH`, `409 VERSION_CONFLICT` (error envelope `error.code`, `category: "CONCURRENCY"`). Status codes are an implementer choice, not owner-approved (see D-12). Route tests exist in `test/security.test.ts`. |
| Policy commands | `command_type`: `policy.PublishPolicyVersion`, `policy.RetirePolicy`, `policy.EvaluatePolicy` (payload `{rule_key}`), `policy.CreatePolicyVersion` (payload `{rules:[{rule_type,key,value}]}`; rule types `FeatureToggle`, `Threshold`, `Authorization`, `Retention`). Target type `policy`. |
| Evaluate result | `result.command_result = {rule_key, rule_found, result, evaluated_version}`. `rule_found=false` ⇒ missing rule; `result=null` with `rule_found=true` ⇒ no value defined. `rule_found` exists only after this Worker version is deployed. |
| Policy queries | `GET /api/query?envelope=<base64url JSON>` with `{query_id, query_type, schema_version "1.0", organization_id, filters, limit, sort_order}`; types `policy.list`, `policy.detail` (filter `target_id`), `legal_hold.detail`. Concurrency values come from the query row (`version`, `lifecycle_epoch`, `authority_epoch`). |
| Policy create | `POST /api/admin/policies` with `Idempotency-Key` header, body `{name}` → `policy_id`. |
| Legal hold | Apply: `POST /api/admin/legal-holds` with `Idempotency-Key`, body `{target_id, target_type, reason}` → `legal_hold_id`. Release: command `ReleaseLegalHold`, target type `legal_hold`, payload `{reason}`. The Tauri admin sends `legal_hold.ReleaseLegalHold`, which the Worker does not alias (see R-13). |
| Audit | `recordAudit` rows for login, failed login (known accounts only), logout, searches, exports, imports; before/after values on class, activate/deactivate, parent change, admin deactivation, org-server change, and generic command audits (prior and merged aggregate state). All-Father-only readers: `GET /api/allfather/audit`, `DELETE /api/allfather/audit/:id` (deletion is itself audited with a snapshot), `GET /api/allfather/users` (read-only cross-org). |
| Import | `POST /api/admin/profiles/import` accepts xlsx/csv/json; every row creates an account with a generated ONYX ID and one-time password; returns an xlsx creation report. Export: `GET /api/admin/profiles/export?format=`. |

### 2.3 Slint app conventions (admin-slint)

- Tokens are memory-only (decision Q-01). A guard test in `auth.rs` scans `auth.rs`/`main.rs` for `std::fs`, `keyring`, `sessionStorage`, `localStorage`, `File::create`. File I/O belongs in `profiles.rs`.
- Network calls: each module exposes async functions taking the token; `main.rs` runs them with `run_org_call(weak, session, lang, action, done)` (tokio current-thread runtime on a worker thread; `done` runs on the UI thread).
- Labels: add keys to `shared/i18n/{en,fa}.json` (both files), run the sync script, then set them in `apply_language` (`main.rs`) and declare them in the `Strings` global (`ui/app.slint`).
- Gate: `cargo +stable clippy -p admin-slint --all-targets -- -D warnings` and `cargo +stable test -p admin-slint` (9 tests as of 2026-11-16).

---

## 3. Remaining-work register

Status values: `TODO`, `BLOCKED` (needs an owner decision or external action), `DONE-UNVERIFIED` (built, never run live). Stage column maps to section 5.

### 3.1 Repository, CI and release hygiene

| ID | Item | Status | Evidence / acceptance | Stage |
|---|---|---|---|---|
| R-01 | Merge PR #474 and get `main` build/test green on Linux and Windows | TODO | Acceptance: required checks green on `main` after merge. Diagnose `check`, Windows tests and `merge-gate` failures on `d2dba18`; only E0063 is known. | S0 |
| R-02 | `rust-advisories` failing on `main` (pre-existing). Seen: `astral-tokio-tar` 0.6.4 (RUSTSEC-2026-0440, unmaintained), `bincode` 2.0.1 (RUSTSEC-2025-0141, unmaintained), `chrono` (id not read). Full list not read. | BLOCKED | Needs security-owner disposition (D-11). Do not weaken policy. Agent may propose upgrades/replacements and an ignore-list proposal with justification. | S0 |
| R-03 | Merge the six Admin Slint branches into `main` | TODO | Rebase on current `main`; resolve conflicts (expect `Cargo.lock`); fmt/clippy/test green; PR per branch or one PR for the tip, per owner preference. | S2 |
| R-04 | Triage 17 open Dependabot PRs; verify package suites on the already-bumped `main` | TODO | Earlier session merged 15+ Dependabot PRs without per-PR CI review, including major bumps (TypeScript 7, Vitest 5, Tailwind 4 in mobile-pwa). Run `web-ui`, `mobile-pwa`, `admin-shell/ui`, `desktop-shell/ui` build/test on `main`. Merge only PRs whose checks pass. | S1 |
| R-05 | Dependabot branches remain on remote (ref deletes blocked with HTTP 403 for the session) | BLOCKED | Owner action: delete in GitHub UI, or enable "Automatically delete head branches". | S1 |
| R-06 | Possible conflict with owner decision 2026-11-08: "Web UI postponed; do not start the deferred Vite dependency work". Some merged Dependabot PRs may touch `web-ui`/Vite. | VERIFY FIRST | List merged PRs touching `web-ui/` and any Vite packages; report to owner (D-15). Do not revert without owner instruction. | S1 |
| R-07 | Pinned Rust toolchain 1.97.1 cannot be downloaded in the agent sandbox; agents use `cargo +stable`. Formatting must match CI. | KNOWN RISK (DEP-01) | Always run `cargo +stable fmt --all --check` before pushing. | all |

### 3.2 Worker / backend

| ID | Item | Status | Acceptance | Stage |
|---|---|---|---|---|
| R-10 | Deploy Worker and apply D1 migrations (pending includes `0011_organization_server_url.sql`) | BLOCKED | Owner must confirm Worker name, D1 database, route, test account, and the deploy route (GitHub workflow chosen by owner on 2026-11-16). Agent must not hold credentials. See S4. | S4 |
| R-11 | Live verification after deploy: lookup route, login by ONYX ID/username/email, org mismatch, forced password change, concurrency codes, `rule_found`, audit rows, Track A relay-ticket checks | BLOCKED by R-10 | Written checklist with response codes for each. | S4 |
| R-12 | Route tests missing for new endpoints and behaviors: `/api/organizations/lookup`, `PUT /api/allfather/organizations/:id/server`, audit list/delete, `/api/allfather/users`, audit rows on login/logout/search/export/import, organization-mismatch login, before/after audit payloads, `rule_found` | TODO | New tests in `deploy/cloudflare/edge-gateway/test/`; suite green. | S3 |
| R-13 | Tauri admin sends `legal_hold.ReleaseLegalHold`; Worker recognizes `ReleaseLegalHold` | VERIFY FIRST | Confirm by reading `settingsHelpers.ts` and the Worker command registry. If confirmed: fix the client or add a Worker alias; add a test. (Owner approval needed only if changing Worker contract surface — see D-17.) | S3 |
| R-14 | Staff profile save (`PUT /api/admin/profiles`) has no before/after audit values; user creation records only created state | TODO | Audit details include `before`/`after` for profile saves. | S3 |
| R-15 | Validate `server_url` in `setOrganizationServer` (scheme `https`/`http` only; no credentials in URL) | VERIFY FIRST | Read the handler; add validation and test if absent. The desktop already accepts only `http(s)://` on its side. | S3 |
| R-16 | Retention/deletion policy for plaintext one-time-password creation reports (STAFF-REPORT-OPEN-01) | BLOCKED | Owner decision D-04. | — |
| R-17 | Organization-admin create DTO mismatch (G0B-12): Worker response vs UI-expected fields | TODO | Source-level comparison; canonical DTO proposal for owner. | S9 |

### 3.3 Admin Slint (feasibility build; all `DONE-UNVERIFIED` unless stated)

| ID | Item | Status | Acceptance | Stage |
|---|---|---|---|---|
| R-20 | Map the four concurrency codes to user-facing messages in policy and legal-hold flows. Today only `Forbidden`/`Failed` are shown. | TODO | Parse `error.code` from the Worker body, never infer from text; messages per C-03; English now, Farsi per D-03. | S5 |
| R-21 | Evaluation display: object/array results should be expandable structured JSON; Farsi wording for true/false (proposed «تأیید شده»/«رد شده») not approved | TODO + D-02 | Show actual value + evaluated version; never show a missing rule as false. Currently one raw line. | S5 |
| R-22 | Pending-rules list is shared across policies; switching policies does not clear it; no way to remove one pending rule; `parse_rule_value` has no unit test | TODO | Clear on policy change (or key by policy id); remove-rule control; unit tests for true/false/int/float/string. | S5 |
| R-23 | Legal hold apply: every click generates a new Idempotency-Key, so a retry after an uncertain failure can create a duplicate; release reason is fixed ("Released by administrator"); holds can only be viewed by id (no list) | TODO + D-16 | Keep the key until success; reason input; list view if a query exists (`VERIFY FIRST`). | S5 |
| R-24 | Tauri-vs-Slint parity audit for AD-03…AD-06 (Users 8 elements, Profiles 17, Policy/Settings 20, Organizations 12 per audit) | TODO | Matrix: every Tauri control mapped to a Slint control or an explicit omission; gaps fixed or recorded. | S6 |
| R-25 | Plan Phase 1/2 checklist never audited against what exists: theme toggle, language switch, password show/hide, loading state, `LOGIN_*` error mapping, access-denied (AD-07), loading (AD-02), role gating (Organizations only for `ALL_FATHER`), logout | VERIFY FIRST | Checklist with pass/fail per item, with test or screenshot evidence; implement the missing ones. | S6 |
| R-26 | Users screen: search, sorting, paging (plan Phase 3) — presence unknown. Table widget limits are open spike U-01. | VERIFY FIRST | As R-25. | S6 |
| R-27 | Q-14 theme/language persistence: approved in direction; schema (fields, defaults, fallback) missing; current behavior unverified | BLOCKED | Needs schema artifact (D-05). | S9 |
| R-28 | Q-16/U-05 Latin-digit helper (8 tests exist) used on every screen and shared with Desktop | TODO | Apply helper to every ID display in Admin Slint; share crate/module for Desktop. | S6 |
| R-29 | Farsi placeholders: up to 10 labels may be English in `fa.json`: `auth.changePasswordTitle`, `auth.changePasswordBody`, `auth.currentPassword`, `auth.newPassword`, `auth.changePasswordSubmit`, `auth.changingPassword`, `common.ruleType`, `settings.addRule`, `settings.createVersion`, `settings.rulesPending` | VERIFY FIRST | List every `fa.json` value identical to its English value; get Persian review (D-01); update. | S9 |
| R-30 | Live click-through of every Admin Slint screen against a deployed Worker | BLOCKED by R-10 | Evidence per screen. | S4/S6 |

### 3.4 Desktop

| ID | Item | Status | Acceptance | Stage |
|---|---|---|---|---|
| R-40 | Slint desktop host login: add Organization field (order: Organization, Staff identifier, Password), org lookup, per-server session | TODO | `desktop-slint` uses `SessionAuthority::with_api_factory` + `sign_in_at`; `composition.rs` fixed origin replaced by the session's server; tests for lookup failure, unknown org, restore on other server. Tauri equivalent already implemented. | S7 |
| R-41 | Slint desktop forced password-change screen | TODO | Shown when `must_change_password`; calls `POST /api/auth/password`; signs out after success; tests. | S7 |
| R-42 | Tauri desktop: ChangePassword and 3-field login never run as a GUI; no "password changed, sign in again" notice; no voluntary change-password entry | DONE-UNVERIFIED | Manual run on a deployed Worker; fix gaps. | S4/S7 |
| R-43 | Desktop foundation (Phase 7: DS-01 startup states + retry, DS-02 shell, DS-03 access denied; 8 navigation items; drawer below 960 px; sync indicator polled every 5 s via `get_sync_status`) | Gate-0b-dependent | See S10. | S10 |
| R-44 | Desktop screens DS-04 Dashboard, DS-11 Settings | Gate-0b-dependent | S11. | S11 |
| R-45 | DS-05 Missions (9 elements), DS-06 Tasks (8) | Gate-0b-dependent | S12. | S12 |
| R-46 | DS-07 Approvals (7) | Gate-0b-dependent | S13. | S13 |
| R-47 | DS-10 Notifications (2) | Gate-0b-dependent + D-07 | S14. | S14 |
| R-48 | DS-09 Files (7; 100 MiB limit) | Gate-0b-dependent | S15. | S15 |
| R-49 | DS-08 Messaging (13; split 13a/13b/13c) | Gate-0b-dependent + D-09 | S16. | S16 |

### 3.5 Governance, Gate 0b and documentation

| ID | Item | Status | Stage |
|---|---|---|---|
| R-60 | Gate 0b blockers G0B-01…G0B-14 (see `MASTER-MIGRATION-RECORD.md` §5): signed Contract Freeze v1; C-01 re-pin to exact source baseline; Q-01/keyring evidence; Q-05 round-trip; Q-14 schema; Q-16 rollout; U-04 parity; Slint spikes U-01/U-02/U-03/U-10/U-12 evidence; C-03 mapping; C-09 fixture provenance; C-12 DTO; license-policy disposition (G0B-13); CI evidence (G0B-14) | BLOCKED (mostly owner/human evidence) | S8, S9 |
| R-61 | Pending spike questions OQ-1…OQ-5: live OS motion-setting updates; Windows reduced-motion fallback; Persian placeholder alignment; focus-ring style; who performs manual device tests and when | BLOCKED (D-10) | S8 |
| R-62 | Missing governing BOUND plan: decision log cites `common/ONYX_Slint_Migration_BOUND_Plan_Execution_Ready.md` (404). A revised plan exists in the Onyx project: `ONYX_Slint_Migration_BOUND_Plan_Revised.md` | TODO + owner approval | S9 |
| R-63 | Master record and decision log not updated for work since 2026-11-10: policies, legal holds, rule editor, Excel import, 3-field login, forced password change, audit trail, concurrency codes, `rule_found`, merge/Dependabot events. Several decisions from 2026-11-14/15/22 exist only in the log. | TODO | S9 |
| R-64 | Keep the Onyx project docs in sync after merges (project doc `claude/admin-slint-migration-plan.md` is dated 2026-10-26 and stale) | TODO | S9 |

### 3.6 Plan phases not started

| ID | Item | Gate / blockers | Stage |
|---|---|---|---|
| R-70 | Phase 14 animation layer (login visuals, transitions; reduce-motion path) | Gate-0b-dependent; U-10 | S17 |
| R-71 | Phase 15 packaging and release replacement (Windows installer, Linux `.deb`/AppImage; release gating on tag push; signing) | U-06, U-07, Q-15 (D-08) | S18 |
| R-72 | Phase 16 removal of Tauri and React UIs | Only after Gate 15 and a full Slint release (owner) | S19 |

---

## 4. Owner decisions register

### 4.1 Decided — do not reopen

| ID | Decision | Source |
|---|---|---|
| Q-01 | Admin tokens are memory-only (no disk, keyring or browser storage) | decision log |
| STAFF-LOGIN | Staff desktop login has 3 inputs: Organization (ID or name), Staff identifier (ONYX ID, username or email), Password. Organization determines the server; mapping lives in the Worker registry maintained by All-Father. Desktop signs in to that server and the saved session records it (owner, 2026-11-14/16) | owner |
| ONYX-ID | ONYX ID is the primary login identifier; username and email stay as aliases; `must_change_password` enforced | decision log O-3/O-4 |
| ALLFATHER | Cross-organization access; read-only below organization-admin level; keeps write rights over organizations and org admins via dedicated endpoints. All-Father must not bypass tenant isolation on generic queries/commands; unknown actions denied; all privileged actions audited (approved 2026-11-22). Policies therefore use the session's own organization | owner |
| AUDIT | Log logins, logouts, failed logins (known accounts), searches, exports, imports; reads/views not logged; kept forever; only All-Father reads or deletes; deletions logged in full; changes carry before/after values | owner |
| EVAL-LABELS | Policy evaluation labels: Evaluation result / نمایش نتیجه ارزیابی; Policy version / نسخه سیاست; Rule not found / قانون یافت نشد; No value defined / مقداری تعریف نشده است. Show actual result plus evaluated version; missing rule never shown as false | owner, 2026-11-15 |
| ERR-CODES | `LIFECYCLE_EPOCH_REQUIRED`, `AUTHORITY_EPOCH_REQUIRED`, `EPOCH_MISMATCH`, `VERSION_CONFLICT`; UI maps codes to text and never infers from error text | owner, 2026-11-15 |
| FA-LABELS | «شناسه کارمند، نام کاربری یا ایمیل» = Staff ID, username or email; «اعمال دستور منع تغییر» = Apply legal hold | owner, 2026-11-22 |
| IMPORT-UI | Import button text "Import from Excel" / «افزودن گروهی از اکسل» | owner |
| TEST-BUILD | No separate test-version packaging; build the main Slint app and test on the way | owner |
| U-04 | Policy lifecycle Active → Retired (terminal); versions Draft → Published (immutable); one pending draft; evaluation requires Active + published | decision log |
| MIG-APPROVAL-01 | "Approved for full migration" (direction only; does not satisfy Gate 0b) | master record |

### 4.2 Pending — owner must decide (agents must not)

| ID | Question | Blocks |
|---|---|---|
| D-01 | Persian review of every English placeholder in `fa.json` (see R-29) | R-29 |
| D-02 | Farsi wording for evaluation true/false («تأیید شده»/«رد شده» proposed in review, not approved) | R-21 |
| D-03 | English and Persian text for the four concurrency-error messages (English proposals: "Required lifecycle version is missing. Refresh and try again." / "Required authority version is missing. Refresh and try again." / "This record has changed. Refresh it before continuing." / "The record was updated by another operation. Review the latest version and retry.") | R-20 |
| D-04 | Retention/deletion policy for plaintext one-time-password reports | R-16 |
| D-05 | Q-14 preference-persistence schema (fields, defaults, fallback, where stored) | R-27 |
| D-06 | Settings: require re-authentication? (Q-07) | S11 |
| D-07 | OS-native notifications: in scope, which events, timing (Q-09/U-09) | S14 |
| D-08 | Windows code signing availability; installer tool; Slint release channel name (U-06, U-07, Q-15) | S18 |
| D-09 | Two-client messaging test environment (U-08/OPEN #10) | S16 |
| D-10 | OQ-1…OQ-5: live motion updates; Windows reduced-motion acceptance; Persian placeholder alignment; focus ring (std-widgets vs custom); named manual testers and dates | S8 |
| D-11 | Security/license-policy disposition for `rust-advisories` and G0B-13 | R-02 |
| D-12 | HTTP statuses for concurrency errors (implemented: 400 for the two `*_REQUIRED` codes, 409 for the others) | R-20 |
| D-13 | Confirm deploy targets (Worker `onyx-framework`, D1 `onyx-free-db`, no custom domain), the deploy route, and provide a dedicated test account | R-10 |
| D-14 | Who signs Gate 0b / Contract Freeze v1, and when | R-60 |
| D-15 | Whether merged Dependabot PRs conflict with the 2026-11-08 web-UI/Vite postponement | R-06 |
| D-16 | Legal-hold apply retry semantics and release-reason input | R-23 |
| D-17 | If R-13 needs a Worker alias for `legal_hold.ReleaseLegalHold`, approve widening the command surface (alternative: fix the client) | R-13 |
| D-18 | Whether Admin Slint may gain a voluntary change-password entry and a "signed out after change" notice on Desktop | R-42 |

---

## 5. Stages (each completable by one agent from one prompt)

Each stage lists: objective, preconditions, deliverable, acceptance, stop conditions, and a paste-ready prompt body (always prepend section 0). Stages may run in parallel only if the "Depends on" column allows.

| Stage | Name | Depends on | Owner input needed |
|---|---|---|---|
| S0 | Repair `main` baseline | — | D-11 for advisories |
| S1 | Dependabot triage + package-suite verification | S0 | D-15, branch cleanup |
| S2 | Merge the Admin Slint branch stack | S0 | — |
| S3 | Worker test hardening and audit completeness | S0 | D-17 (only if alias) |
| S4 | Worker deploy and live verification | S3 | D-13 |
| S5 | Admin Slint completion A (errors, evaluation, rules, legal-hold safety) | S2 | D-02, D-03, D-16 |
| S6 | Admin Slint parity audit and gap fixes | S2 (live parts need S4) | — |
| S7 | Desktop Slint login/session and forced password change | S0 | — |
| S8 | Slint feasibility spikes | — | D-10 (human testers) |
| S9 | Gate 0b evidence package and documentation | S2, S3 | D-01, D-04, D-05, D-14 |
| S10 | Desktop foundation (Phase 7) | Gate 0b signed, S7 | — |
| S11–S16 | Desktop screens | S10 | per screen |
| S17 | Animation layer | Gate 0b, S10 | — |
| S18 | Packaging and release | Gate 15 path | D-08 |
| S19 | Tauri/React removal | Gate 16 | owner |

### S0 — Repair `main` baseline

- **Objective:** make required checks pass on `main`.
- **Preconditions:** read the failing runs on `origin/main` (`merge-gate`, `check`, `Test and build` Linux and Windows, `rust-advisories`).
- **Work:** (1) get PR #474 merged once green; (2) diagnose and fix every other failure unrelated to advisories, one PR per root cause; (3) for `rust-advisories`: read the full report, list every finding with crate, version, advisory ID, whether a maintained alternative or upgrade exists, and propose upgrades or a justified ignore-list. **Do not weaken the policy and do not merge an ignore-list without owner approval (D-11).**
- **Deliverable:** green `main` except advisories (with a written disposition proposal).
- **Acceptance:** all non-advisory required checks green on `main`; report lists each failure, its cause, and the fixing commit.
- **Stop if:** a failure cannot be reproduced or explained.
- **Prompt body:** "Make required CI checks pass on `origin/main`. Start by listing every failing check on the current `main` head and reading its logs. #474 fixes a known compile error (E0063). Diagnose the rest from logs, not guesses. One PR per root cause. For the Rust advisories job, only produce a findings table and a proposal; do not change policy files."

### S1 — Dependabot triage and package verification

- **Objective:** a deliberate, evidence-based state for dependencies.
- **Work:** (1) on current `main`, run install/build/test for `web-ui`, `mobile-pwa`, `admin-shell/ui`, `desktop-shell/ui` (use each package's CI commands; the desktop UI CI build is `tsc -b && vite build`); record results. (2) For each of the 17 open Dependabot PRs: record version jump, risk (major bump?), CI result; merge only green, non-major PRs automatically; list the rest with a recommendation. (3) List merged PRs that touch `web-ui/` or Vite for D-15. (4) Do not delete remote branches (blocked); output the list for the owner.
- **Deliverable:** a report in the PR/issue plus the merges done.
- **Acceptance:** every open Dependabot PR has a recorded disposition; package suites' results on `main` are recorded.
- **Stop if:** a bump breaks a suite and the cause is unclear.
- **Prompt body:** "Triage the open Dependabot PRs and verify the package test suites on `main`. Merge only PRs that are green and are not major version jumps. Do not merge anything that touches `web-ui` or Vite dependencies; the owner postponed that work on 2026-11-08. Output a table: PR, jump, CI result, action taken, recommendation."

### S2 — Merge the Admin Slint branch stack

- **Objective:** bring `feat/admin-users-writes` … `feat/admin-legal-holds` into `main` through reviewed PRs.
- **Preconditions:** S0 done (so CI signal is trustworthy).
- **Work:** create a branch from current `main`; rebase or merge the stack in order, resolving conflicts (expect `Cargo.lock`, `i18n` catalogs; regenerate catalogs with the sync script, do not hand-merge generated files); run Appendix A gates; open PR(s) (one for the tip is acceptable if the owner agrees; otherwise one per branch in order).
- **Acceptance:** `cargo +stable fmt --all --check`, clippy `-D warnings`, `cargo +stable test -p admin-slint` all pass; i18n `--check` passes; CI green.
- **Stop if:** a conflict changes behavior rather than text (e.g. both sides edit the same function logic).
- **Prompt body:** "Merge the Admin Slint feature branches into main in dependency order using PRs. Base on current origin/main. Resolve conflicts by preserving both sides' intent; regenerate generated files instead of editing them. Run fmt, clippy and tests for admin-slint before each push. Report conflicts and how each was resolved."

### S3 — Worker test hardening and audit completeness

- **Objective:** R-12, R-13, R-14, R-15.
- **Work:** write route tests listed in R-12 (use the existing helpers in `test/security.test.ts` and `test/auth.test.ts`); verify R-13 by reading `crates/bins/admin-shell/ui/src/pages/settingsHelpers.ts` and the Worker command registry before changing anything; add before/after audit payloads to profile saves (R-14); verify and add `server_url` validation (R-15).
- **Acceptance:** `npx vitest run` in `deploy/cloudflare/edge-gateway` passes; each new behavior has a test; no change to contracts beyond those stated.
- **Stop if:** R-13 requires widening the Worker command surface — report and wait for D-17.
- **Prompt body:** "Add the missing Worker route tests and audit before/after values listed in the register. Read the code first; do not assume the alias bug in R-13 exists. Do not deploy. Run the full vitest suite."

### S4 — Worker deploy and live verification  *(owner-gated)*

- **Objective:** deploy and verify.
- **Preconditions:** owner answers D-13 in the prompt; an account with deploy rights exists in the agent's environment; a dedicated test account is provided.
- **Work:** confirm repository state and tests; use the GitHub workflow `deploy-cloudflare-edge.yml` (owner's chosen route) — read it first to confirm triggers and inputs; list pending D1 migrations; apply only after the owner-confirmed target; deploy; run the live checklist below; record results in the decision log.
- **Live checklist:** `GET /api/organizations/lookup` (known org → 200 with `server_url`; unknown → 404); login by ONYX ID, username and email; login with wrong organization rejected; account with `must_change_password` gets `PASSWORD_CHANGE_REQUIRED` until `POST /api/auth/password`; command with missing `expected_lifecycle_epoch` → `LIFECYCLE_EPOCH_REQUIRED`; evaluate returns `rule_found`; audit rows appear for login/search/export/import; Track A relay-ticket checks (master record).
- **Stop if:** any migration fails (do not edit migrations), or credentials/targets are unclear.
- **Prompt body:** "Deploy the Worker only after confirming the owner-provided targets below. Do not look for or print credentials. Apply pending D1 migrations first, then deploy, then run the live checklist and record results. [Insert D-13 answers here.]"

### S5 — Admin Slint completion A

- **Objective:** R-20, R-21, R-22, R-23 (feasibility scope).
- **Work:** error-code mapping from `error.code`; expandable structured display for object/array evaluation results; fix pending-rule scoping and add per-rule removal and unit tests; legal-hold retry safety (retain the Idempotency-Key across retries of one user action until success) and release-reason input; add tests for every new pure function.
- **Inputs:** D-02, D-03, D-16 answers. If not provided, use English text only, put Farsi as the English placeholder, and list the labels.
- **Acceptance:** clippy `-D warnings`, `cargo +stable test -p admin-slint`, i18n `--check` pass; new labels in both catalogs.
- **Prompt body:** "Implement items R-20 to R-23 in admin-slint following the conventions in section 2.3. Map concurrency errors by `error.code` only. Never show a missing rule as false. Add unit tests for new logic. Report any label that lacks approved Farsi."

### S6 — Admin Slint parity audit and gap fixes

- **Objective:** R-24, R-25, R-26, R-28.
- **Work:** build a parity matrix from the Tauri screens (`Users.tsx`, `Settings.tsx`, Profiles and Organizations pages) and the plan's Phase 1–6 checklists against the Slint app; fix gaps that are feasible; record the rest. Apply the Latin-digit helper to every ID display.
- **Deliverable:** `docs/migrations/slint/admin-parity-matrix.md` (new) plus fixes.
- **Acceptance:** every control/behavior has status `present`, `fixed`, `omitted (reason)` or `blocked (reason)`; tests for fixes.
- **Stop if:** a gap depends on an unresolved spike (U-01 tables) — record as blocked.
- **Prompt body:** "Produce a control-by-control parity matrix between the Tauri admin screens and the Slint admin app and the plan's Phase 1–6 checklists, then fix the feasible gaps. Do not claim parity without test or screenshot evidence."

### S7 — Desktop Slint login/session and forced password change

- **Objective:** R-40, R-41 (and R-42 code fixes).
- **Work:** replace the two-field login in `crates/bins/desktop-slint` with Organization / Staff identifier / Password; resolve the server through `GET /api/organizations/lookup` (null → cloud base); use `SessionAuthority::with_api_factory(...http_api_factory())` and `sign_in_at`; remove the fixed-origin use in `composition.rs`; keep refresh/revoke/restore on the session's recorded server; add a forced password-change screen (`POST /api/auth/password`; sign out on success); tests: lookup failure, unknown org, wrong org, restore on another server, forced change.
- **Constraints:** `StoredSession.must_change_password` is `#[serde(default)]`; do not break existing stored records; secrets stay in the keyring only.
- **Acceptance:** `cargo +stable test -p desktop-slint -p desktop-session`, clippy, fmt pass; Linux and Windows CI green.
- **Prompt body:** "Bring the Slint desktop host to parity with the Tauri desktop login: three fields, org lookup, per-server session, forced password change. Reuse `desktop-session`; do not duplicate session logic. Add tests listed in the register."

### S8 — Slint feasibility spikes

- **Objective:** produce evidence for U-01 (sortable/paged tables, multiline input, Persian IME, bidi text), U-02 (accessibility: UI Automation / AT-SPI), U-03 (Vazirmatn shaping), U-10 (reduced-motion reading).
- **Work:** build the smallest prototypes in a spike crate; automated parts in CI (Xvfb); write pass/fail with evidence per item; list what humans must verify on real devices (do not claim manual results).
- **Deliverable:** updated `docs/migrations/slint/spike-results.md`.
- **Prompt body:** "Run the Slint feasibility spikes and record pass/fail per item with commit and environment. Mark every item needing a real device or assistive-technology user as 'manual evidence required', do not infer it."

### S9 — Gate 0b evidence package and documentation

- **Objective:** make the gate decidable; reconcile documents.
- **Work:** update `MASTER-MIGRATION-RECORD.md` and `decision-log.md` for everything since 2026-11-10 (dated amendments, no rewriting); prepare drafts for C-01 (route-by-route reconciliation to a pinned commit), C-03 (typed error map), C-09 (fixture schema identity and provenance), Q-14 schema (after D-05), canonical org-admin DTO (R-17); list `fa.json` values equal to English (R-29); propose placing the revised BOUND plan at the path the log cites (R-62); update the Onyx project docs.
- **Rule:** drafts are `PROPOSED`; no approval is implied.
- **Prompt body:** "Prepare the Gate 0b evidence package as drafts and reconcile the migration documents with the repo. Label every unapproved artifact PROPOSED. Do not mark any blocker closed unless its exit evidence exists."

### S10 — Desktop foundation (Phase 7)  *(after Gate 0b)*

- Startup states with retry (`get_current_session`), session reuse from S7, shell with eight navigation items, responsive drawer below 960 px, sync indicator via `get_sync_status` every 5 s tolerating transient failure; stored sessions outside the canonical origin sign out (note: since S7 the origin is per-session; reconcile this rule with the owner-approved per-server behavior and record the change). **First task:** read `crates/bins/desktop-shell/src/lib.rs` and the registries and write the C-02 adapter contract (command/query/event shapes) before any UI.
- **Prompt body:** "Write the Desktop adapter contract from the code, then build the Slint shell. No raw JSON parsing in Slint; typed view-models only."

### S11 — Dashboard (DS-04) and Settings (DS-11)  *(D-06)*
### S12 — Missions (DS-05) and Tasks (DS-06)  *(ID format per Q-16 and the 16-byte-ID open item)*
### S13 — Approvals (DS-07)  *(commands carry expected version and identity/device/organization context; only submitted tasks reviewable)*
### S14 — Notifications (DS-10)  *(D-07; live updates from `subscribe_events`)*
### S15 — Files (DS-09)  *(100 MiB upload limit; native dialogs; test persistence and OS path permissions, which the current app never tested)*
### S16 — Messaging (DS-08) in three PRs  *(16a conversations/members; 16b messages/reactions; 16c connections; needs D-09)*

For S11–S16 each stage prompt is: "Implement screen(s) X per plan Phase N using the S10 adapter contract. Per element: behavior, error states, Farsi/English text, keyboard path. Tests per the plan's list for that screen. Manual checklist items are listed, not claimed."

### S17 — Animation layer
Login visuals and transitions with a reduce-motion path (U-10; unknown setting ⇒ reduced motion per decision U-10). Visual design is specified by the owner later; do not invent it.

### S18 — Packaging and release
Windows installer and Linux packages from the Slint binaries; keep tag-gated publishing; reuse the icon set; document upgrade/uninstall/rollback/recovery. Needs D-08. Tauri stays the default release until the owner approves.

### S19 — Removal of Tauri and React
Only after Gate 15 and one full Slint release. Tag the last Tauri release; remove UI folders, configs, dependencies; update docs and audit.

---

## Appendix A — Commands and known pitfalls

**Admin Slint / Rust**
- `cargo +stable fmt --all --check` (CI rustfmt differs from the sandbox default; run this before every push)
- `cargo +stable clippy -p <crate> --all-targets -- -D warnings`
- `cargo +stable test -p admin-slint` (also `-p desktop-session -p desktop-slint` for session work)
- After Dependabot bumps the offline registry cache is stale: build online (omit `--offline`).
- Builds can exceed 2 minutes; run detached (`setsid nohup … &`) and poll a log. Disk fills during workspace builds: `rm -rf target` frees ~20 GB; build only the crates you touch (`-p`).
- Smoke run: `timeout 8 xvfb-run -a target/debug/admin-slint` (exit 124 = still running).

**Worker:** in `deploy/cloudflare/edge-gateway`: `npx vitest run` (no `tsconfig`; `tsc` is not part of this package's gate).

**Desktop Tauri UI:** in `crates/bins/desktop-shell/ui`: `npm ci`, then `npm run build` (= `tsc -b && vite build`, what CI runs). `tsc --noEmit -p .` does **not** cover the project references and missed a real bug on 2026-11-18.

**Tauri schema files** (`crates/bins/desktop-shell/gen/schemas/*`) are regenerated by builds. Do not commit them: `git checkout -- crates/bins/desktop-shell/gen/schemas`.

**i18n:** change `shared/i18n/en.json` and `fa.json` together; run `--sort-catalogs`, generate, `--check`; commit generated files (including `crates/bins/admin-slint/src/i18n_catalog.rs`).

**GitHub from the sandbox:** `gh api` REST works; GraphQL-based `gh pr list` does not. Deleting remote refs returns 403. Force-push and branch deletion are not available to the session.

**Git hook noise:** a stop-hook repeatedly reports "N unpushed commit(s)" for stacked branches even when the remote is current. Verify with `git ls-remote origin refs/heads/<branch>` against `git rev-parse HEAD` before acting; do not push empty branches to satisfy it.

**Attribution:** follow the attribution rules your own environment provides for commits and PR bodies.

## Appendix B — Source documents

`docs/migrations/slint/MASTER-MIGRATION-RECORD.md`, `docs/migrations/slint/decision-log.md`, `docs/migrations/slint/spike-results.md`, `docs/migrations/slint/backend-accounts-current-state.md`, `docs/production/api-contract.md`, `docs/audits/desktop-ui-ux-forensic-inventory/`; Onyx project docs: `ONYX_Slint_Migration_BOUND_Plan_Revised.md` (the revised plan, phases/briefs/gates), `claude/admin-slint-migration-plan.md` (dated 2026-10-26, stale in places), `ONYX_Slint_BOUND_Implementation_Readiness_Matrix_2026-10-27.md`, `ONYX_Desktop_UI_UX_Forensic_Inventory_Report.md`.
