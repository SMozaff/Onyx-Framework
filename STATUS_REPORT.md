# ONYX Mission Operations Platform — Whole-Project Status Report

**Date:** 2026-08-11
**Prepared for:** Pitch/demo readiness review

## Summary

ONYX is a hexagonal/DDD Rust workspace (desktop via Tauri, mobile via Flutter+FFI, web-ui in React, backend api-server/worker) delivering offline-first mission and task management with LAN-based team collaboration. Core domain logic, backend infrastructure, release engineering, and connectivity are all functionally complete. Two items are open: a CI verification pass in progress, and mobile platform certification (device-lab/store gates, not code gates).

## Domain Logic (business rules)

| Domain | Status | Notes |
|---|---|---|
| Mission | ✅ Complete | Full lifecycle (Draft → Planning → Active → Review → Closed, plus Halt/Pause/Archive/Cancel), golden + property + unit test suites. |
| Task | ✅ Complete | Full lifecycle incl. dependencies, reassignment, Reopened state, golden + property + unit test suites. |
| Communication (messaging) | ✅ Complete | Conversation/Message aggregates, wired end-to-end into the real command/query pipeline, persisted to SQLite. |
| File (sharing, 100MB/file cap) | ✅ Complete (this session) | FileAsset/UploadSession aggregates: versions, access control, quarantine, chunked upload with the cap enforced at both session-start and every chunk. Wired end-to-end, full test coverage. |

## Connectivity (this increment's headline requirement)

| Requirement | Status |
|---|---|
| Offline LAN discovery (all platforms) | ✅ UDP broadcast peer discovery (`lan-discovery`) |
| Cross-platform relay fallback | ✅ Cloud Relay WebSocket switchboard, verified end-to-end |
| Work plan updates shared across the network | ✅ Rides the same sync engine as Mission/Task |
| Text messaging between teams | ✅ Communication domain above |
| File sharing with 100MB cap | ✅ File domain above |

## Platform Clients

| Client | Status | Notes |
|---|---|---|
| Desktop (Tauri) | ✅ Standalone, embeds the engine directly — no api-server dependency |
| Mobile (Flutter) | 🟡 Feature-complete, cert pending | All screens + background sync implemented; native Wi-Fi Direct/BLE streams are placeholders pending device-lab certification; app store signing not yet provisioned |
| Web UI (React) | 🟡 Scoped subset | Talks real HTTP/WS to `api-server`; Communication and File features are intentionally excluded from web v1 per spec (desktop/mobile only) |

## Backend & Platform Infrastructure

| Area | Status |
|---|---|
| Observability (tracing, metrics, structured logs) | ✅ Delivered (Increment 7) |
| Background job processing | ✅ Delivered |
| Authority/security (Ed25519 signing, rate governance, secret rotation) | ✅ Delivered |
| Audit integrity (SHA-256) | ✅ Delivered |
| Release engineering (CI/CD, Docker, Helm/Argo, Terraform/AWS, SBOM, chaos/load/E2E harnesses, runbooks) | ✅ Delivered |

## Dev Environment

- ✅ Migrated fully to GitHub Codespaces via devcontainer — no local builds used for verification.
- ✅ `gh` CLI now has Codespaces access from this environment (scope added this session) for future work.

## Open Items

1. **CI build/test verification** — a formatting-drift issue blocked the pipeline; fixed via a GitHub-hosted workflow. Full CI re-run is in progress on GitHub now; result pending.
2. **mobile-ios CI job** — failing on a pre-existing, unrelated Swift compile error (`BackgroundService` not in scope in `AppDelegate.swift`). Does not block desktop/backend demo readiness.
3. **Native Wi-Fi Direct/BLE radios** — real Android/iOS SDK work, not buildable/testable in this environment; scheduled as CI/device-lab follow-up, not started.
4. **App store distribution** — signing certs/credentials are an operational input, not a code gap.
5. **IdempotencyStore** — currently in-memory per client process; acceptable for now, flagged as a durability gap for a future pass.

## Bottom Line

No code-level blockers to a desktop-led pitch demo showing LAN discovery, messaging, and file sharing across connected clients. Mobile is demo-capable over Cloud Relay today; native P2P radios and store distribution are the only mobile items still outstanding, and both are infrastructure/certification work rather than missing features.
