# Spec Conformance & Robustness Register

Written 2026-08-10 from a full read of the frozen specification set — Blueprint
v1.6 (Part I architecture, Part II implementation Ch.1/6/7/8/9/10/11), Handover
Document v1.0, Team Prompts 1–8, the Verification Scripts document, and the UI
Documentation package — checked against the code actually in this repository.

Every gap below is a difference between what a **frozen** document requires and
what exists on disk. Nothing here is a style opinion. Where a document permits
something explicitly, it is not listed, and where the code deliberately deviates
with a written rationale, that rationale is quoted rather than overruled.

Severity uses the specs' own language: **BLOCKING** is something a frozen
document marks MUST or names as a release gate; **GAP** is specified but absent;
**RISK** is present but weaker than the guarantee it is supposed to provide.

---

## 1. The constitution has no enforcement

The Verification Scripts document opens by stating its scripts "run on **every
commit** and **every PR** to ensure no team violates the constitution." Ten of
the thirteen do not exist, including the master orchestrator that the other
twelve hang from.

| Script | Enforces | Status |
|---|---|---|
| `verify_contract.sh` | master orchestrator, all phases | **MISSING** |
| `verify_dependencies.sh` | domain crates import no `sqlx`/`tokio`/`reqwest`/… | **MISSING** |
| `verify_trait_completeness.sh` | every aggregate implements all 6 `AggregateRoot` methods | **MISSING** |
| `verify_error_exhaustiveness.sh` | no wildcard `_ =>` in error matches | **MISSING** |
| `verify_ffi_signatures.sh` | `mobile-core` C ABI matches frozen header | **MISSING** |
| `verify_openapi_spec.sh` | API surface matches frozen spec | **MISSING** |
| `verify_serialization.sh` | wire formats unchanged (golden fixtures) | **MISSING** |
| `verify_authority_controlled.sh` | authority fields never reach CRDT merge | **MISSING** |
| `verify_no_secrets.sh` | no `AuthorityProof` in events, no secrets in logs | **MISSING** |
| `verify_structured_logs.sh` | logs carry `trace_id`/`operation_id`/… | **MISSING** |
| `verify_crdt_determinism.sh` | — | present |
| `verify_tombstone_gc.sh` | — | present |
| `verify_migration_idempotency.sh` | — | present |

**Severity: BLOCKING.** Three of the platform's stated safety properties —
"authority-controlled state is never resolved by LWW" (Part I Governing
Principle 4, AD-008), "errors expose no secrets" (§3.4), and the domain layer's
infrastructure-freedom (§1.5) — are currently enforced by nothing but review
attention. `verify_dependencies.sh` and `verify_authority_controlled.sh` are the
two worth writing first: both are short greps, and both guard invariants the
architecture treats as inviolable.

Also absent: `scripts/ci/parse_contracts.py`, and the eight per-increment
workflow files (`.github/workflows/increment1-domain.yml` … `increment8-release.yml`).

---

## 2. The domain layer is 2 of 18 bounded contexts

Part I Chapter 4 specifies eighteen bounded contexts, each with its own
aggregate boundary, commands, events, invariants, synchronization contract, and
authority model. Appendix A's canonical workspace names eight domain crates.

**Present:** `mission-domain`, `work-domain`, `synchronization-domain`.

**Named in Appendix A, absent:** `organization-domain`, `identity-domain`,
`approval-domain`, `timeline-domain`, `audit-policy-domain`.

**Specified in Part I Ch.4, no crate at all:** Context (§4.5), Meeting (§4.6),
Communication (§4.7), File (§4.8), Reporting & Evidence (§4.9), Capacity
(§4.12), Forecasting (§4.13), Automation (§4.14), Notification (§4.15).

Missing application crates: `mission-application`, `work-application` (Appendix A
names both; neither exists — `query-application` and `worker-application` do).

**Severity: GAP, and the largest one by volume.** Everything around the domain
layer is substantially built: all five binaries from Appendix A exist, all six
infrastructure adapters exist, both transports exist, and the `crdt` crate is
complete. The hole is the middle. Note that several *user-visible* features in
the UI documentation depend directly on the absent contexts — the Approval
workflow, Notification centre, and Timeline/critical-marker UI each require a
context that has no crate.

---

## 3. Frozen contracts with no verification artifact

