import { http, HttpResponse } from 'msw';
import { setupServer } from 'msw/node';
import type { ApprovalProjection, NotificationProjection } from '../../src/types/query';

const organizationId = '11111111-1111-1111-1111-111111111111';
const baseMissions = [{ id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1', name: 'Coastal Response Readiness', summary: 'Coordinate readiness.', status: 'active', owner: 'Operations Lead', priority: 'high', progress: 68, version: 4, lifecycle_epoch: 2, authority_epoch: 1, updated_at: '2026-08-05T11:45:00Z' }];
const baseTasks = [{ id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1', mission_id: baseMissions[0].id, title: 'Validate emergency communications', status: 'active', owner: 'Field Coordinator', priority: 'high', due_at: '2026-08-06T16:00:00Z', version: 5, lifecycle_epoch: 1, authority_epoch: 1, updated_at: '2026-08-05T11:50:00Z' }];
const baseNotifications: NotificationProjection[] = [{ id: 'dddddddd-dddd-4ddd-8ddd-ddddddddddd1', title: 'Critical marker approaching', message: 'Critical marker tomorrow.', priority: 'high', status: 'unacknowledged', source_id: baseMissions[0].id, source_type: 'mission', created_at: '2026-08-05T11:30:00Z', acknowledged_at: null, version: 1, lifecycle_epoch: 0, authority_epoch: 0 }];
const baseApprovals: ApprovalProjection[] = [{ id: 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeee1', title: 'Approve evidence', description: 'Review evidence.', status: 'pending', requested_by: 'Coordinator', target_id: baseTasks[0].id, target_type: 'task', created_at: '2026-08-05T11:35:00Z', decided_at: null, decision_reason: null, web_action_permitted: true, version: 1, lifecycle_epoch: 0, authority_epoch: 0 }];
const baseReports = [{ id: 'ffffffff-ffff-4fff-8fff-fffffffffff1', title: 'Readiness Status Report', status: 'approved', subject_id: baseMissions[0].id, subject_type: 'mission', author: 'Analyst', submitted_at: '2026-08-05T08:30:00Z', summary: 'Ready.', evidence: [{ label: 'Checklist', file_name: 'checklist.pdf' }], version: 2, lifecycle_epoch: 0, authority_epoch: 0, updated_at: '2026-08-05T09:15:00Z' }];

let notifications = structuredClone(baseNotifications);
let approvals = structuredClone(baseApprovals);

export function resetMockData() { notifications = structuredClone(baseNotifications); approvals = structuredClone(baseApprovals); }

function decodeEnvelope(url: URL) {
  const encoded = url.searchParams.get('envelope') ?? '';
  const normalized = encoded.replaceAll('-', '+').replaceAll('_', '/');
  const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, '=');
  return JSON.parse(Buffer.from(padded, 'base64').toString('utf8')) as { query_id: string; query_type: string; filters: Record<string, unknown> };
}

function response(queryId: string, data: unknown[]) {
  return HttpResponse.json({ query_id: queryId, data, has_more: false, next_cursor: null, total_count: data.length, freshness: { projection_version: 5, last_updated_at: '2026-08-05T12:00:00Z', is_stale: false } });
}

export const handlers = [
  http.post('*/api/auth/login', async ({ request }) => {
    const body = await request.json() as { username: string; password: string };
    // Mirrors the real server after audit fix H-01: the `operator`/`onyx`
    // constants are gone, so the mock accepts the same fixture identity the
    // Rust E2E harness bootstraps.
    if (body.username !== 'test-admin' || body.password !== 'test-admin-passphrase') return HttpResponse.json({ error: { code: 'INVALID_CREDENTIALS' } }, { status: 401 });
    return HttpResponse.json({ access_token: 'mock.access.token', refresh_token: 'mock.refresh.token', expires_in: 3600, user: { id: '22222222-2222-2222-2222-222222222222', username: 'test-admin', organization_id: organizationId } });
  }),
  http.post('*/api/auth/logout', () => HttpResponse.json({ success: true })),
  http.get('*/api/query', ({ request }) => {
    const envelope = decodeEnvelope(new URL(request.url));
    const filterId = envelope.filters.id;
    switch (envelope.query_type) {
      case 'mission.list': return response(envelope.query_id, baseMissions);
      case 'mission.detail': return response(envelope.query_id, baseMissions.filter((item) => item.id === filterId));
      case 'task.list': return response(envelope.query_id, baseTasks);
      case 'task.detail': return response(envelope.query_id, baseTasks.filter((item) => item.id === filterId));
      case 'timeline.list': return response(envelope.query_id, []);
      case 'notification.list': return response(envelope.query_id, notifications);
      case 'approval.list': return response(envelope.query_id, approvals);
      case 'report.detail': return response(envelope.query_id, baseReports);
      case 'dashboard.summary': return response(envelope.query_id, [{ missions: 1, active_missions: 1, tasks: 1, blocked_tasks: 0, unread_notifications: notifications.filter((i) => i.status === 'unacknowledged').length, pending_approvals: approvals.filter((i) => i.status === 'pending').length, activity: [] }]);
      default: return response(envelope.query_id, []);
    }
  }),
  http.post('*/api/command', async ({ request }) => {
    const body = await request.json() as { operation_id: string; command_type: string; target: { id: string }; expected_version: number; payload: { reason?: string } };
    if (body.command_type === 'notification.Acknowledge') {
      const item = notifications.find((entry) => entry.id === body.target.id);
      if (!item) return HttpResponse.json({ error: { code: 'NOT_FOUND' } }, { status: 404 });
      if (item.version !== body.expected_version) return HttpResponse.json({ error: { code: 'VERSION_CONFLICT', category: 'CONCURRENCY', retryability: 'RETRYABLE', safe_details: {}, correlation_id: 'test' } }, { status: 409 });
      item.status = 'acknowledged'; item.version += 1;
    }
    if (body.command_type.startsWith('approval.')) {
      const item = approvals.find((entry) => entry.id === body.target.id);
      if (!item) return HttpResponse.json({ error: { code: 'NOT_FOUND' } }, { status: 404 });
      item.status = body.command_type === 'approval.Approve' ? 'approved' : 'rejected'; item.version += 1; item.decision_reason = body.payload.reason ?? null;
    }
    return HttpResponse.json({ success: true, operation_id: body.operation_id, new_version: body.expected_version + 1, events: [] });
  }),
];

export const server = setupServer(...handlers);
