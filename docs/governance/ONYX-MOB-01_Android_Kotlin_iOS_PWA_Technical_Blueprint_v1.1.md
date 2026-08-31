# ONYX Android Kotlin + iOS PWA Technical Blueprint v1.1

**Document ID:** ONYX-MOB-01  
**Status:** Execution-Ready Technical Blueprint  
**Version:** 1.1 — Read-Only iOS Observer Revision  
**Authority:** ONYX-MOB-00 v1.1 + canonical ONYX IFEM contracts  
**Scope:** Android Operational Client, iOS Observer Client, backend observer capability boundary, Flutter retirement  
**Date:** 31 August 2026

---

# 1. Objective

This blueprint defines the implementation architecture for:

1. **Native Android Kotlin Operational Client**
2. **iOS Home Screen PWA Observer Client**

The two clients share ONYX identity, read contracts, presentation language, and product semantics.

They do not share execution authority.

Android is an operational endpoint.

The PWA is an awareness endpoint.

---

# 2. Principal Revision from v1.0

Version 1.0 treated the PWA as a reduced mobile client that could eventually perform HTTP mutations such as file upload and other normal operational actions.

Version 1.1 changes that architecture.

The PWA is now **operationally read-only by design**.

This revision changes:

- `client_type` from `mobile` to `mobile_observer`;
- backend authorization from normal mobile permissions to an observer capability ceiling;
- PWA client service from mixed read/write to read-only;
- file behavior from upload/download to download/view only;
- approval behavior from actionable to view-only;
- conflict behavior from actionable to visibility-only or omitted;
- service-worker scope from shell resilience to shell resilience only;
- PWA test strategy to include mandatory negative mutation tests;
- acceptance criteria from “mobile feature parity” to “observer capability completeness.”

---

# 3. Verified Android Baseline

| Property | Baseline |
|---|---|
| Namespace | `com.onyx` |
| Application ID | `com.onyx` |
| Minimum Android API | 29 |
| Java compatibility | 17 |
| Kotlin JVM target | 17 |
| Native delivery | `jniLibs` |
| Native ABIs | `arm64-v8a`, `armeabi-v7a`, `x86_64` |
| Background scheduler | WorkManager |
| Rust core | `mobile-core` |

The Android migration preserves these unless an explicit compatibility decision changes them.

---

# 4. Target Architecture

```text
                                ONYX BACKEND
                    ┌─────────────────────────────┐
                    │ Auth / Policy / Projection  │
                    │ Command / File / Push APIs  │
                    └──────────────┬──────────────┘
                                   │
                  ┌────────────────┴────────────────┐
                  │                                 │
                  ▼                                 ▼
       ANDROID OPERATIONAL CLIENT           iOS OBSERVER CLIENT
       ──────────────────────────           ───────────────────
       Jetpack Compose                      React + TypeScript
       ViewModels/UDF                       Vite
       OperationalClient                    ObserverClient
       HttpGateway                          ObserverHttpGateway
       NativeCoreGateway                    Service Worker
       JNI                                  Web Push
                  │
                  ▼
        mobile-android-jni
                  │
                  ▼
            mobile-core
         SQLite / Files / Sync
```

Backend authorization applies a different capability ceiling to the two client classes.

---

# 5. Repository Layout

```text
/
├── crates/
│   ├── mobile-core/
│   └── mobile-android-jni/
│
├── mobile/                         # existing Flutter, frozen
├── mobile-android/                 # native Kotlin Operational Client
├── mobile-pwa/                     # iOS Observer Client
│
├── docs/mobile-migration/
│   ├── parity-matrix.md
│   ├── observer-capability-matrix.md
│   ├── android-jni-contract.md
│   ├── pwa-observer-contract.md
│   ├── pwa-capability-matrix.md
│   └── flutter-retirement-record.md
│
└── .github/workflows/
    └── ...
```

---

# 6. Client-Type Contract

The authentication contract MUST distinguish client capability classes.

## Android

```json
{
  "client_type": "mobile"
}
```

## iOS PWA

```json
{
  "client_type": "mobile_observer"
}
```

The backend MUST reject unknown client types.

The backend MUST NOT trust arbitrary client-declared capabilities.

It maps the recognized client type to a server-owned capability profile.

---

# 7. Observer Capability Enforcement

The observer model is:

```text
effective_permissions(user, session)
    =
user_permissions(user)
    INTERSECT
observer_capabilities(session.client_type)
```

