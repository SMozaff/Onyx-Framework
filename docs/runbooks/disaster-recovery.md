# ONYX Disaster Recovery

## Scope and targets

This plan covers loss of an availability zone, PostgreSQL primary, Kubernetes cluster, deployment region, or release-signing infrastructure. Production recovery must achieve RTO below one hour and RPO below five minutes.

## Recovery order

1. Identity, signing keys, and secret distribution.
2. PostgreSQL and audit integrity.
3. Migration tool and schema status.
4. API read/command path.
5. Worker outbox/jobs/scheduler/snapshotter.
6. Sync agent and external relay.
7. Web/native clients and non-critical projections.

## Regional recovery

1. Declare SEV-0/SEV-1 and freeze DNS/deployments.
2. Provision the standby region from the pinned Terraform release.
3. Restore PostgreSQL to the latest verified point within the RPO target.
4. Restore encrypted secrets through the approved secret-management process; do not copy plaintext through chat or CI logs.
5. Install Argo Rollouts, ingress, metrics, Jaeger/OpenTelemetry, then deploy migration-tool, worker, sync-agent, and API charts into namespace `onyx`.
6. Verify cosign signatures, GPG signatures, SPDX SBOM, and provenance before running images.
7. Run migration status, audit-chain verification, Journeys 1–4, chaos database-failover smoke, and the 100-user load profile.
8. Move DNS at 10% traffic, observe for 30 minutes, then 50% for 30 minutes, then 100%.

## DR drill

Run at least twice per year. A drill is successful only when:

- Measured RTO is below one hour.
- Measured data loss is below five minutes.
- No audit-chain break exists.
- All mandatory backend journeys pass.
- Canary rollback can be demonstrated.
- Incident and customer communications are exercised.

Store drill evidence and remediation owners with the release sign-off.
