-- ONYX Team 6 deterministic SQLite fixture
-- Organization: 11111111-1111-1111-1111-111111111111
BEGIN;
DELETE FROM domain_events;
DELETE FROM outbox;
DELETE FROM idempotency;
DELETE FROM aggregates;

-- mission: aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1
INSERT INTO aggregates (
  id, aggregate_type, version, lifecycle_epoch, authority_epoch,
  state, updated_at, organization_id
) VALUES (
  X'aaaaaaaaaaaa4aaa8aaaaaaaaaaaaaa1',
  'mission', 4, 2, 1,
  '{"public_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1","name":"Coastal Response Readiness","summary":"Coordinate readiness work across field teams.","status":"active","owner":"Operations Lead","priority":"high","progress":68,"version":4,"lifecycle_epoch":2,"authority_epoch":1,"updated_at":"2026-08-05T11:45:00Z","id":[170,170,170,170,170,170,74,170,138,170,170,170,170,170,170,161]}',
  1785923200000,
  X'11111111111111111111111111111111'
);

-- mission: aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2
INSERT INTO aggregates (
  id, aggregate_type, version, lifecycle_epoch, authority_epoch,
  state, updated_at, organization_id
) VALUES (
  X'aaaaaaaaaaaa4aaa8aaaaaaaaaaaaaa2',
  'mission', 3, 2, 1,
  '{"public_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2","name":"Infrastructure Recovery","summary":"Restore critical service capacity and evidence completion.","status":"paused","owner":"Recovery Manager","priority":"critical","progress":42,"version":3,"lifecycle_epoch":2,"authority_epoch":1,"updated_at":"2026-08-05T10:20:00Z","id":[170,170,170,170,170,170,74,170,138,170,170,170,170,170,170,162]}',
  1785923200000,
  X'11111111111111111111111111111111'
);

-- task: bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1
INSERT INTO aggregates (
  id, aggregate_type, version, lifecycle_epoch, authority_epoch,
  state, updated_at, organization_id
) VALUES (
  X'bbbbbbbbbbbb4bbb8bbbbbbbbbbbbbb1',
  'task', 5, 1, 1,
  '{"public_id":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1","mission_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1","title":"Validate emergency communications","status":"active","owner":"Field Coordinator","priority":"high","due_at":"2026-08-06T16:00:00Z","version":5,"lifecycle_epoch":1,"authority_epoch":1,"updated_at":"2026-08-05T11:50:00Z","id":[187,187,187,187,187,187,75,187,139,187,187,187,187,187,187,177]}',
  1785923200000,
  X'11111111111111111111111111111111'
);

-- task: bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb2
INSERT INTO aggregates (
  id, aggregate_type, version, lifecycle_epoch, authority_epoch,
  state, updated_at, organization_id
) VALUES (
  X'bbbbbbbbbbbb4bbb8bbbbbbbbbbbbbb2',
  'task', 2, 1, 1,
  '{"public_id":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb2","mission_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2","title":"Verify restoration evidence","status":"blocked","owner":"Evidence Reviewer","priority":"critical","due_at":"2026-08-05T18:00:00Z","version":2,"lifecycle_epoch":1,"authority_epoch":1,"updated_at":"2026-08-05T10:35:00Z","id":[187,187,187,187,187,187,75,187,139,187,187,187,187,187,187,178]}',
  1785923200000,
  X'11111111111111111111111111111111'
);

-- timeline: cccccccc-cccc-4ccc-8ccc-ccccccccccc1
INSERT INTO aggregates (
  id, aggregate_type, version, lifecycle_epoch, authority_epoch,
  state, updated_at, organization_id
) VALUES (
  X'cccccccccccc4ccc8cccccccccccccc1',
  'timeline', 1, 0, 0,
  '{"public_id":"cccccccc-cccc-4ccc-8ccc-ccccccccccc1","subject_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1","subject_type":"mission","label":"Readiness checkpoint","kind":"critical_marker","at":"2026-08-06T12:00:00Z","status":"upcoming","version":1,"lifecycle_epoch":0,"authority_epoch":0,"updated_at":"2026-08-05T11:00:00Z","id":[204,204,204,204,204,204,76,204,140,204,204,204,204,204,204,193]}',
  1785923200000,
  X'11111111111111111111111111111111'
);

