import { describe, expect, it } from 'vitest';
import { buildCommandEnvelope } from '../../src/api/command';
import { encodeEnvelope } from '../../src/api/query';
import { statusTone } from '../../src/components/StatusBadge';
import type { QueryEnvelope } from '../../src/types/query';

function decodeBase64Url(value: string): unknown {
  const normalized = value.replaceAll('-', '+').replaceAll('_', '/');
  const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, '=');
  return JSON.parse(atob(padded));
}

describe('frozen wire contract matrix', () => {
  it.each(Array.from({ length: 64 }, (_, index) => index))('round-trips base64url query envelope case %i', (index) => {
    const envelope: QueryEnvelope = {
      query_id: crypto.randomUUID(), query_type: index % 2 ? 'task.list' : 'mission.list', schema_version: '1.0',
      organization_id: '11111111-1111-1111-1111-111111111111', filters: { index, label: `case-${index}` }, limit: 100,
    };
    expect(decodeBase64Url(encodeEnvelope(envelope))).toEqual(envelope);
  });

  it.each(Array.from({ length: 32 }, (_, index) => index))('generates complete command identifiers case %i', (index) => {
    const envelope = buildCommandEnvelope({
      command_type: index % 2 ? 'approval.Approve' : 'approval.Reject',
      target: { id: crypto.randomUUID(), type: 'approval', organization_id: '11111111-1111-1111-1111-111111111111' },
      expected_version: index, expected_lifecycle_epoch: 0, expected_authority_epoch: 0, payload: { reason: `case-${index}` },
    });
    expect(envelope.command_id).toMatch(/^[0-9a-f-]{36}$/i);
    expect(envelope.operation_id).toMatch(/^[0-9a-f-]{36}$/i);
    expect(envelope.correlation_id).toMatch(/^[0-9a-f-]{36}$/i);
    expect(envelope.expected_version).toBe(index);
  });

  it.each([
    ['active', 'success'], ['approved', 'success'], ['acknowledged', 'success'], ['connected', 'success'],
    ['paused', 'warning'], ['pending', 'warning'], ['blocked', 'warning'], ['connecting', 'warning'],
    ['rejected', 'danger'], ['halted', 'danger'], ['critical', 'danger'], ['disconnected', 'danger'],
    ['syncing', 'info'], ['processing', 'info'], ['draft', 'neutral'], ['archived', 'neutral'],
  ] as const)('maps status %s to %s', (status, tone) => expect(statusTone(status)).toBe(tone));
});