These are the fixtures the specs require in order for conformance to be
*checkable* rather than asserted.

| Artifact | Required by | Status |
|---|---|---|
| `docs/openapi-spec.json` | Verification Scripts §6; Team Prompt 6 §6.1 | **MISSING** |
| `crates/mobile-core/expected_ffi.h` | Verification Scripts §5 | **MISSING** |
| `tests/golden/` fixtures | Team Prompt 1 §8.3; Handover §17 amendment process | **MISSING** |

Without the golden fixtures, Team Prompt 1's stated update procedure ("run
`cargo test --test golden_fixtures -- --ignored`, copy the output to the fixture
file, commit") has nothing to compare against, and any wire-format change ships
silently. Without `expected_ffi.h`, the FFI surface can drift from the frozen
C ABI with no signal — which matters more than usual here, because the delivered
header already deviates from the contract (see §4).

---

## 4. Documented FFI deviations from the frozen C ABI

`crates/mobile-core/mobile-core.h` is honest about these — each is flagged in a
doc comment rather than silently worked around, which is the right call. They
are recorded here because a frozen contract that the implementation cannot
satisfy needs an amendment (Handover §17), not a comment.

- **`mobile_core_ios_background_sync(uint64_t task_id)`** — Team Prompt 5 §3.3's
  signature has *no handle parameter*, so the function cannot reach a
  `MobileApp` instance. Implemented taking an explicit handle. The header notes
  this "is a genuine gap in the frozen contract's own C header, not a
  transcription error," and that inventing a `static` handle to match the
  literal signature would itself be an unrequested architectural choice. The
  Swift caller in Team Prompt 5 §3.5 would need updating.
- **`mobile_core_android_do_work(void* env, void* thiz)`** — same deviation.
  `env`/`thiz` are accepted and ignored; real JNI plumbing (resolving `thiz` to
  a stored native handle) is not implemented.
- **Error reporting** — `mobile_core_execute_command` returns `null` on failure
  with no richer detail, because Team Prompt 5 §3.3 specifies no error mechanism
  beyond "returns JSON string with the result." Every failure is therefore
  indistinguishable at the Dart boundary.

**Severity: RISK.** The third item is the one with product consequence: mobile
cannot show a user *why* a command failed.

---

## 5. Cloud Relay is a stub, and it is load-bearing

`crates/bins/desktop-shell/src/lib.rs` contains `NotYetImplementedSocketFactory`,
where "every call fails immediately rather than silently pretending to succeed."

Cloud Relay is not one transport among four. Per Part II §8.3.1 and §8.4 it is
the **mandatory always-available fallback**, the terminal step of the selection
order (Wi-Fi Direct → BLE → QUIC cross-network → Cloud Relay), and per §8.6 it is
also the **rendezvous point** through which cross-network QUIC peers exchange
reachability candidates. Per Chapter 7 §7.7.1 it is the delivery path that makes
conflict escalation deadlock-free under partition.

With it stubbed:

- multi-device sync does not work at all;
- QUIC cross-network P2P cannot discover peers, so it cannot work either;
- Chapter 7 §7.9's escalation acceptance criterion cannot pass;
- the mobile Settings screen's promise that local-first mode "syncs
  automatically once Cloud Relay support is available" describes an unbuilt
  feature.

**Severity: BLOCKING for any multi-device claim.** Single-device local-first
operation is unaffected.

---

## 6. Sync session resumption is not real resumption

Part II §10.6 requires that a `SynchronizationSession` interrupted by OS
background suspension "persist its partial state … and resume on next foreground
or background-task invocation, never silently abandon a partial reconciliation,"
and §10.8's acceptance criteria require a test proving an interrupted session
reaches the same final state as an uninterrupted one.

`crates/mobile-core/src/ios_background.rs` documents that this is not what
happens: `SynchronizationSession` has no cursor-argument constructor and no
`resume(cursor)`; `pause()`/`resume()` operate on in-memory state that "cannot
survive process death." The function therefore runs a **fresh full sync cycle**
via `trigger_sync_now`.

**Severity: RISK.** Correctness is preserved (a full cycle converges); the cost
is bandwidth and battery on every wake, and Chapter 10's acceptance criterion
cannot honestly be signed off. The header correctly identifies the fix as
`SynchronizationSession` gaining a real resumption API — an Increment 3 change,
which `mobile-core` has no mandate to make unilaterally.

