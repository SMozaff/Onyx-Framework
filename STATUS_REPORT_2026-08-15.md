# ONYX — Status Report (2026-08-15)

## Done, verified, tested
Desktop core (Missions/Tasks/Approvals/Messaging/Files/Settings), Web core
(read-only by design), Policy domain, Manager→Class hierarchy migration,
Communication extensions (Supergroup/SubTeam/ConnectionRequest), real file
storage (`BlobStore`), Staff Profiles (view/edit/batch import-export),
**Admin Platform** (separate Tauri app: Users, Policy/Settings, Profiles —
type-checked, backend tested end-to-end, 22/22 `api-server` tests pass).

---

## What remains — priority-graded

### 🔴 P0 — Blocking / correctness risk
| Item | Status |
|---|---|
| ~~`admin-shell` full binary build never completed~~ | **✅ RESOLVED 2026-08-15** — real `cargo build` succeeded after freeing disk space; produces a working 224MB ELF binary, clippy clean. See `DECISIONS.md`. |
| `is_manager` → `class` backfill not executed | Existing managers keep old access but gain nothing new until an Admin manually reclassifies them (runbook exists: `docs/runbooks/user-class-migration.md`). No deadline forces this, but it's a real operational gap sitting in prod-adjacent code. |

### 🟠 P1 — Explicit product decisions still needed from you
| Item | Blocks |
|---|---|
| **Escalation's scope** — confirmed "almost every authority," but exactly which approval points beyond Todo/Target and staff-loans need it is unconfirmed | Phase E design |
| Admin capability removal from `desktop-shell`/`web-ui` | Deliberately held until Admin Platform is confirmed working (P0 above) — do this right after |

### 🟡 P2 — Designed but not built
| Item | Status |
|---|---|
| **`todo-domain` crate** (Phase D) — Todo/Target lists, list-level verification, Team Leader pre-check | Fully designed (state machine, verification rules, all confirmed). Zero code. Biggest remaining build. |
| **Staff loan mechanism** (Phase C) | Fully designed. Zero code. |
| **Escalation mechanism** (Phase E) | Fully designed (manual-only, step-by-step, no auto-trigger). Zero code. Depends on Phase D. |
| **Allfather → narrow provisioning capability** (Phase B, redefined) | Resolved as a one-off org-provisioning action (creates org + first Admin, then stops — no standing cross-org account). Not yet built; small, no longer blocked. |
| Work-stats real data | `WorkStats::unavailable()` by design until Phase D exists — not a bug, a placeholder. |
| Web messaging (Phase 2 of original plan) | Never started — deferred behind Desktop/Admin work by your own sequencing. |
| Flutter mobile (Phase 3) | Frozen, as instructed. |

### 🟢 P3 — Minor / hygiene
- `admin-shell`'s app icon reuses the main product icon (no distinct Admin branding) — cosmetic.
- `IdempotencyStore` is in-memory only, doesn't survive a restart (flagged since Phase 1, still true, low real-world impact so far).

---

## Bottom line
**Nothing is broken or silently incomplete** — every gap above is either
explicitly designed-and-waiting, or explicitly flagged as needing your
decision. The single most consequential next step: **get a real `cargo
build`/`tauri build` of `admin-shell` running somewhere with disk headroom**
(P0) — everything else is sequenced behind that or behind your call on
Allfather.
