# Outbox Backlog Recovery

## Detection

Investigate when `outbox_pending` rises continuously, the oldest unpublished row exceeds policy, or published event throughput stops while commands continue.

## Diagnosis

```sql
SELECT count(*) AS pending, min(created_at) AS oldest
FROM outbox WHERE published = false;

SELECT retry_count, count(*)
FROM outbox WHERE published = false GROUP BY retry_count ORDER BY retry_count;

SELECT last_error, count(*)
FROM outbox WHERE published = false GROUP BY last_error ORDER BY count(*) DESC;
```

Check event publisher availability, worker replicas, leases, database locks, rate policy, and dead-letter growth.

## Recovery

1. Preserve rows and record the highest/lowest `outbox_id`.
2. Fix the downstream or credential cause before scaling workers.
3. Clear only expired leases through the supported store operation or by restarting the owning worker after lease expiry.
4. Increase worker replicas one at a time and monitor duplicate suppression, database CPU, and publish latency.
5. Move permanently failing items to dead-letter only after the configured retry policy.
6. Verify Inbox deduplication and consumer projections after drain.

Never delete or mark rows published to make the metric green. Published and dead-letter records are audit evidence.
