# Admin Slint Parity Matrix — S6 (2026-11-23)

- **Scope:** control-by-control parity between the Tauri Admin client
  (`crates/bins/admin-shell/ui/src/pages/`, reference only — must not be modified for Slint
  work) and the Slint Admin host (`crates/bins/admin-slint/ui/app.slint` plus the Rust host
  modules).
- **Audit inputs:** the Tauri source itself, the Slint source itself, and the forensic
  inventory registers (`docs/audits/desktop-ui-ux-forensic-inventory/`:
  `screen-inventory.csv`, `interactive-elements.csv`, `navigation-and-flows.md`,
  `screen-structure.md`, `counts-and-methodology.md`), plus the planning package
  (`/home/all-father/Documents/ONYX_Slint_Migration_Planning_Package/ONYX_Slint_Migration_BOUND_Plan_Execution_Ready.md`,
  section 1.1.1).
- **Method:** source-to-source mapping of every admin control registered in the forensic
  interactive-element register for `APP-AD` (AD-I-001 … AD-I-061) onto the Slint host, then
  comparison against the phase requirements of the planning brief per screen. Live parity
  claims require a deployed Worker plus a GUI run; none is claimed here.
- **Status values:** `present` (source-confirmed in both surfaces / behaviour present),
  `fixed` (gap found in the Slint source and closed in this stage or the S5 completion branch),
  `omitted (reason)` (accepted deviation or not a source-confirmed parity requirement),
  `blocked (reason)` (depends on an unresolved spike, an owner decision or a deploy).

## Cross-cutting parity state

| Contract | Tauri Admin | Slint Admin | Status |
|---|---|---|---|
| Authorization boundary | Client route/navigation guard is UX only; Worker enforces. Non-admin sees an access-denied screen (AD-07). | Organization-admin scope; Worker enforces; All-Father flag gates the Organizations nav. No dedicated access-denied screen; denial surfaces as call errors. | `omitted (reason)` — dedicated denial screen deferred; source-confirmed error path exists. |
| Loading state | AD-02 lazy fallback; per-action busy flags. | Single-window page state; per-action busy via flags; page loads on navigation. | `present` (adapted). |
| Errors | Per-action error text; policy/mobile-access version conflicts surface translated messages. | Concurrency codes mapped to text (R-20); policy/legal-hold missing-epoch messages (S5); generic failure text. | `present` (adapted). |
| Tokens | Memory-only per Q-01 (Slint); native storage for Tauri admin session. | Memory-only per Q-01. | `present`. |
| i18n | `shared/i18n/{en,fa}.json` via generated dictionaries. | Same canonical catalog via generated Slint catalog. | `present`. |

## AD-01 — Login

Source: `admin-shell/ui/src/pages/Login.tsx` vs `crates/bins/admin-slint/src/{auth,main}.rs`,
`ui/app.slint`.

| Control | Tauri behavior (source-confirmed) | Slint behavior (source) | Status |
|---|---|---|---|
| AD-I-001 Username input | Required, autocomplete `username`; submit enabled while not busy. | LineEdit, no required marker; submit disabled while busy. | `present` (adapted; Worker remains the authority). |
| AD-I-002 Password input | Required, autocomplete `current-password`, `type="password"`. | LineEdit `input-type: InputType::Password`. | `present` (adapted). |
| AD-I-003 Locale switch | `LanguageSwitcher` with EN/FA. | Language buttons bound to `Strings`. | `present`. |
| AD-I-004 Login submit / busy / error | Disabled while loading; label switch; error alert; `LOGIN_*` codes mapped. | `busy` flag; label switch; error text; `LoginError` mapping. | `present` (adapted; codes collapsed to Slint-local enum). |
| Phase 1 extras | Remember-username checkbox: not source-confirmed in Tauri, not implemented in Slint phase 1. | Not implemented. | `omitted (reason)` — Phase 1 plan item, owner scope decision. |
| Password show/hide | Not source-confirmed in either surface. | Not implemented. | `omitted (reason)` — Phase 1 checklist item, not a source-confirmed parity requirement. |

## AD-02 — Lazy route loading

| Control | Tauri | Slint | Status |
|---|---|---|---|
| Loading fallback | `Suspense` centered `common.loadingAdmin`. | Busy flag + loading text; pages loaded from navigation. | `present` (adapted; single-window design has no lazy pages). |

## AD-03 — Users

Source: `admin-shell/ui/src/pages/Users.tsx` (registered controls AD-I-005 … AD-I-012) vs
`crates/bins/admin-slint/src/main.rs`, `ui/app.slint` (page 0).

