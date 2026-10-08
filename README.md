# ONYX — Tectosilicate Coordination

**Governed execution for complex, high-accountability operations.**

> **Operations continue. Accountability remains.**

ONYX connects authority, missions, work, decisions, approvals, evidence, policy, synchronization, and audit into one governed operational record.

## Product

ONYX is designed for environments where execution must be:

- authorized and scope-aware;
- stateful and lifecycle-controlled;
- evidence-backed and auditable;
- policy-aware;
- synchronizable across supported native clients;
- reconstructable after the operation.

### Core pillars

| Pillar | Purpose |
|---|---|
| **Authority** | The right people act with the right scope. |
| **Execution** | Missions, work, timelines, and lifecycle remain structured. |
| **Coordination** | Meetings, decisions, conversations, and actions stay connected. |
| **Evidence** | Files, approvals, verification, reports, and audit preserve what happened. |
| **Resilience** | Supported native clients synchronize distributed operational state and surface conflicts explicitly. |
| **Foresight** | Capacity, forecasting, automation, notifications, and escalation support preparation and response. |

## Architecture

ONYX uses a native application layer with managed infrastructure around it:

1. **Cloudflare Edge** — public ingress, TLS, routing, and edge protection.
2. **ONYX application layer** — Rust/Axum/Tokio API and worker remain authoritative for identity, authorization, domain logic, transactions, jobs, synchronization, policy, and audit.
3. **Managed persistence** — Render PostgreSQL plus Hugging Face Storage Buckets through provider-neutral application ports.

Cloudflare is not the ONYX authorization authority, and the native ONYX runtime is not being rewritten as Workers.

## Current deployment

The current production deployment path is:

**Cloudflare Worker → Render Axum API → Render PostgreSQL + Hugging Face object storage**

- Render API service: `onyx-api-docker`
- Health/readiness endpoint: `/ready`
- PostgreSQL: Render PostgreSQL 16
- Object storage: private Hugging Face Storage Bucket via S3-compatible API
- Production container port: `10000`

The deployment is operationally verified for API readiness and the Cloudflare-to-Render path. The authenticated Hugging Face object PUT/GET/DELETE cycle remains an explicitly pending verification item.

## Client status

- **Web UI:** online thin client; no local ONYX domain replica or offline domain commands.
- **Desktop:** native local replica and synchronization support; disconnected commands are not currently presented as guaranteed queued execution.
- **Android:** native Kotlin/Android is the active Android target.
- **iOS Observer:** specified in governance documentation but not shipped from the current repository snapshot.
- **Flutter:** frozen reference/migration material, not the active Android implementation.

See the application manifest and migration documentation for the authoritative capability boundaries.

## Development and verification

The repository uses:

- Rust unit, integration, property-based, HTTP, E2E, and chaos tests;
- frontend lint/type/build/accessibility/browser checks;
- Android build and instrumentation-oriented checks;
- SQLx offline metadata and migration verification;
- dependency/advisory scanning;
- container security scanning;
- SPDX SBOM generation;
- release provenance and signing workflows.

Run development work in the repository's documented devcontainer/Codespaces environment. See `docs/RUN_LOCALLY.md` for advanced local execution.

## Deployment documentation

- `docs/BACKEND_ARCHITECTURE.md`
- `docs/RENDER_DEPLOYMENT.md`
- `deploy/cloudflare/edge-gateway/`
- `deploy/helm/`
- `deploy/docker/`
- `docs/runbooks/`
- `docs/release/go-live-checklist.md`

## Commercial and legal status

ONYX is proprietary software. See `LICENSE.md` and `LEGAL/` for the current ownership and proprietary-status records.

Third-party dependencies remain governed by their respective licenses. Commercial release still requires completion of the documented licensing, chain-of-title, privacy/DPA, trademark, contract, and release-governance gates.

## Contributions

ONYX is a proprietary project. Do not assume that submitting code, assets, or other material through GitHub transfers ownership or grants redistribution rights. External contributions or commissioned work require appropriate written terms before acceptance into commercial distributions.

## Non-claims

The repository should not be read as evidence of:

- broad AI capability;
- quantitative enterprise-scale performance;
- customer production adoption;
- guaranteed full offline operation across every client;
- regulatory or compliance certification.

Those claims require separate evidence.

## License

ONYX original materials are proprietary. The repository's proprietary notice is in `LICENSE.md`.

Third-party materials retain their applicable licenses.
