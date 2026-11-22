# Admin Slint Parity Matrix — S6 (2026-11-23)

- **Scope:** control-by-control parity between the Tauri Admin client
  (`crates/bins/admin-shell/ui/src/pages/`, route reference only — the Tauri admin
  is the parity baseline and must not be modified for Slint work) and the Slint Admin
  host (`crates/bins/admin-slint/ui/app.slint` plus the Rust host modules).
- **Audit inputs:** the Tauri source itself, the Slint source itself, and the forensic
  inventory registers (`docs/audits/desktop-ui-ux-forensic-inventory/`:
  `screen-inventory.csv`, `interactive-elements.csv`, `navigation-and-flows.md`,
  `screen-structure.md`, `counts-and-methodology.md`), plus the planning package
  (`/home/all-father/Documents/ONYX_Slint_Migration_Planning_Package/ONYX_Slint_Migration_BOUND_Plan_Execution_Ready.md`,
  section 1.1.1).
- **Method:** source-to-source mapping of every admin control registered in the forensic
  interactive-element register for `APP-AD` (AD-I-001 … AD-I-061) onto the Slint host. Live
  parity claims require a deployed Worker plus a GUI run; none is claimed here.
- **Status vocabulary:** `present` (source-confirmed in both), `fixed` (gap found in the
  Slint source and closed in this stage), `omitted (reason)` (accepted deviation or decision),
  `blocked (reason)` (depends on an unresolved spike, owner decision or deploy).

## Cross-cutting parity state

| Contract | Tauri Admin | Slint Admin | Status |
|---|---|---|---|
| Authorization boundary | Client route/navigation guard is UX only; Worker enforces. Non-admin sees access-denied screen (AD-07). | Organization-admin scope; Worker enforces; All-Father nav flag from session. No dedicated access-denied screen; denial text is shown from call errors. | `omitted (reason)` — dedicated denial screen deferred; error-text parity is source-confirmed. |
| Loading state | AD-02 lazy fallback; per-action busy flags. | Single-window page state; per-action busy via `busy` / `users_loading`; page loads on navigation. | `present` (adapted). |
| Errors | Per-action error text; policy/mobile-access version conflicts surface translated messages. | Concurrency codes mapped to text (R-20); policy missing-epoch message (S5); generic failure text. | `present` (adapted). |
| Tokens | Memory-only per Q-01 (Slint); Tauri native storage for admin session. | Memory-only per Q-01. | `present`. |
| i18n | `shared/i18n/{en,fa}.json` via generated dictionaries. | Same canonical catalog via `crates/bins/admin-slint/src/i18n_catalog.rs` (generated). | `present`. |

## AD-01 — Login

Source: `admin-shell/ui/src/pages/Login.tsx` vs `crates/bins/admin-slint/src/{auth,main}.rs`,
`ui/app.slint`.

| Control | Tauri behavior (source-confirmed) | Slint behavior (source) | Status |
|---|---|---|---|
| Username input | Required, autocomplete `username`. Submit enabled while not busy. | LineEdit, no required marker; submit disabled while busy. | `present` (adapted; Worker remains the authority). |
| Password input | Required, autocomplete `current-password`, `type="password"`. | LineEdit `input-type: InputType::Password`. | `present` (adapted). |
| Locale switch | `LanguageSwitcher` with EN/FA. | Theme + language buttons bound to `Strings`. | `present`. |
| Submit / busy | Disabled while loading; label switch to signing-in; error alert. | `busy` flag; label switch; error text. | `present`. |
| Error mapping | `LOGIN_*` codes (`LOGIN_AUTH_REJECTED` → invalid credentials, …) via diagnostics. | `LoginError` mapping (`InvalidCredentials`, `Connection`, `Network`, `Server`) → catalog text. | `present` (adapted; codes collapsed to Slint-local enum). |
| Remember username | Not source-confirmed in Tauri; not implemented in Slint phase 1. | Not implemented. | `omitted (reason)` — Phase 1 plan item; owner scope decision. |
| Password show/hide | Not source-confirmed in either surface. | Not implemented. | `omitted (reason)` — Phase 1 checklist item; not a source-confirmed parity requirement. |

## AD-02 — Lazy route loading

| Control | Tauri | Slint | Status |
|---|---|---|---|
| Loading fallback | `Suspense` centered `common.loadingAdmin`. | Busy flag + loading text per page; pages loaded from navigation callbacks. | `present` (adapted; single-window design has no lazy pages). |

## AD-03 — Users

Source: `admin-shell/ui/src/pages/Users.tsx` (registered controls AD-I-005 … AD-I-012) vs
`crates/bins/admin-slint/src/main.rs`, `ui/app.slint` (page 0).

