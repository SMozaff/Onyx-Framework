# ONYX — Status Report (2026-08-16, final)

Supersedes all earlier same-day versions. This update closes out the
remaining items from the prior report: Staff Loans UI, the two
smaller Todo/Target UI gaps, the two open product decisions, and a
real authorization bug found along the way.

## Done, verified, tested (as of this update)

Everything the prior 2026-08-16 reports listed, plus:

- **Staff Loans UI** (`web-ui/src/pages/StaffLoans/`) — request,
  approve, decline, extend, end. Role-appropriate actions per loan,
  matching design doc §2.1's three approval gates exactly. Verified:
  `tsc -b` clean, production build succeeds, all 130 relevant tests
  pass.
- **A real, pre-existing authorization gap found and fixed**:
  `RecordTeamLeaderPreCheck` had no server-side class check at all —
  any authenticated user could record a pre-check on any list, despite
  `UserClass::TeamLeader` being the class design doc §2.2 says carries
  this authority. Fixed with a new `require_team_leader_or_admin` gate,
  live-verified against a real server (rejected as an unclassed user,
  accepted after promotion to `team_leader`), and covered by 3 new
  permanent integration tests.
- **Two remaining Todo/Target UI gaps closed**: adding an item to a
  Draft `TodoList`, and recording a Team Leader pre-check from the UI
  (now gated by the fix above).
- **Two outstanding product decisions resolved, by the person
  directly**: first-Admin credentials will be delivered via invite
  email with a setup link (not a temp password); admin-screen removal
  from `desktop-shell`/`web-ui` is confirmed and done.
- **Duplicate admin UI removed**: `desktop-shell`'s `Settings.tsx`
  (Policy/LegalHold administration using a placeholder, unauthenticated
  session) deleted, along with its route and nav entry. `web-ui` had no
  equivalent to remove. `desktop-shell`'s UI re-verified clean
  (`tsc -b`, production build) after the removal.
- **Full fresh re-verification of everything**, from a clean install/
  build cache: `todo-domain` (42 tests), `security-adapter`/
  `security-application` (24 tests), `api-server` (25 tests, up from
  22 — the 3 new authorization tests), `worker` (compiles clean),
  `web-ui` (`tsc -b` clean, production build succeeds, 130 tests pass).

---

## What remains — priority-graded

### 🔴 P0 — Blocking / correctness risk

*(none — the only P0 this feature ever had, the C.3 Postgres
verification, was closed earlier this session)*

The general `is_manager` → `class` backfill remains a real, low-urgency
operational gap unrelated to this feature (runbook exists:
`docs/runbooks/user-class-migration.md`).

### 🟠 P1 — Explicit product decisions still needed from you

| Item | Blocks |
|---|---|
| **Escalation's scope** — confirmed "almost every authority," but exactly which approval points beyond Todo/Target and staff-loans need it is unconfirmed | Phase E design and, downstream, D.4's escalation-widening (currently a documented gap, not silently stubbed) |

Both other P1 items from earlier reports (admin-screen removal timing,
first-Admin credential delivery) are now resolved — see above.

### 🟡 P2 — Designed but not built

| Item | Status |
|---|---|
| **`ManagerAssigned` list/loan creation** (assigning to someone else) | Needs a user-id picker neither `TodoTargets/CreateListForm.tsx` nor `StaffLoans/CreateLoanForm.tsx` has yet — both currently take raw UUID input for other parties, or self-only creation. A natural, contained follow-up. |
| **Escalation mechanism** (Phase E) — routing/target-selection | Design confirmed. Commands exist to record that escalation was invoked; nothing resolves *who* it goes to. |
| Phase B.1–B.2 (the org-provisioning action itself) | Design resolved (§2.4, §3 of the implementation plan); no code built. B.4 (this update) specifies what the last step should do once B.1–B.2 exist. |
| Work-stats real data | Unchanged — placeholder until this feature generates real data at scale. |
| Web messaging (Phase 2 of original plan) | Never started, unchanged. |
| Flutter mobile (Phase 3) | Frozen, unchanged. |

### 🟢 P3 — Minor / hygiene / infrastructure gaps

- `admin-shell`'s app icon reuses the main product icon — cosmetic, unchanged.
- `IdempotencyStore` is in-memory only — unchanged.
- **No ESLint configuration exists anywhere in this repository
  checkout** — a pre-existing gap, unrelated to and not fixed by this
  work, discovered while verifying the UI.
- ~~Workspace-wide Docker-backed E2E suite could not be run~~ —
  **resolved 2026-08-17**. Confirmed not a fixed limitation: asked
  directly, Manus installed a local Docker Engine, worked around one
  sandbox-specific daemon config issue with no test-code changes, and
  ran the real suite — 4/4 runnable journeys pass, 3 ignored for their
  own declared reasons (other teams' client/mobile work, unrelated to
  this feature). See `DECISIONS.md` for the full per-journey result.
- Disk-space constraints required aggressive `target/`/`node_modules`
  cleanup repeatedly across this feature's work — not a code defect,
  expect a cold build cache if resuming in a similarly constrained
  environment.

---

## Bottom line

Todo/Target lists and Staff Loans now have complete, live-verified
backends and working UIs in `web-ui`. A real, previously-undetected
authorization gap (anyone could record a Team Leader pre-check) was
found and closed with a permanent regression test, not left for later.
Both outstanding product decisions from earlier reports are resolved
and the duplicate admin UI is removed. What's left is either a small,
contained UI follow-up (a user picker for assigning lists/loans to
someone else) or genuinely new work outside this feature's original
scope (Phase E's escalation routing, Phase B's provisioning action).

