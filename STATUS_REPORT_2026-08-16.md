# ONYX — Status Report (2026-08-16)

Supersedes `STATUS_REPORT_2026-08-15.md`, which is now stale — several
items it listed as "designed but not built" were built and live-tested
this session. See `DECISIONS.md` and
`IMPLEMENTATION_PLAN_User_Hierarchy.md` §11 for the full, dated account
this report summarizes.

## Done, verified, tested (as of today)

Everything the 2026-08-15 report listed, plus:

- **Phase A (User Hierarchy data model)** — discovered already
  complete from an earlier session (`UserClass`, `parent_user_id` tree,
  cycle detection, both DB adapters, `require_class`), confirmed via a
  fresh test run (24/24 passing).
- **`todo-domain` crate** — `TodoList`/`TargetList`/`StaffLoan` as full
  `AggregateRoot`s, 42/42 unit tests passing, clean clippy/docs.
- **Full `api-server` HTTP integration** — creation routes, `/api/command`
  dispatch, `/api/query` reads, for all three aggregates. **Live-tested**
  end to end: create → submit → verify → re-query, and staff-loan
  request → approve, against a real running server.
- **D.4, verifier-resolution** — who may verify a given list, combining
  the org tree with active staff loans. **Live-tested both directions**:
  an unrelated user rejected, the real manager accepted.
- **D.5, Team Leader pre-check redaction** — Staff never see a
  pre-check's substance, only that it happened. **Live-tested both
  directions** against a real query response. Found and fixed a real
  bug along the way (pre-check data was being silently discarded before
  it ever reached storage).
- **C.3, the staff-loan background job** — advance-warning and expiry
  scan/notification. Compiles clean, clippy clean, JSON shape confirmed
  against real serialized output. **Not verified against a live
  Postgres** (see the P0 item below — this is the one real gap from
  today's work).

---

## What remains — priority-graded

### 🔴 P0 — Blocking / correctness risk

| Item | Status |
|---|---|
| **C.3 background job SQL never run against real Postgres** | No Postgres was available in the build sandbox and disk space was too tight to install one. The SQL was checked against documentation and the exact JSON shapes it depends on were confirmed via a real test, but the `jsonb_set`/`->>` statements themselves were never executed. **Run this once against a real Postgres instance before relying on it in production.** |
| `is_manager` → `class` backfill not executed | Unchanged from 2026-08-15 — still a real, low-urgency operational gap. Runbook exists: `docs/runbooks/user-class-migration.md`. |

### 🟠 P1 — Explicit product decisions still needed from you

| Item | Blocks |
|---|---|
| **Escalation's scope** — confirmed "almost every authority," but exactly which approval points beyond Todo/Target and staff-loans need it is unconfirmed | Phase E design and, downstream, D.4's escalation-widening (currently a documented gap, not silently stubbed) |
| Admin capability removal from `desktop-shell`/`web-ui` | Deliberately held until confirmed working in practice — unchanged from 2026-08-15 |
| How a newly-provisioned org's first Admin credential reaches the customer (Phase B.4) | Small, non-blocking, still open |

### 🟡 P2 — Designed but not built

| Item | Status |
|---|---|
| **Todo/Target/StaffLoan UI** (Desktop, Admin, or Web) | Backend fully built and live-tested (above); zero UI code. This is now the single largest remaining piece of user-facing work for this feature. |
| **Escalation mechanism** (Phase E) — routing/target-selection | Design confirmed (manual-only, step-by-step, no auto-trigger). `EscalateTodoList`/`EscalateTargetList` commands exist and record that escalation was invoked, but nothing resolves *who* it goes to. |
| Work-stats real data | Unchanged — placeholder until this feature's UI exists to generate real data. |
| Web messaging (Phase 2 of original plan) | Never started, unchanged. |
| Flutter mobile (Phase 3) | Frozen, unchanged. |

### 🟢 P3 — Minor / hygiene

- `admin-shell`'s app icon reuses the main product icon — cosmetic, unchanged.
- `IdempotencyStore` is in-memory only — unchanged, still true.
- The sandbox this work was built in repeatedly ran into disk-space
  limits (as low as 128MB free at one point) during this session,
  requiring `target/` to be cleared entirely at the end to safely
  package the deliverable. Not a defect in the code, but worth knowing
  if resuming work in the same environment — expect to `cargo build`
  from a cold cache next time.

---

## Bottom line

The Todo/Target/StaffLoan backend — the single largest item on
2026-08-15's list — is now built, wired into the real HTTP API, and
live-tested for every major path including authorization and
redaction. The one genuine gap from today's work is narrow and
specific: the background job's raw SQL needs to be run against a real
Postgres once before production use. Everything else is either done or
waiting on a product decision from you, same as before.