-- notification: dddddddd-dddd-4ddd-8ddd-ddddddddddd1
INSERT INTO aggregates (
  id, aggregate_type, version, lifecycle_epoch, authority_epoch,
  state, updated_at, organization_id
) VALUES (
  X'dddddddddddd4ddd8dddddddddddddd1',
  'notification', 1, 0, 0,
  '{"public_id":"dddddddd-dddd-4ddd-8ddd-ddddddddddd1","title":"Critical marker approaching","message":"Coastal Response Readiness reaches its critical marker tomorrow.","priority":"high","status":"unacknowledged","source_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1","source_type":"mission","created_at":"2026-08-05T11:30:00Z","acknowledged_at":null,"version":1,"lifecycle_epoch":0,"authority_epoch":0,"id":[221,221,221,221,221,221,77,221,141,221,221,221,221,221,221,209]}',
  1785923200000,
  X'11111111111111111111111111111111'
);

-- notification: dddddddd-dddd-4ddd-8ddd-ddddddddddd2
INSERT INTO aggregates (
  id, aggregate_type, version, lifecycle_epoch, authority_epoch,
  state, updated_at, organization_id
) VALUES (
  X'dddddddddddd4ddd8dddddddddddddd2',
  'notification', 1, 0, 0,
  '{"public_id":"dddddddd-dddd-4ddd-8ddd-ddddddddddd2","title":"Task blocked","message":"Verify restoration evidence is blocked and requires attention.","priority":"critical","status":"unacknowledged","source_id":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb2","source_type":"task","created_at":"2026-08-05T10:40:00Z","acknowledged_at":null,"version":1,"lifecycle_epoch":0,"authority_epoch":0,"id":[221,221,221,221,221,221,77,221,141,221,221,221,221,221,221,210]}',
  1785923200000,
  X'11111111111111111111111111111111'
);

-- approval: eeeeeeee-eeee-4eee-8eee-eeeeeeeeeee1
INSERT INTO aggregates (
  id, aggregate_type, version, lifecycle_epoch, authority_epoch,
  state, updated_at, organization_id
) VALUES (
  X'eeeeeeeeeeee4eee8eeeeeeeeeeeeee1',
  'approval', 1, 0, 0,
  '{"public_id":"eeeeeeee-eeee-4eee-8eee-eeeeeeeeeee1","title":"Approve task completion evidence","description":"Review the submitted evidence package for emergency communications.","status":"pending","requested_by":"Field Coordinator","target_id":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1","target_type":"task","created_at":"2026-08-05T11:35:00Z","decided_at":null,"decision_reason":null,"web_action_permitted":true,"version":1,"lifecycle_epoch":0,"authority_epoch":0,"id":[238,238,238,238,238,238,78,238,142,238,238,238,238,238,238,225]}',
  1785923200000,
  X'11111111111111111111111111111111'
);

-- approval: eeeeeeee-eeee-4eee-8eee-eeeeeeeeeee2
INSERT INTO aggregates (
  id, aggregate_type, version, lifecycle_epoch, authority_epoch,
  state, updated_at, organization_id
) VALUES (
  X'eeeeeeeeeeee4eee8eeeeeeeeeeeeee2',
  'approval', 1, 0, 0,
  '{"public_id":"eeeeeeee-eeee-4eee-8eee-eeeeeeeeeee2","title":"Restricted policy exception","description":"This decision requires a native client and senior authority.","status":"pending","requested_by":"Recovery Manager","target_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2","target_type":"mission","created_at":"2026-08-05T09:00:00Z","decided_at":null,"decision_reason":null,"web_action_permitted":false,"version":1,"lifecycle_epoch":0,"authority_epoch":0,"id":[238,238,238,238,238,238,78,238,142,238,238,238,238,238,238,226]}',
  1785923200000,
  X'11111111111111111111111111111111'
);

-- report: ffffffff-ffff-4fff-8fff-fffffffffff1
INSERT INTO aggregates (
  id, aggregate_type, version, lifecycle_epoch, authority_epoch,
  state, updated_at, organization_id
) VALUES (
  X'ffffffffffff4fff8ffffffffffffff1',
  'report', 2, 0, 0,
  '{"public_id":"ffffffff-ffff-4fff-8fff-fffffffffff1","title":"Readiness Status Report","status":"approved","subject_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1","subject_type":"mission","author":"Operations Analyst","submitted_at":"2026-08-05T08:30:00Z","summary":"Readiness is progressing with one communications dependency under review.","evidence":[{"label":"Field checklist","file_name":"field-checklist.pdf"}],"version":2,"lifecycle_epoch":0,"authority_epoch":0,"updated_at":"2026-08-05T09:15:00Z","id":[255,255,255,255,255,255,79,255,143,255,255,255,255,255,255,241]}',
  1785923200000,
  X'11111111111111111111111111111111'
);

COMMIT;