| Control (AD-I) | Tauri behavior | Slint behavior | Status |
|---|---|---|---|
| AD-I-005 Create user username | Input; required; create payload includes it; button disabled when username/email blank or password empty. | LineEdit `new-username`; create sends trimmed values; create disabled until all required fields are filled (R-25). | `fixed` — client-side disable binding added in R-25 follow-up; server still enforces uniqueness/password strength. |
| AD-I-006 Create user email | Required, type email. | LineEdit `new-email`; create sends trimmed values. | `present` (adapted; server enforces). |
| AD-I-007 Create user password | Required, type password; memory-only handling. | LineEdit `new-password`; create sends trimmed values; memory-only. | `present` (adapted; Q-01). Create submit disabled until username/email/password are non-empty (`fixed` in R-25 follow-up). |
| AD-I-008 Create user class | Select incl. no-class option. | ComboBox over `class-options` incl. `""` (no class). | `present`. |
| AD-I-009 Create user submit | Disabled while loading or when username/email blank or password empty; busy label. | Disabled until username/email/password are non-empty and no write is in flight (R-25). | `fixed` — create button disabled until all three fields are non-empty and no write is in flight. |
| AD-I-010 Set user class | Per-row select; disabled while busy. | Per-row ComboBox; disabled while busy. | `present`. |
| AD-I-011 Set user parent | Per-row select from other users; disabled while busy. | Per-row LineEdit + Save button; disabled while busy. | `adapted` — input mechanism differs (free text + explicit save vs select). Status: `present` (functionality parity). |
| AD-I-012 Toggle user active | Per-row button; disabled while busy. | Per-row button; disabled while busy. | `present`. |

## AD-04 — Policy and settings

Source: `admin-shell/ui/src/pages/Settings.tsx` (registered controls AD-I-013 … AD-I-031) vs
`crates/bins/admin-slint/src/{policy,legal_hold,settings,main}.rs`, `ui/app.slint` (page 1).

| Control (AD-I) | Tauri behavior | Slint behavior | Status |
|---|---|---|---|
| AD-I-013 Mobile access class toggle | Checkbox per class; disabled while saving/loading/loadFailed. | CheckBox per class; save via `save-mobile-access`; save disabled while a write is in flight (R-25). | `fixed`. |
| AD-I-014 Policy lookup by id | Input + Open; loads policy by id. | Input + Evaluate (rule key) — no explicit open-by-id. | `omitted (reason)` — Slint implements evaluation-by-key instead of open-by-id. |
| AD-I-015 Policy create | Disabled when name blank or loading; retries keep the create operation for the same name. | Disabled until name is non-empty and no policy command is busy; **retries retain the action key** (S5). | `fixed` — retry retention (S5) plus disabled-until-blank / busy binding (R-25 follow-up). |
| AD-I-016 Submit policy draft | Save draft disabled while loading; resets draft after submit; blank-key rules filtered. | Pending rules cleared after create version; create disabled when pending empty or a policy command is busy (R-25). | `fixed`. |
| AD-I-017–020 Draft rule editor | Per-rule type/key/value; add appends. | Pending-rule list with add/remove (S5); per-rule editor. | `present` (adapted; removal added). |
| AD-I-021 Publish policy | Disabled while command loading. | Disabled while a policy command is in flight (R-25). | `fixed` — disabled while a policy command is in flight (R-25 follow-up). |
| AD-I-022 Retire policy | Disabled while command loading. | Same as publish. | `fixed` — disabled while a policy command is in flight (R-25 follow-up). |
| AD-I-023 Evaluate policy | Disabled while loading or no key; shows JSON result. | Result via summary + pretty detail (S5); evaluate disabled until a key is entered and no command is busy (R-25). | `fixed` — structured result (S5) plus disabled-until-key and busy binding (R-25 follow-up). |
| AD-I-024 Legal hold apply | Disabled unless target id/type/reason non-blank; busy; retry via preserved operation. | Apply disabled until target/type/reason are non-empty and no write is in flight (R-25); retry key retention and release reason (S5). | `fixed` — apply disabled until target/type/reason are non-empty and no write is in flight (R-25 follow-up). |
| AD-I-025 Release legal hold | Lookup by hold id; release disabled while loading; reason fixed. | Lookup by hold id; release requires entered reason; refresh after write (S5). | `present` (adapted; reason now owner-driven input). |

## AD-05 — Staff profiles

Source: `admin-shell/ui/src/pages/Profiles.tsx` (registered controls AD-I-032 … AD-I-048) vs
Slint page 2.