`mobile_observer` MUST never expand user permissions.

It only narrows them.

A system administrator authenticated through `mobile_observer` still cannot mutate operational state.

---

# 8. Capability Layer

The server SHOULD model client-class capabilities explicitly.

Conceptually:

```text
ClientCapabilities {
    can_read_projections
    can_read_notifications
    can_read_evidence
    can_download_files
    can_submit_domain_commands
    can_approve
    can_transition_lifecycle
    can_resolve_conflicts
    can_upload_files
    can_administer
}
```

For `mobile_observer`:

```text
can_read_projections      = true
can_read_notifications    = true
can_read_evidence         = policy-controlled
can_download_files        = policy-controlled

can_submit_domain_commands = false
can_approve                = false
can_transition_lifecycle   = false
can_resolve_conflicts      = false
can_upload_files           = false
can_administer             = false
```

The exact implementation may be enum/bitset/typed policy object, but the behavior MUST be explicit and testable.

---

# 9. API Enforcement Strategy

Read-only must not rely on frontend routing.

Every operational mutation endpoint MUST reject `mobile_observer`.

This includes, at minimum:

- Mission command endpoints;
- Task command endpoints;
- Approval decisions;
- lifecycle transitions;
- conflict resolution;
- file upload/update/delete;
- organization mutation;
- user mutation;
- policy mutation;
- administrative actions.

Preferred outcome:

`403 Forbidden` with a deterministic observer-capability error code.

Example:

```json
{
  "error": "ClientCapabilityDenied",
  "client_type": "mobile_observer",
  "required_capability": "submit_domain_command"
}
```

Exact field names must align with ONYX error conventions.

---

# 10. Observer-Safe Mutations

The PWA still requires a small control-plane mutation surface.

Allowed exceptions are limited to:

- login;
- refresh;
- logout;
- session revocation;
- push subscription create/update/delete;
- explicitly approved browser preference records.

These routes MUST be classified as **client-control operations**, not domain commands.

They MUST NOT alter Mission, Task, Approval, File-domain, governance, organizational, or policy state.

---

# 11. Android Architecture

Android remains unchanged in principle from v1.0.

```text
Compose Screen
      │
      ▼
ViewModel
      │
      ▼
OperationalClient
      │
      ├─────────────────┐
      ▼                 ▼
NativeCoreGateway    HttpGateway
      │                 │
      ▼                 ▼
JNI/mobile-core      api-server
```

Android remains the full operational client.

---

# 12. JNI Architecture

Add/retain:

`crates/mobile-android-jni`

Responsibilities:

- native library registration;
- JNI marshaling;
- handle ownership;
- callbacks;
- result/error translation.

It MUST NOT contain business logic.

Preferred loading:

`OnyxApplication → System.loadLibrary(...) → JNI_OnLoad/RegisterNatives`

---

# 13. Android OperationalClient Service

Conceptual surface:

```text
OperationalClient
  authenticate
  refreshSession
  logout

  query
  executeCommand

  listMissions
  getMission
  createMission
  updateMission

  listTasks
  getTask
  createTask
  updateTask

  approve
  reject

  uploadFile
  downloadFile

  listConflicts
  resolveConflict
  triggerSync
  getSyncStatus
```

Actual domain-specific methods may differ, but Android may expose the complete operational contract.

---

# 14. iOS ObserverClient Service

The PWA uses a separate client service interface.

It MUST be read-oriented by construction.

Conceptual surface:

```text
ObserverClient
  authenticate(...)
  refreshSession(...)
  logout(...)

  getDashboard(...)
  listMissions(...)
  getMission(...)

  listTasks(...)
  getTask(...)

  listApprovalRequirements(...)
  getApprovalRequirement(...)

  listNotifications(...)
  markLocalNotificationPresentationState(...)  # if purely client-local

  getEvidence(...)
  getAuditView(...)
  getHierarchyView(...)

  getFileMetadata(...)
  downloadFile(...)

  registerPushSubscription(...)
  unregisterPushSubscription(...)
```

It MUST NOT define operational methods such as:

```text
executeCommand
createMission
updateMission
createTask
updateTask
approve
reject
transitionLifecycle
resolveConflict
uploadFile
deleteFile
mutateUser
mutateOrganization
mutatePolicy
```

This compile-time/service-interface narrowing is defense in depth.

The backend remains authoritative.

