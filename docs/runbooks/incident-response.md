# ONYX Incident Response

## Objectives

Restore safe service inside the **RTO of one hour**, preserve an **RPO below five minutes**, and retain audit evidence throughout the response. Operational Halt never disables security, audit, legal retention, or emergency communication.

## Severity

| Severity | Definition | Initial response |
|---|---|---|
| SEV-0 | Confirmed tenant isolation breach, unrecoverable data corruption, signing-key compromise | 5 minutes |
| SEV-1 | API unavailable, command loss risk, database primary failure, audit-chain failure | 10 minutes |
| SEV-2 | Degraded latency, outbox/job backlog, partial sync failure | 30 minutes |
| SEV-3 | Non-urgent defect with workaround | Next business day |

## First 15 minutes

1. Declare the incident in the incident channel and assign Incident Commander, Operations Lead, Communications Lead, and Scribe.
2. Record UTC start time, affected organizations, deployed image digests, migration version, and current canary weight.
3. Freeze non-essential deployments. Do not delete evidence or restart all replicas simultaneously.
4. Inspect API and worker metrics: `requests_total`, `request_duration_seconds`, `outbox_pending`, `job_queue_depth`, `sync_conflicts_open`, and `audit_entries_total`.
5. Query Kubernetes events and logs using operation, trace, actor, and organization identifiers. Never paste tokens or private keys into the incident channel.
6. If a canary is active and HTTP error rate exceeds 1%, abort it immediately:

```bash
kubectl argo rollouts abort onyx-api -n onyx
kubectl argo rollouts undo onyx-api -n onyx
```

## Containment decisions

- **Authentication/signing compromise:** revoke the exposed key ID, rotate `ONYX_AUTHORITY_SIGNING_KEY`, set a narrowly bounded previous-key grace window only when compromise is not suspected, and invalidate active refresh tokens.
- **Tenant-boundary concern:** deny commands for the affected organization, preserve read-only audit access, and escalate to Security.
- **Database corruption:** stop writers before restore. Preserve the failed volume and transaction logs.
- **Outbox backlog:** follow `outbox-backlog.md`; do not truncate outbox rows.
- **Conflict storm:** follow `conflict-resolution.md`; do not resolve authority conflicts using Last-Write-Wins.

## Evidence collection

Collect and checksum:

- Kubernetes manifests, rollout history, events, and pod descriptions.
- Container image digest, cosign verification output, SPDX SBOM, and build provenance.
- Structured logs for the affected trace/operation IDs.
- PostgreSQL timeline, replication lag, WAL position, slow queries, and locks.
- Audit-chain verification output.
- Relevant conflict, dead-letter, outbox, and job rows.

Store evidence in the encrypted incident bucket under legal-retention policy.

## Recovery and validation

1. Restore the smallest safe component first.
2. Run health checks, migration status, audit integrity verification, Journeys 1–4, and a five-minute command smoke load.
3. Verify replication lag below five minutes and no missing outbox sequence.
4. Resume rollout at 10% only after the Incident Commander and Security Lead agree.
5. Close only after metrics remain healthy for 30 minutes and customer communication is complete.

## Post-incident

Within two business days, produce a blameless report containing timeline, root cause, contributing controls, customer impact, RTO/RPO performance, evidence links, corrective actions, owners, and deadlines.
