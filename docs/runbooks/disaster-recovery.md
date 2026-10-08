# ONYX Disaster Recovery

## Scope

This runbook covers loss or degradation of the current production service, Render PostgreSQL, Cloudflare edge routing, object storage, or release infrastructure.

Current production path:

**Cloudflare Edge → Render Axum API → Render PostgreSQL + Hugging Face object storage**

## Current resilience posture

The present Render PostgreSQL instance is:

- single primary;
- Frankfurt region;
- PostgreSQL 16;
- 15 GB;
- no HA standby;
- no read replicas;
- no disk autoscaling.

Therefore the historical RTO/RPO values below remain **release targets**, not measured guarantees.

## Recovery targets

- **RTO target:** less than one hour.
- **RPO target:** less than five minutes.

These targets require a real recovery drill before they can be represented as achieved.

## Recovery order

1. Confirm incident scope and freeze deployments.
2. Verify Cloudflare Worker configuration and origin.
3. Verify Render service health and deployment state.
4. Protect/restore PostgreSQL state.
5. Verify migration/schema state.
6. Verify ONYX API readiness and authentication.
7. Verify worker/outbox/scheduler behavior.
8. Verify synchronization and client recovery.
9. Verify Hugging Face object access and required object integrity.
10. Restore normal traffic only after application and audit validation.

## PostgreSQL recovery

Use Render's supported PITR/recovery workflow to create a recovery database rather than executing destructive rollback against production.

After recovery:

1. Validate schema/migration status.
2. Verify representative aggregate and domain-event counts.
3. Verify audit-chain integrity.
4. Verify outbox/job state.
5. Run mandatory backend journeys.
6. Point a staging/validation deployment at the recovered database.
7. Measure recovery time and data loss.
8. Repoint production only after sign-off.

Maintain logical exports according to the organization's retention policy so recovery is not dependent on one provider mechanism.

## Cloudflare recovery

The Cloudflare Worker is an edge proxy only.

Recovery verification must confirm:

- Worker deployment is live;
- `ONYX_ORIGIN_URL` points to the intended Render service;
- HTTPS origin is enforced;
- WebSocket 101 pass-through remains intact;
- no authorization/domain logic has been moved into the edge layer.

## Hugging Face recovery

Verify the private bucket, namespace, credentials, endpoint, and object-key semantics.

The recovery procedure must include an authenticated object read/write verification before release.

The production PUT/GET/DELETE cycle is currently not independently proven and therefore remains a go-live verification gap.

## Release recovery

Only deploy an artifact whose:

- commit/tag is identified;
- SBOM is available;
- provenance is available;
- required signatures are verified;
- migration compatibility is confirmed;
- CI/security gates are green.

## DR drill

Run a recovery drill before production launch and at least twice per year thereafter.

A successful drill must produce evidence for:

- measured RTO;
- measured RPO/data loss;
- database recovery;
- audit-chain integrity;
- API readiness;
- object-storage recovery;
- client reconnection;
- incident/customer communications.

Until that evidence exists, the release remains **NO-GO** for claims of proven disaster recovery.
