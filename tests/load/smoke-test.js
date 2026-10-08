import http from 'k6/http';
import { check } from 'k6';
import { Trend, Rate } from 'k6/metrics';

const commandLatency = new Trend('command_latency', true);
const commandFailures = new Rate('command_failures');
const baseUrl = __ENV.ONYX_BASE_URL || 'http://127.0.0.1:3000';
const organizationId = '11111111-1111-1111-1111-111111111111';

export const options = {
  vus: 100,
  duration: '60s',
  thresholds: {
    command_latency: ['p(95)<500'],
    command_failures: ['rate<0.01'],
    http_req_failed: ['rate<0.01'],
  },
};

function commandEnvelope() {
  return {
    command_id: '88888888-8888-4888-8888-888888888888',
    operation_id: '99999999-9999-4999-8999-999999999999',
    command_type: 'notification.Acknowledge',
    schema_version: '1.0',
    target: {
      id: 'dddddddd-dddd-4ddd-8ddd-ddddddddddd1',
      type: 'notification',
      organization_id: organizationId,
    },
    expected_version: 1,
    expected_lifecycle_epoch: 0,
    expected_authority_epoch: 0,
    issued_at: new Date().toISOString(),
    vector_clock: { entries: {} },
    correlation_id: '77777777-7777-4777-8777-777777777777',
    causation_id: null,
    payload: {},
  };
}

export function setup() {
  const response = http.post(
    `${baseUrl}/api/auth/login`,
    JSON.stringify({
      username: __ENV.ONYX_TEST_USERNAME || 'ci-admin',
      password: __ENV.ONYX_TEST_PASSWORD || 'ci-smoke-password-2026',
      client_type: 'admin',
    }),
    { headers: { 'content-type': 'application/json' } },
  );
  if (response.status !== 200) throw new Error(`login failed: ${response.status}`);

  // Warm the fixed operation through one successful mutation before the
  // concurrent phase. Without this, 100 VUs can all observe the same
  // operation_id as a cache miss, race on the version-1 notification, and
  // legitimately produce optimistic-concurrency failures. The sustained
  // phase therefore measures the authenticated command/idempotency path
  // under load rather than turning the smoke test into a race detector.
  const warmup = http.post(
    `${baseUrl}/api/command`,
    JSON.stringify(commandEnvelope()),
    {
      headers: {
        authorization: `Bearer ${response.json('access_token')}`,
        'content-type': 'application/json',
        'x-correlation-id': '77777777-7777-4777-8777-777777777777',
      },
    },
  );
  if (warmup.status !== 200) {
    throw new Error(`command warmup failed: ${warmup.status} ${warmup.body}`);
  }

  return { token: response.json('access_token') };
}

export default function (data) {
  const envelope = commandEnvelope();
  const started = Date.now();
  const response = http.post(`${baseUrl}/api/command`, JSON.stringify(envelope), {
    headers: {
      authorization: `Bearer ${data.token}`,
      'content-type': 'application/json',
      'x-correlation-id': envelope.correlation_id,
    },
  });
  commandLatency.add(Date.now() - started);
  const passed = check(response, {
    'command returned success': (result) => result.status === 200,
  });
  commandFailures.add(!passed);
}
