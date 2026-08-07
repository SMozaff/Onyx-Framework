# Synchronization Conflict Resolution

## Principle

Authority-controlled state is never resolved by Last-Write-Wins. Concurrent authority-sensitive operations create a `ConflictRecord`, place the object in `ConflictPending`, and block new authority-sensitive commands while reads remain available.

## Reviewer procedure

1. Verify reviewer authority, organization scope, and current AuthorityEpoch.
2. Inspect both operations, actors, devices, vector clocks, policy versions, lifecycle epochs, evidence, and causal context.
3. Confirm neither operation is stale because of lifecycle or authority epoch invalidation.
4. Select `LocalWins`, `RemoteWins`, or a policy-approved custom merge. Use `Escalated` when the decision requires higher authority.
5. Record rationale and supporting evidence. Resolution is a new event; never modify the original operations.
6. Replay/reconcile affected replicas and verify the object exits `ConflictPending`.
7. Confirm audit entry and notifications were emitted.

## Partition handling

If reviewers are unreachable, keep the object visibly pending and let durable escalation proceed through outbox/notification. Do not choose a temporary winner. Emergency communication and audit continue during Operational Halt.

## Bulk conflict events

A sudden increase in `sync_conflicts_open` may indicate stale client schemas, authority-epoch propagation failure, or duplicate device identity. Quarantine the affected replica when necessary and preserve its operation log for investigation.