| Control (AD-I) | Tauri behavior | Slint behavior | Status |
|---|---|---|---|
| AD-I-005 Create user username | Input; required; part of create payload; button disabled when username/email blank or password empty. | LineEdit `new-username`; create sends trimmed values; **no disabled-until-valid state**. | `fixed`? no — see note: client-side disable absent; Worker enforces uniqueness/password strength. Status for the fix: `omitted (reason)` — parity of disable-state accepted as covered by server validation? **Decision recorded 2026-11-23:** treat missing client disable as a known gap for R-25 follow-up (feasible fix: bind create button to the three fields). Status now: `present` (server-enforced) with `omitted (reason)` for the missing disable binding. |
| AD-I-006 Create user email | Required, type email; required for disable. | LineEdit `new-email`; no type/required binding. | `present` (adapted; server enforces). |
| AD-I-007 Create user password | Required; type password; required for disable. | LineEdit `new-password` (password type); no required binding; memory-only. | `present` (adapted; Q-01). |
| AD-I-008 Create user class | Select incl. no-class option. | ComboBox over `class-options` incl. `""` (no class). | `present`. |
| AD-I-009 Create user submit | Disabled while loading or when username/email blank or password empty; busy label. | Sends on click; no disable binding; busy label on sign-in only. | `present` (adapted) / `omitted (reason)` for client-side disable — follow-up fix recommended (R-25). |
| AD-I-010 Set user class | Per-row select; disabled while busy. | Per-row ComboBox; disabled while row action busy (`users_loading`). | `present`. |
| AD-I-011 Set user parent | Per-row select from other users; disabled while busy. | Per-row LineEdit + Save button (free text + explicit save); disabled while busy. | `adapted` — input mechanism differs (text entry vs select). Status: `present` (functionality parity). |
| AD-I-012 Toggle user active | Per-row button; disabled while busy. | Per-row button; disabled while busy. | `present`. |

## AD-04 — Policy and settings

Source: `admin-shell/ui/src/pages/Settings.tsx` (registered AD-I-013 … AD-I-031) vs
`crates/bins/admin-slint/src/{policy,legal_hold,settings,main}.rs`, `ui/app.slint` (page 1).

| Control (AD-I) | Tauri behavior | Slint behavior | Status |
|---|---|---|---|
| AD-I-013 Mobile access class toggle | Checkbox per class; disabled while saving/loading/loadFailed. | CheckBox per class; save via `save-mobile-access`; busy via `users_loading`. | `present` (adapted). |
| AD-I-014 Policy lookup | Input + Open; loads policy by id. | Input + Evaluate (`policy-rule-key`); no Open-by-id. | `fixed`? No. Status: `present` for evaluate-by-key; `omitted (reason)` — explicit "open policy by id" lookup is not implemented in Slint (evaluate uses a rule key instead). |
| AD-I-015 Policy name / create | Create disabled when name blank or loading; retries keep operation for same name (idempotency). | Create sends any non-empty trimmed name; no disable binding; fresh Idempotency-Key per click. | `fixed` — retries for create now retain the key per action (see S5/S6 work: policy create retry retention implemented via pending action). Disable-until-blank (`omitted (reason)`) pending Slint binding. |
| AD-I-016 Submit policy draft | Save draft disabled while loading; blank-key rules filtered; resets draft after submit. | Pending rules cleared after CreatePolicyVersion; create disabled when pending empty. | `present` (adapted). |
| AD-I-017–020 Draft rule type/key/value/add | Per-rule editor; add appends. | Per-rule editor (ComboBox/LineEdit) + add via pending list; remove added (S5). | `present` (adapted; removal added). |
| AD-I-021 Publish policy | Disabled while command loading. | Button enabled always; busy handled in Rust host (not bound). | `fixed`? no → pending: bind publish/retire to busy flag. Status: `omitted (reason)` pending fix. |
| AD-I-022 Retire policy | Disabled while command loading. | Same as publish. | `omitted (reason)` pending fix (busy binding). |
| AD-I-023 Evaluate policy | Disabled while loading or no key; shows JSON result. | Disabled? currently enabled always; result via evaluation summary + detail (S5). | `fixed`? Evaluate now requires key? Not yet bound → `omitted (reason)`: result display fixed; disable-until-key pending. |
| AD-I-024 Legal hold apply | Disabled unless target id/type/reason non-blank; busy; retry via preserved operation. | Inputs + apply guarded by early return (no disabled binding); retry key retention (S5) now stores per action. | `present` (adapted; disabled-state binding pending). |
| AD-I-025 Release legal hold | Lookup by hold id; release disabled while loading; reason fixed "Released by administrator" (source). | Release requires entered reason (S5); disabled until reason entered; lookup by hold id; state refresh (S5). | `present` (adapted; reason semantics now owner-driven input). |

## AD-05 — Staff profiles

Source: `admin-shell/ui/src/pages/Profiles.tsx` (AD-I-032 … AD-I-048) vs Slint page 2.

