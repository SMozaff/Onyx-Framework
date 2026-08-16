# ONYX Performance Tuning

## Service-level target

The Team 8 release load profile is 100 concurrent users for 60 seconds with command p95 below 500 ms and less than 1% failed requests.

## Diagnose before changing

1. Confirm whether latency is API CPU, PostgreSQL, rate governance, audit append, outbox, or downstream network time.
2. Compare `request_duration_seconds` by route and status.
3. Inspect PostgreSQL `pg_stat_statements`, locks, connection saturation, cache hit rate, WAL pressure, and slow queries.
4. Check `outbox_pending` and `job_queue_depth`; worker starvation can make commands appear stale even when API latency is healthy.
5. Correlate traces by `trace_id` and `operation_id`.

## API

- Keep blocking work out of Axum request tasks.
- Size connection pools below database connection limits; reserve connections for migration and incident access.
- Return projection DTOs rather than loading cross-domain aggregate graphs.
- Do not add optimistic browser mutations or offline command queues.
- Scale horizontally with HPA before raising per-pod CPU limits.

## PostgreSQL

Recommended indexes already cover aggregate tenant/type, event streams, outbox runnable rows, job leases, rate windows, and audit organization sequence. Before adding an index, capture `EXPLAIN (ANALYZE, BUFFERS)` and write a regression test.

Routine checks:

```sql
SELECT wait_event_type, wait_event, count(*) FROM pg_stat_activity GROUP BY 1,2;
SELECT query, calls, mean_exec_time, rows FROM pg_stat_statements ORDER BY mean_exec_time DESC LIMIT 20;
SELECT relname, n_live_tup, n_dead_tup FROM pg_stat_user_tables ORDER BY n_dead_tup DESC;
```

## Worker

- Increase workers only while lease contention and database write I/O remain healthy.
- Keep job leases longer than the 99th percentile execution time.
- Never reduce retry backoff below the Team 7 one-second floor or exceed the 300-second cap without a ruling.
- Snapshotting is hourly and only after more than 1,000 new events; stagger large organizations.

## Load-test procedure

```bash
ONYX_COMMAND_RATE_LIMIT=1000000 cargo run -p api-server
k6 run tests/load/smoke-test.js --vus 100 --duration 60s
```

Record p50/p95/p99, throughput, error rate, CPU, memory, database connections, lock time, and replication lag. A release fails when command p95 is 500 ms or greater.