---

# 15. ObserverHttpGateway

The PWA HTTP gateway SHOULD expose only:

- authentication/session endpoints;
- GET/read projection endpoints;
- authorized file download;
- push subscription management.

It SHOULD NOT contain generic helpers such as:

```text
postCommand(...)
executeArbitraryMutation(...)
```

unless those helpers are structurally restricted to approved client-control routes.

Avoid a generic “API client that can call anything” when the product contract is intentionally read-only.

---

# 16. Read Projection API

The PWA MAY reuse existing read endpoints if they already provide appropriate authorization and data minimization.

If existing APIs expose mixed read/write semantics, introduce read-specific routes or adapters rather than exposing mutation capability accidentally.

The Observer Client SHOULD be able to read:

- Dashboard projection;
- Mission list/detail;
- Task list/detail;
- approval requirement/status;
- notifications;
- selected hierarchy information;
- evidence/audit data when authorized;
- file metadata;
- sync/status summaries if useful and safe.

---

# 17. Approval UX

The PWA MAY show:

- that approval is required;
- who/what class of authority is required, if policy permits;
- current approval status;
- supporting evidence.

It MUST NOT show functional Approve/Reject actions.

Recommended presentation:

> **Action required in an ONYX Operational Client.**

Deep-linking to another trusted client MAY be added later if a secure cross-client mechanism is approved.

---

# 18. Conflict UX

The PWA MUST NOT resolve conflicts.

It MAY:

- show that a conflict exists;
- show non-sensitive conflict metadata;
- show that operational intervention is required.

If conflict details themselves contain sensitive or confusing implementation data, omit them from the Observer Client.

---

# 19. File Model

## Android

Full ONYX File-domain behavior remains available subject to authorization.

## PWA

Read-only:

- list permitted file metadata;
- view permitted file metadata;
- download permitted file content.

Forbidden:

- upload;
- replace;
- delete;
- release from quarantine;
- mutate metadata;
- change file access state.

The PWA needs only a provider-neutral download/read contract.

A PWA upload API is removed from this migration plan.

---

# 20. PWA Service Worker

The service worker owns:

- app-shell precache;
- static asset caching;
- offline informational fallback;
- cache cleanup;
- safe update lifecycle;
- Web Push receipt/display.

It does NOT own:

- ONYX domain data;
- operational command queues;
- sync;
- authority;
- file content cache;
- approval actions.

---

# 21. Offline PWA Behavior

When offline:

- the shell opens;
- cached static UI remains available;
- the user sees explicit offline status;
- authoritative operational data is not presented as current;
- no domain action can be taken;
- no command queue exists.

Optional previously rendered in-memory data must be labeled stale.

---

# 22. Web Push

Push is informational.

Flow:

```text
Home Screen PWA
   ↓
feature detection
   ↓
user enables notifications
   ↓
PushSubscription created
   ↓
subscription registered with ONYX
   ↓
server sends informational Web Push
   ↓
notification click opens Observer Client
   ↓
Observer Client fetches current authorized state
```

Push payloads SHOULD minimize sensitive data.

No silent/data-only background synchronization is assumed.

---

# 23. Security Architecture

The PWA threat model prioritizes:

- XSS;
- session theft;
- service-worker compromise;
- dependency/supply-chain compromise;
- unauthorized read access;
- over-broad file downloads;
- accidental capability expansion.

Required controls:

- strict CSP;
- no uncontrolled HTML injection;
- HTTPS;
- secure session handling;
- server-enforced `mobile_observer`;
- object-level read authorization;
- tenant isolation;
- file authorization;
- service-worker scope discipline;
- minimal third-party script execution.

Read-only reduces integrity blast radius but not confidentiality exposure.

---

# 24. Session Architecture

The observer session SHOULD be identifiable server-side as an observer session for its full lifetime.

A refresh operation MUST NOT accidentally convert it into an ordinary mobile session.

The client type/capability ceiling MUST survive:

- refresh;
- token rotation;
- reconnect;
- push-subscription updates.

Logout/revocation applies normally.

---

# 25. Android Work Packages

## A0 — Freeze Flutter Android Behavior

Inventory Android-equivalent functionality.

## A1 — Kotlin Skeleton

- `mobile-android`
- Compose
- navigation
- minSdk 29
- CI

## A2 — JNI

- `mobile-android-jni`
- native registration
- adapter tests

## A3 — Startup/Auth