---

## 7. Documentation that contradicts the code

- **`docs/PITCH_DAY_SETUP.md`** states "the desktop shell talks to the same
  `127.0.0.1:3000`." It does not. `desktop-shell` embeds `AppState` with the real
  `CommandRegistry`/`QueryRegistry` and reaches them through seven Tauri
  commands; there is no `reqwest`, no HTTP client, and no reference to port 3000
  anywhere in its source. This is consistent with Part II §9.1.1 ("the desktop
  client embeds the same Rust core … linked into the desktop application
  directly"), so **the code is right and the doc is wrong**. Correct it: starting
  `api-server` is not a prerequisite for demoing desktop.
- **`mobile/android/app/build.gradle`** previously had `ndkVersion` removed on
  the reasoning that the module has no native code. It does:
  `mobile/tool/build_rust_android.sh` cross-compiles `mobile-core` with
  `cargo-ndk` into `src/main/jniLibs` for three ABIs. `jniLibs` is generated and
  gitignored, so its absence in a fresh checkout is not evidence of anything.
  Restored, with the reason recorded in the file.

---

## 8. Robustness refinements worth making

Ordered by value, not effort. None of these are spec violations — they are
places where the implementation is thinner than the guarantee it supports.

1. **Write `verify_dependencies.sh` and `verify_authority_controlled.sh` first.**
   Both are short. Between them they guard AD-008 and the hexagonal boundary —
   the two invariants whose violation would be hardest to detect after the fact
   and most expensive to unwind.
2. **Generate `expected_ffi.h` now, before more drift.** `cbindgen` already
   produces `mobile-core.h`; freezing a copy as the expected header costs one
   command and converts silent ABI drift into a failing check.
3. **Give the FFI a real error channel.** Returning `null` for every failure is
   the single change with the most direct user-visible benefit on mobile.
   Requires a Handover §17 amendment to Team Prompt 5 §3.3, which currently
   specifies no error mechanism at all — so amend the contract rather than
   inventing one under it.
4. **Make CI enforce what it claims.** `.github/workflows/ci.yml` runs
   `cargo clippy -- -D warnings` but not `scripts/verify/verify_contract.sh`. The
   Verification Scripts document's §16 table maps each IFEM phase to a script
   run; none of that wiring exists.
5. **Decide Cloud Relay explicitly.** It is currently a stub with a `TODO`. It is
   either the next increment or an acknowledged v1 scope cut. Left implicit, it
   reads as an oversight in a system whose entire value proposition is
   multi-device local-first sync.
6. **Add the tombstone-GC long-offline-replica test.** Chapter 7 §7.9 names it
   specifically: reconnect a long-offline replica *after* a GC pass and confirm
   its operations still merge rather than resurrecting collected elements. The
   `crdt` crate implements GC correctly (causal completeness, no wall-clock);
   this test is what proves it stays correct.
7. **Web UI bundle budget.** Team Prompt 6 §4.7 and the UI docs set 500 KB
   gzipped. Current initial JS is ~102 KB — comfortable, worth keeping the
   `check-bundle.mjs` gate wired as features land.

---

## 9. What is genuinely complete

Recorded so the gaps above are read in proportion.

- **`crates/synchronization/crdt`** — all six taxonomy types (OR-Set,
  LWW-Register, MV-Register, PN-Counter, RGA, append-only log) plus tombstone GC,
  matching Part I §3.5 and Part II §7.5 exactly, including the requirement that
  GC be gated on causal completeness and never on wall-clock time.
- **All five binaries** from Appendix A: `api-server`, `worker`, `sync-agent`,
  `desktop-shell`, `migration-tool`.
- **All six infrastructure adapters** named in Appendix A, plus
  `persistence-common` and `background-jobs`.
- **Both transport crates**, including `sync-transport-mobile`.
- **Deployment surface**: `deploy/docker`, `deploy/helm`, `deploy/terraform`, and
  `docs/runbooks` — the Team Prompt 8 deliverables.
- **Test surface**: `tests/` carries `property`, `integration`, `chaos`,
  `end-to-end`, `load`, and `fixtures`; `web-ui/tests/` carries `accessibility`,
  `e2e`, `feature-audit`, `integration`, and `unit` — including the
  excluded-features audit Team Prompt 6 §6.2 requires.