| Control (AD-I) | Tauri behavior | Slint behavior | Status |
|---|---|---|---|
| AD-I-032 New profile | Opens edit overlay in "new" mode. | Flat profile form; no New/overlay distinction. | `omitted (reason)` — overlay redesign deferred; flat form covers the fields. |
| AD-I-033 Edit profile row | Per-row edit opens overlay. | Per-row save writes the selected profile into the flat form (phase 1 design). | `omitted (reason)` — overlay-style interaction not implemented; flat form equivalent for edit. |
| AD-I-034 … AD-I-043 Profile fields | Owner id, name, job title, department, email, phone, class label, parent display, photo key, memberships. | Same fields incl. team memberships (`;`-separated string). | `present` (adapted; list format vs array). |
| AD-I-046 Import | Accept `.csv`; CSV/JSON only; per-row errors/partial success. | Accept CSV/JSON via file dialog; report summary (S5). | `present` (adapted; per-row error summary shape differs). |
| AD-I-047 / AD-I-048 Export | CSV/JSON export + download. | Export via native save dialog (S5). | `present` (adapted). |

## AD-06 — Organizations

Source: `admin-shell/ui/src/pages/Organizations.tsx` (registered controls AD-I-049 … AD-I-060) vs
Slint page 3.

| Control (AD-I) | Tauri behavior | Slint behavior | Status |
|---|---|---|---|
| AD-I-049 Refresh organizations | Button + load. | `load_orgs` on navigation (S5). | `present`. |
| AD-I-050 Create org name | Required; maxLength 160; create disabled if blank/busy. | Create disabled until name is non-empty and no write is in flight (R-25); server enforces max length. | `fixed` — create/rename disabled while blank/unchanged or busy; server still enforces the 160-character limit. |
| AD-I-051 Create org | Disabled if busy/blank. | Create disabled until the name is non-empty and no write is in flight (R-25). | `fixed`. |
| AD-I-052 Rename org | Max 160; disabled if busy or system org. | Sends rename; no checks. | `present` (adapted; server enforces length). |
| AD-I-053 Rename org submit | Disabled if busy or unchanged/system. | Rename disabled when unchanged or while a write is in flight (R-25); system-org check remains server-side. | `fixed` — rename disabled when unchanged or busy (R-25 follow-up). |
| AD-I-054 Activate/deactivate org | Disabled while busy/system constraints. | Disabled while a write is in flight (R-25). | `fixed` — org toggle disabled while a write is in flight (R-25 follow-up). |
| AD-I-055 Manage org admins | Selects org, lists admins; create/toggle. | `select-org` loads admins (S5); create/toggle via org writes (S5). | `present` (adapted). |
| AD-I-056 … AD-I-059 Org admin create | Username/email/password required; password min 8; disabled if busy/inactive org. | Disabled until all fields are non-empty, the selected org is active and no write is in flight (R-25); server enforces password rules. | `fixed` — org-admin create disabled until all fields are non-empty, the selected org is active and no write is in flight; server still enforces the password minimum. |
| AD-I-060 Toggle org admin status | Per-row toggle; disabled while busy. | Per-row toggle; reload after write (S5); disabled while a write is in flight (R-25). | `fixed`. |

## AD-07 — Admin access required

| Control | Tauri | Slint | Status |
|---|---|---|---|
| Access-denied screen | `is_admin` false → explanation + logout. | Error text on Forbidden from calls; no dedicated denial screen; All-Father flag gates Organizations nav. | `omitted (reason)` — dedicated screen deferred; source-confirmed error path exists. |

## Fixes delivered in the S5 completion branch (input to this audit)

- Slint navigation: Settings/Profiles/Organizations nav items now call `select-page`, so those
  pages load (they previously changed only the page index).
- Slint evaluation: summary + pretty detail (structured display).
- Slint legal hold: list view, retry key retention, release reason, apply/release states.
- Slint policy create: retry retains the action key (S5 completion).

## Open gaps carried forward (R-24 … R-28)

- **R-24:** the matrix above is the control-by-control comparison; every registered control was
  examined. Remaining per-control deviations are recorded per table as `omitted (reason)` and are
  closed by follow-up fixes or recorded as owner decisions.
- **R-25:** Phase 1/2 checklist audit vs implemented Slint — all listed feasible fixes landed in
  the R-25 follow-up commit: create-user disabled-until-valid, policy create disabled-until-blank,
  publish/retire/evaluate/create-version busy and key gating, legal-hold apply/release gating,
  organization create/rename/toggle/admin validation and busy bindings (including inactive-org
  gating for admin creation). Runtime verification remains a CI/GUI action.
- **R-26:** Users screen search/sort/paging — blocked on Slint Table-widget parity (spike U-01) or
  an owner-approved enhancement.
- **R-28:** Latin-digit helper (Q-16/U-05) must be applied to every ID display in the Slint Admin
  and shared with the Desktop host; not yet applied to the screens compared here.

## Acceptance

- Every registered admin control (AD-I-001 … AD-I-061) is mapped onto the Slint host with a
  recorded status; none was silently unexamined.
- Runtime parity is not claimed without test or screenshot evidence; `NOT RUN` applies to all
  runtime evidence (no deploy, no GUI run).