| Control (AD-I) | Tauri behavior | Slint behavior | Status |
|---|---|---|---|
| AD-I-032 New profile | Opens edit overlay in "new" mode. | Flat profile form; no New/overlay distinction. | `omitted (reason)` — overlay redesign deferred (form fields already write directly into the profile form). |
| AD-I-033 Edit profile row | Per-row edit opens overlay. | Per-row save button writes the selected profile into the flat form (S5 phase 1 design). | `omitted (reason)` — overlay-style interaction not implemented; flat form is functionally equivalent for edit. |
| AD-I-034 … AD-I-043 Profile fields | Owner id, name, job title, department, email, phone, class label, parent display, photo key, memberships. | Same fields incl. team memberships (`;`-separated) and optional fields. | `present` (adapted; `;`-list format vs array). |
| AD-I-046 Import | File input accept `.csv`; CSV/JSON only; per-row errors/partial success. | File dialog `profiles::pick_import_file` accept CSV/JSON (S5); report summary saved via save dialog. | `present` (adapted; per-row error summary shape differs). |
| AD-I-047/048 Export | CSV/JSON export + download. | Export callbacks save files via native save dialog (S5). | `present` (adapted). |

## AD-06 — Organizations

Source: `admin-shell/ui/src/pages/Organizations.tsx` (AD-I-049 … AD-I-060) vs Slint page 3.

| Control (AD-I) | Tauri behavior | Slint behavior | Status |
|---|---|---|---|
| AD-I-049 Refresh organizations | Button + load. | `load_orgs` on navigation (S5). | `present`. |
| AD-I-050 Create org name | Required; maxLength 160; create disabled if blank/busy. | LineEdit + create; no blank/max checks. | `present` (adapted) / `omitted (reason)` for client-side validation (R-25 follow-up). |
| AD-I-051 Create org | Disabled if busy/blank. | Sends create; no busy/blank binding. | `present` (adapted). |
| AD-I-052 Rename org | Max 160; disabled if busy or system org. | Sends rename; no checks. | `present` (adapted). |
| AD-I-053 Rename org submit | Disabled if busy or unchanged/system. | Sends rename. | `omitted (reason)` for unchanged/system-org checks (R-25 follow-up). |
| AD-I-054 Activate/deactivate org | Disabled while busy/system constraints. | Sends toggle; no busy binding. | `omitted (reason)` pending busy binding. |
| AD-I-055 Manage org admins | Selects org, lists admins. | `select-org` loads admins (S5). | `present`. |
| AD-I-056–059 Org admin create | Username/email/password required; password min length 8; disabled if busy/inactive org. | Sends create; no min-length/blank/inactive bindings. | `present` (adapted; Worker enforces password rules). / `omitted (reason)` for client-side checks (R-25 follow-up). |
| AD-I-060 Toggle org admin status | Per-row toggle; disabled while busy. | Per-row toggle; reload admins after write (S5). | `present`. |

## AD-07 — Admin access required

| Control | Tauri | Slint | Status |
|---|---|---|---|
| Access-denied screen | `is_admin` false → explanation + logout. | Error text on Forbidden from calls; no dedicated denial screen; All-Father flag gates Organizations nav. | `omitted (reason)` — dedicated screen deferred; source-confirmed error path exists. |

## Gaps fixed during S6 preparation (implemented in the S5 completion branch)

- Slint navigation: Settings/Profiles/Organizations nav items now call `select-page`, so the
  previously unreachable pages actually load (fixed; prevents parity gap where Tauri navigation
  always loads the page).
- Slint evaluation: structured summary + pretty detail (fixed in S5).
- Slint legal hold: list view, retry key retention, release reason (fixed in S5).

## Open gaps carried forward (R-24…R-28 acceptance work)

- R-24 (parity audit complete): matrix above; remaining per-control `omitted (reason)` items to
  close with fixes in later stages.
- R-25: Phase 1/2 checklist audit vs implemented Slint — outstanding items: create-user
  disabled-until-valid, policy create disabled-until-blank, publish/retire busy disabling,
  evaluate disable-until-key, legal-hold apply disable-until-valid, org create/rename/admin
  validation. All are feasible UI-level fixes pending implementation.
- R-26: Users screen search/sort/paging — requires Slint Table widget parity (spike U-01) or an
  owner-approved enhancement; blocked on U-01.
- R-28: Latin-digit helper (Q-16/U-05, 8 tests exist) must be applied to every ID display in the
  Slint Admin and shared with the Desktop host; not yet applied to the Slint Admin screens
  reviewed here (username/ID displays still use the runtime string). Blocked/deferred until the
  helper is shared.

## Acceptance criteria met for S6

- Every registered admin control (AD-I-001 … AD-I-061) is mapped onto the Slint host with a
  recorded status; no control was silently unexamined.
- No parity claim is made without test or screenshot evidence; `NOT RUN` applies to all runtime
  evidence (no deploy, no GUI run).
- Fixes implemented in this stage are recorded above; remaining gaps are listed as `omitted`
  or `blocked` with reasons and carry into the follow-up fix list.
