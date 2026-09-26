import { beforeEach, describe, expect, it } from 'vitest';
import { buildQueryEnvelope, encodeEnvelope } from '../../src/api/onyx';
import { useAuthStore } from '../../src/stores/authStore';
import type { QueryEnvelope } from '../../src/types/query';

function decodeEnvelope(encoded: string): QueryEnvelope {
  const binary = atob(encoded.replaceAll('-', '+').replaceAll('_', '/'));
  const bytes = new Uint8Array([...binary].map((char) => char.charCodeAt(0)));
  return JSON.parse(new TextDecoder().decode(bytes)) as QueryEnvelope;
}

beforeEach(() => {
  useAuthStore.setState({
    accessToken: 'access',
    refreshToken: 'refresh',
    user: { id: 'user-1', username: 'observer', organization_id: 'org-uuid-1' },
    isAuthenticated: true,
  });
});

describe('encodeEnvelope', () => {
  it('round-trips through base64url without padding', () => {
    const envelope: QueryEnvelope = {
      query_id: 'q-1',
      query_type: 'mission.list',
      schema_version: '1.0',
      organization_id: 'org-uuid-1',
      filters: { status: 'active' },
      limit: 100,
      sort_order: 'desc',
    };
    const encoded = encodeEnvelope(envelope);
    expect(encoded).not.toContain('=');
    expect(encoded).not.toContain('+');
    expect(encoded).not.toContain('/');
    expect(decodeEnvelope(encoded)).toEqual(envelope);
  });
});

describe('buildQueryEnvelope', () => {
  it('fills query identity, organization, and defaults', () => {
    const envelope = buildQueryEnvelope('approval.list', { status: 'pending' });
    expect(envelope.query_type).toBe('approval.list');
    expect(envelope.organization_id).toBe('org-uuid-1');
    expect(envelope.schema_version).toBe('1.0');
    expect(envelope.filters).toEqual({ status: 'pending' });
    expect(envelope.limit).toBe(100);
    expect(envelope.sort_order).toBe('desc');
    expect(envelope.query_id).toBeTruthy();
  });

  it('honors options', () => {
    const envelope = buildQueryEnvelope('task.list', {}, { limit: 5, cursor: '20', sort_order: 'asc' });
    expect(envelope.limit).toBe(5);
    expect(envelope.cursor).toBe('20');
    expect(envelope.sort_order).toBe('asc');
  });

  it('throws without an authenticated user', () => {
    useAuthStore.setState({ user: null, isAuthenticated: false });
    expect(() => buildQueryEnvelope('mission.list')).toThrow(/Authenticated user/);
  });
});