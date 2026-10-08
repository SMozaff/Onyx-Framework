import { describe, expect, it } from 'vitest';
import { buildCommandEnvelope, executeCommand } from '../../src/api/command';
import { buildQueryEnvelope, encodeEnvelope, executeQuery } from '../../src/api/query';
import { authenticate } from '../test-utils';

const target = { id: 'dddddddd-dddd-4ddd-8ddd-ddddddddddd1', type: 'notification', organization_id: '11111111-1111-1111-1111-111111111111' };

describe('API contract helpers', () => {
  it('builds a command without client-constructed authority proof', () => {
    const envelope = buildCommandEnvelope({ command_type: 'notification.Acknowledge', target, expected_version: 1, expected_lifecycle_epoch: 0, expected_authority_epoch: 0, payload: {} });
    expect(envelope.authority_proof).toBeUndefined();
    expect(envelope.actor).toBeUndefined();
    expect(envelope.command_type).toBe('notification.Acknowledge');
  });

  it('encodes a query as base64url JSON', () => {
    authenticate();
    const envelope = buildQueryEnvelope('mission.list');
    expect(encodeEnvelope(envelope)).toMatch(/^[A-Za-z0-9_-]+$/);
  });

  it('executes a query against the mock API', async () => {
    authenticate();
    const result = await executeQuery<{ name: string }>('mission.list');
    expect(result.data[0].name).toBe('Coastal Response Readiness');
  });

  it('executes a command against the mock API', async () => {
    authenticate();
    const result = await executeCommand({ command_type: 'notification.Acknowledge', target, expected_version: 1, expected_lifecycle_epoch: 0, expected_authority_epoch: 0, payload: {} });
    expect(result.success).toBe(true);
    expect(result.new_version).toBe(2);
  });
});
