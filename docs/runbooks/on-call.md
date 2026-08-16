# ONYX On-Call Handbook

## Rotation

One primary and one secondary engineer cover API, worker, synchronization, PostgreSQL, Kubernetes, and release automation. Security retains a separate escalation path. Handover occurs weekly with an explicit review of active incidents, canaries, migration state, outbox/job backlog, open conflicts, expiring certificates, and key-rotation windows.

## Alert priorities

Page immediately for:

- API availability below 99.9% or five-minute error rate above 1%.
- PostgreSQL unavailable, replication lag approaching five minutes, storage exhaustion, or audit-chain verification failure.
- Outbox/job dead-letter growth, no worker progress, or oldest queued item above policy.
- Authority-signature verification anomalies or tenant-boundary denial spikes.
- Canary analysis failure.

Ticket rather than page for low-volume client errors, expected rate limiting, or isolated recoverable sync conflicts.

## Triage commands

```bash
kubectl get rollout,pods,events -n onyx
kubectl argo rollouts get rollout onyx-api -n onyx
kubectl logs -n onyx deploy/onyx-worker --since=30m
kubectl port-forward -n onyx svc/onyx-api-stable 9090:9090
DATABASE_URL=postgres://... ONYX_DATABASE_KIND=postgres migration-tool status
```

## Safe actions

- Abort a failed canary and restore the previous immutable image digest.
- Scale workers gradually while monitoring PostgreSQL locks and I/O.
- Requeue an eligible dead-letter item only after identifying the cause and recording authorization.
- Rotate secrets using the documented current/previous grace convention.

## Prohibited actions

- Truncating outbox, audit, conflict, domain-event, or dead-letter tables.
- Rewriting a published migration.
- Resolving authority conflicts through timestamp or Last-Write-Wins.
- Sharing bearer tokens, private keys, passwords, or unredacted logs.
- Deploying unsigned images or binaries.

Every page requires an incident record, even when automatically recovered.