- login
- secure session
- hierarchy
- local startup state machine

## A4 — Core Screens

- Dashboard
- Missions
- Tasks
- Details
- Notifications

## A5 — Operational Screens

- Approvals
- Files
- Sync
- Conflicts
- Settings
- background work

## A6 — Android Acceptance

- real hardware
- local/offline
- accessibility
- release artifact

---

# 26. PWA Work Packages

## P0 — Freeze Observer Capability Contract

Produce:

- `mobile_observer` capability matrix;
- read endpoint inventory;
- forbidden mutation inventory;
- client-control exception list.

**Gate:** no implementation team guesses what “read-only” means.

## P1 — Backend Capability Enforcement

**Implementation note (H10):** prior to this work, `client_type` was an
unchecked `Option<String>` compared against exactly one hardcoded literal
(`"mobile"`); no code path anywhere rejected an unrecognized value or
denied a mutation on the basis of client class. P1 below was not a
pre-existing capability being documented — it is the phased breakdown of
building it for the first time, landed as
`crates/bins/api-server/src/routes/client_type.rs`.

### P1.1 — Closed client-type enum

Replace the loose, uncomparable `client_type` string with a closed,
server-owned `ClientType` enum (`Mobile`, `MobileObserver`, `Desktop`,
`Admin`, `Web`) deriving `serde::Deserialize`. A plain string-valued enum
already rejects any value outside this set (`unknown_variant`) — this
alone satisfies "the backend MUST reject unknown client types" (§6) with
no hand-written validation. An *absent* `client_type` remains a distinct,
explicitly-handled case (`ClientType::default_on_absence`), not a
rejection, preserving this project's existing back-compat contract for
callers that predate this field.

### P1.2 — Server-owned capability mapping

A static `ClientCapabilities` struct, matching §8's field list exactly,
with two profiles: `FULL_CAPABILITIES` (every other client class) and
`OBSERVER_CAPABILITIES` (`mobile_observer`: every `can_read_*` and
`can_download_files` true, every mutation-capable flag false). Mapped via
a `const fn capabilities_for(ClientType) -> ClientCapabilities` — the
"enum/bitset/typed policy object" choice §8 explicitly leaves open.

### P1.3 — Mutation denial at every operational endpoint

`require_capability` checks the authenticated session's mapped
capability and denies before any domain-specific authority check runs
its own logic, wired into every endpoint class §9 enumerates (mission and
task command endpoints, approval decisions, lifecycle transitions,
conflict resolution, file upload, organization/user/policy mutation, and
administrative actions). This is additive to, never a replacement for,
this project's existing per-route authority checks (`require_admin`,
ownership checks, etc.) — a user who already fails their existing
authority check is still denied by that check first.

### P1.4 — Deterministic capability error

`CLIENT_CAPABILITY_DENIED`, `403 Forbidden`, carrying `client_type` and
`required_capability` inside this project's real `ApiError`/
`safe_details` envelope — §9's illustrative flat JSON shape is explicitly
non-binding ("Exact field names must align with ONYX error conventions");
this project's real convention is the `code`/`category`/`retryability`/
`correlation_id` shape already established by prior hardening work, not
the flat example verbatim.

### P1.5 — Negative-test proof

`crates/bins/api-server/tests/mobile_observer_capability.rs`, against a
real bound server and real authenticated sessions: a `mobile_observer`
session is confirmed denied on a representative mutation endpoint while
its reads continue to succeed unchanged, an unrelated authority check
(cross-tenant) is confirmed to still fire independent of client
capability, and a session refresh is confirmed to preserve the
capability ceiling rather than resetting it to full access.

**Gate:** direct API negative tests prove forbidden mutations fail.

## P2 — PWA Foundation

Build:

- `mobile-pwa`;
- React/TypeScript;
- Vite;
- manifest;
- service worker;
- responsive navigation;
- authentication using `mobile_observer`.

## P3 — Observer Client Service

Implement:

- ObserverClient;
- ObserverHttpGateway;
- read projections;
- read-only Mission/Task/approval/notification views.

## P4 — Files + Evidence

Implement:

- file metadata read;
- authorized download;
- evidence/audit views;
- no upload route in PWA service.

## P5 — Web Push

Implement:

- install guidance;
- capability detection;
- subscription registration;
- informational notification handling.

## P6 — iPhone Acceptance

Prove:

