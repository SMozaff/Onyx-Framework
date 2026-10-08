import { describe, expect, it } from 'vitest';

const base = process.env.ONYX_E2E_BASE;
const suite = base ? describe : describe.skip;
const organizationId = '11111111-1111-1111-1111-111111111111';

interface AuthResponse {
  access_token: string;
  refresh_token: string;
  user: { organization_id: string };
}

// Credentials come from the environment. The hardcoded `operator`/`onyx` pair
// this suite used was removed by audit fix H-01 — those constants no longer
// exist server-side, so a hardcoded login here would now always 401.
//
// Against a freshly-provisioned server the user will not exist yet, so
// `login()` first attempts the one-time bootstrap endpoint. That call is
// expected to fail with 409 on an already-provisioned server, which is not an
// error and is ignored.
const username = process.env.ONYX_E2E_USERNAME ?? 'test-admin';
const password = process.env.ONYX_E2E_PASSWORD ?? 'test-admin-passphrase';
const bootstrapToken = process.env.ONYX_E2E_BOOTSTRAP_TOKEN;

async function ensureUserExists(): Promise<void> {
  if (!bootstrapToken) return;
  const response = await fetch(`${base}/api/admin/bootstrap`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'x-onyx-bootstrap-token': bootstrapToken,
    },
    body: JSON.stringify({ username, password }),
  });
  // 201 = we just created the first admin.
  // 409 = a user already exists; bootstrap is closed. Both are fine.
  if (response.status !== 201 && response.status !== 409) {
    throw new Error(`bootstrap failed with status ${response.status}`);
  }
}

async function login(): Promise<AuthResponse> {
  await ensureUserExists();
  const response = await fetch(`${base}/api/auth/login`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ username, password }),
  });
  expect(response.status).toBe(200);
  return response.json() as Promise<AuthResponse>;
}

function encode(value: unknown): string {
  return Buffer.from(JSON.stringify(value)).toString('base64url');
}

function queryEnvelope(queryType: string, organization_id = organizationId) {
  return {
    query_id: crypto.randomUUID(),
    query_type: queryType,
    schema_version: '1.0',
    organization_id,
    filters: {},
  };
}

function commandEnvelope(input: {
  command_type: 'notification.Acknowledge' | 'approval.Approve' | 'approval.Reject';
  target_id: string;
  target_type: 'notification' | 'approval';
  expected_version: number;
}) {
  return {
    command_id: crypto.randomUUID(),
    operation_id: crypto.randomUUID(),
    command_type: input.command_type,
    schema_version: '1.0',
    target: { id: input.target_id, type: input.target_type, organization_id: organizationId },
    expected_version: input.expected_version,
    expected_lifecycle_epoch: 0,
    expected_authority_epoch: 0,
    issued_at: new Date().toISOString(),
    vector_clock: { entries: {} },
    correlation_id: crypto.randomUUID(),
    causation_id: null,
    payload: { reason: 'Team 6 E2E decision' },
  };
}

async function query(auth: AuthResponse, queryType: string, extraHeaders: Record<string, string> = {}) {
  return fetch(`${base}/api/query?envelope=${encode(queryEnvelope(queryType))}`, {
    headers: { authorization: `Bearer ${auth.access_token}`, ...extraHeaders },
  });
}

suite('real API server journeys', () => {
  it('journey 1: authenticates', async () => {
    const auth = await login();
    expect(auth.access_token.split('.')).toHaveLength(3);
  });

  it('journey 2: lists missions', async () => {
    const auth = await login();
    const response = await query(auth, 'mission.list');
    expect(response.status).toBe(200);
    expect((await response.json()).data.length).toBeGreaterThan(0);
  });

  it('journey 3: lists tasks', async () => {
    const auth = await login();
    expect((await query(auth, 'task.list')).status).toBe(200);
  });

  it('journey 4: lists notifications', async () => {
    const auth = await login();
    expect((await query(auth, 'notification.list')).status).toBe(200);
  });

  it('journey 5: lists approvals', async () => {
    const auth = await login();
    expect((await query(auth, 'approval.list')).status).toBe(200);
  });

  it('journey 6: exposes deterministic 401, 403, 409, 429, and 500 handling', async () => {
    const unauthenticated = await fetch(`${base}/api/query?envelope=${encode(queryEnvelope('mission.list'))}`);
    expect(unauthenticated.status).toBe(401);

    const auth = await login();
    const restricted = await fetch(`${base}/api/command`, {
      method: 'POST',
      headers: { authorization: `Bearer ${auth.access_token}`, 'content-type': 'application/json' },
      body: JSON.stringify(commandEnvelope({
        command_type: 'approval.Approve',
        target_id: 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeee2',
        target_type: 'approval',
        expected_version: 1,
      })),
    });
    expect(restricted.status).toBe(403);

    const stale = await fetch(`${base}/api/command`, {
      method: 'POST',
      headers: { authorization: `Bearer ${auth.access_token}`, 'content-type': 'application/json' },
      body: JSON.stringify(commandEnvelope({
        command_type: 'notification.Acknowledge',
        target_id: 'dddddddd-dddd-4ddd-8ddd-ddddddddddd1',
        target_type: 'notification',
        expected_version: 999,
      })),
    });
    expect(stale.status).toBe(409);

    expect((await query(auth, 'mission.list', { 'x-onyx-test-status': '429' })).status).toBe(429);
    expect((await query(auth, 'mission.list', { 'x-onyx-test-status': '500' })).status).toBe(500);
  });

  it('journey 7: logs out', async () => {
    const auth = await login();
    const response = await fetch(`${base}/api/auth/logout`, {
      method: 'POST',
      headers: { authorization: `Bearer ${auth.access_token}`, 'content-type': 'application/json' },
      body: JSON.stringify({ refresh_token: auth.refresh_token }),
    });
    expect(response.status).toBe(200);
  });
});
