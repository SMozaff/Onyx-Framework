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

export function setup() {
  const response = http.post(
    `${baseUrl}/api/auth/login`,
    JSON.stringify({ username: 'All-Father', password: 'passvord0000' }),
    { headers: { 'content-type': 'application/json' } },
  );
  if (response.status !== 200) throw new Error(`login failed: ${response.status}`);
  return { token: response.json('access_token') };
}

export default function (data) {
  // A fixed operation_id makes the command idempotent after the first
  // successful acknowledgment, so the benchmark measures the complete
  // authenticated command path without generating conflicting mutations.
  const envelope = {
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
