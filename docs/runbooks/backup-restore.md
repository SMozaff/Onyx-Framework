# ONYX Backup and Restore

## Current production topology

Production currently uses Render Postgres 16 for the primary database.

Current verified database posture:

- Render Postgres: `onyx-postgres`
- Region: Frankfurt
- Compute plan: `basic_256mb`
- Storage: 15 GB
- High availability: **disabled**
- Read replicas: **none**
- Disk autoscaling: **disabled**
- Connection pool: **none**
- External IP allow-list: empty; production API uses Render private connectivity.

Do not use the former RDS/AWS recovery commands in this deployment.

## Recovery contract

The historical ONYX recovery target is:

- **RTO:** less than one hour.
- **RPO:** less than five minutes.

These are **targets, not currently demonstrated production guarantees** for the present Render configuration.

The current database has no Render HA standby or read replica. A production release must not claim HA, sub-minute failover, or a measured RPO/RTO until those capabilities are enabled and exercised.

Render paid Postgres provides continuous point-in-time recovery (PITR). The exact available recovery window depends on the Render workspace plan. Render logical exports can also be created from the database Recovery page and downloaded for independent retention.

## Pre-restore safety

1. Declare the incident and assign a database recovery owner.
2. Freeze production schema changes and application deployments.
3. Record the current database identifier, migration status, application image/version, and incident timestamp.
4. Preserve relevant audit and incident evidence.
5. Confirm the desired recovery timestamp and the actual available Render PITR window.
6. Do not run destructive SQL or `migration-tool down` against the production database during recovery.

## Render point-in-time recovery

Use the Render Dashboard database **Recovery** page to create a recovery database at the required point in time.

1. Start a PITR recovery into a new database instance.
2. Validate the recovered database independently.
3. Run migration status checks and ONYX application verification against the recovery instance.
4. Verify aggregates, domain events, outbox/jobs, audit entries, snapshots, and rate-limit state.
5. Verify the audit hash chain.
6. Run the mandatory backend verification journeys.
7. Only after validation, repoint the production service to the recovered database and verify readiness.

The recovery process must preserve the original database until the recovered system has been validated.

## Logical backup

Create periodic logical exports from Render's Recovery page and retain copies outside the Render service according to the applicable commercial/security retention policy.

A logical export is an additional recovery control; it is not a substitute for testing PITR.

## Object-storage recovery

The ONYX BlobStore is provider-neutral. Production object storage is the private Hugging Face bucket.

The production recovery procedure must document:

- bucket and namespace ownership;
- credential rotation procedure;
- object-key/content-addressing strategy;
- export procedure;
- retention/deletion semantics;
- restoration procedure;
- verification of object integrity against ONYX file metadata.

The actual authenticated Hugging Face PUT/GET/DELETE production cycle remains an open verification item.

## Post-restore validation

A restore drill is successful only when:

- migration status is clean;
- no failed migration remains;
- audit chain verifies;
- required domain journeys pass;
- outbox/job workers recover without duplicate effects;
- application readiness is healthy;
- object-storage access is verified;
- measured RTO/RPO are recorded.

Do not mark the go-live checklist green until a real restore drill has produced evidence for the current Render topology.
