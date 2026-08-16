# ONYX Backup and Restore

## Recovery contract

- **RTO:** less than one hour.
- **RPO:** less than five minutes.
- PostgreSQL production runs Multi-AZ with automated backups, point-in-time recovery, retained WAL, encrypted snapshots, and a final snapshot on replacement.
- Binary file backups and release evidence are versioned and encrypted in S3.

## Backup schedule

| Asset | Mechanism | Frequency | Retention |
|---|---|---|---|
| PostgreSQL | RDS automated backup and continuous transaction logs | Continuous | 35 days |
| PostgreSQL logical export | `pg_dump --format=custom` | Daily | 35 days, monthly for one year |
| Kubernetes configuration | Git plus encrypted secret backup | Every change | Repository retention |
| Release artifacts/SBOM/provenance | Versioned S3/GitHub release | Every release | Indefinite |
| Audit exports under legal hold | Immutable export | Policy-driven | Legal-hold duration |

## Pre-restore safety

1. Declare SEV-1 and assign a database recovery owner.
2. Stop API, worker, and sync-agent writers or route them to maintenance mode.
3. Record current database endpoint, WAL/LSN, migration status, image digests, and requested recovery timestamp.
4. Snapshot the failed database before altering it.
5. Confirm the recovery timestamp is no more than five minutes before the incident where possible.

## Point-in-time restore

```bash
aws rds restore-db-instance-to-point-in-time \
  --source-db-instance-identifier onyx-production \
  --target-db-instance-identifier onyx-production-recovery \
  --restore-time 2026-08-05T12:00:00Z \
  --use-latest-restorable-time false
```

After the instance becomes available:

1. Attach the ONYX database security group and parameter group.
2. Run `migration-tool status`; never run `down` on production during recovery.
3. Validate row counts for aggregates, domain events, outbox, audit entries, jobs, snapshots, and rate-limit ledger.
4. Verify the audit hash chain for each affected organization.
5. Compare outbox sequence and latest domain-event timestamps against the incident evidence.
6. Point a staging API/worker deployment to the recovered database and run Journeys 1–4.
7. Switch production secrets/endpoints, then restore API at 10% canary traffic.

## Logical restore

```bash
createdb onyx_restore
pg_restore --clean --if-exists --no-owner --dbname=onyx_restore onyx.dump
DATABASE_URL=postgres://... ONYX_DATABASE_KIND=postgres migration-tool up
```

## Post-restore validation

- No open failed migration.
- Audit chain verifies.
- RPO measured and recorded.
- Outbox and job workers drain without duplicate effects.
- Command p95 remains below 500 ms in the release load profile.
- Security signs off before full traffic.

Conduct a restore drill quarterly and retain the measured RTO/RPO in the go-live checklist.