- Home Screen install;
- standalone launch;
- authentication;
- read projections;
- mutation denials;
- file download;
- push;
- accessibility.

---

# 27. CI — Observer Client

Required:

```text
npm ci
lint
type-check
unit tests
accessibility tests
production build
browser E2E
service-worker tests
manifest validation
```

Security-specific E2E/integration tests MUST include:

- valid observer login succeeds;
- read Mission succeeds when authorized;
- read Mission fails when unauthorized;
- cross-tenant read fails;
- Mission create fails;
- Mission update fails;
- Task create/update fails;
- approve/reject fails;
- lifecycle transition fails;
- conflict resolution fails;
- file upload fails;
- user/org/policy mutation fails;
- admin action fails;
- file download obeys read authorization;
- session refresh retains observer capability ceiling.

---

# 28. Negative Capability Test Matrix

| Operation | Expected with valid `mobile_observer` session |
|---|---|
| GET permitted Mission | 200 |
| GET forbidden Mission | 403/404 per policy |
| GET permitted Task | 200 |
| GET permitted file | 200 |
| POST Mission command | 403 |
| POST Task command | 403 |
| POST approval | 403 |
| POST lifecycle transition | 403 |
| POST conflict resolution | 403 |
| POST file upload | 403 |
| DELETE file | 403 |
| User mutation | 403 |
| Org mutation | 403 |
| Policy mutation | 403 |
| Admin command | 403 |

This matrix is a release gate.

---

# 29. API Design Guidance

Where feasible, ObserverClient should use read-specific DTOs.

Do not automatically send every internal field merely because the full operational client can see it.

The observer projection layer SHOULD minimize:

- internal synchronization metadata;
- internal authority implementation details;
- secrets/tokens;
- infrastructure identifiers;
- fields irrelevant to situational awareness.

Read-only is strengthened by data minimization.

---

# 30. Flutter Retirement

Flutter retirement depends on **Android Kotlin parity**, not PWA parity.

The PWA is a new Observer product surface with intentionally narrower scope.

Flutter may be removed only when:

- Kotlin Android is accepted;
- Android operational workflows pass;
- JNI passes on device;
- files/sync/background work pass as required;
- rollback artifact exists.

PWA completion is independently gated by observer acceptance.

---

# 31. Definition of Done — Android

Android is complete when:

- Kotlin is canonical;
- Rust remains local behavioral authority;
- JNI works on real devices;
- local/offline behavior is preserved;
- operational workflows pass;
- accessibility/security gates pass;
- Flutter is no longer needed.

---

# 32. Definition of Done — iOS Observer PWA

The PWA is complete when:

- it installs to Home Screen;
- it launches standalone;
- `client_type="mobile_observer"` is used;
- observer capability ceiling is server-enforced;
- required read views work;
- prohibited mutations fail at backend;
- authorized file download works;
- file upload is absent/denied;
- Web Push works where supported;
- offline shell behaves honestly;
- accessibility/security gates pass.

---

# 33. Revision Decision Ledger

| ID | Decision |
|---|---|
| MBP-001 | Android remains Operational Client |
| MBP-002 | iOS PWA is Observer Client |
| MBP-003 | PWA uses `mobile_observer` |
| MBP-004 | Observer capability ceiling enforced server-side |
| MBP-005 | ObserverClient service exposes read APIs only |
| MBP-006 | PWA has no domain command surface |
| MBP-007 | PWA approvals are view-only |
| MBP-008 | PWA conflicts are non-actionable |
| MBP-009 | PWA files are download/view only |
| MBP-010 | Push is informational |
| MBP-011 | Session/push operations are allowed control-plane mutations |
| MBP-012 | Observer refresh preserves client class |
| MBP-013 | Negative mutation tests are release gates |
| MBP-014 | Flutter retirement depends on Android parity |

---

# 34. Final Architecture Statement

The resulting mobile strategy is:

> **ONYX Android — Operational Client**  
> Local-first, authority-aware, command-capable, Rust-backed execution endpoint.

> **ONYX iOS PWA — Observer Client**  
> Authenticated, read-only, server-authorized awareness endpoint for status, notifications, evidence and permitted file viewing.

This separation is intentional.

It reduces PWA integrity risk, simplifies the browser client, preserves ONYX authority boundaries, and avoids pretending that two fundamentally different runtime environments must possess identical execution capabilities.

**Blueprint Status: EXECUTION READY.**
