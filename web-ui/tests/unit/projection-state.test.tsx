import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { deriveProjectionState, ProjectionStatePanel } from '../../src/components/ProjectionState';
import { normalizeError } from '../../src/utils/errorHandler';
import type { QueryResponse } from '../../src/types/query';

function queryFixture<T>(overrides: Record<string, unknown>) {
  return {
    data: undefined,
    isPending: false,
    isError: false,
    refetch: vi.fn(),
    ...overrides,
  } as never;
}

const emptyResponse: QueryResponse<{ id: string }> = {
  query_id: 'query',
  data: [],
  has_more: false,
  next_cursor: null,
  total_count: 0,
  freshness: { projection_version: 1, last_updated_at: '2026-08-20T00:00:00Z', is_stale: false },
};

describe('projection state', () => {
  it('treats an initial request failure as unavailable rather than empty', () => {
    const refetch = vi.fn();
    const state = deriveProjectionState(queryFixture({ isError: true, refetch }));
    expect(state.kind).toBe('unavailable');
    if (state.kind === 'unavailable') {
      expect(state.error.message).not.toMatch(/undefined|Error/i);
      state.retry();
      expect(refetch).toHaveBeenCalledOnce();
    }
  });

  it('distinguishes a successful empty response from unavailable data', () => {
    const state = deriveProjectionState(queryFixture({ data: emptyResponse }));
    expect(state).toMatchObject({ kind: 'empty', lastUpdatedAt: '2026-08-20T00:00:00Z' });
  });

  it('keeps successful data visible but marks it stale', () => {
    const response = { ...emptyResponse, data: [{ id: 'mission-1' }], freshness: { ...emptyResponse.freshness, is_stale: true } };
    const state = deriveProjectionState(queryFixture({ data: response }));
    expect(state.kind).toBe('stale');
  });

  it('renders an unavailable panel with recovery action and no empty-state count', () => {
    const state = deriveProjectionState(queryFixture({ isError: true }));
    if (state.kind !== 'unavailable') throw new Error('expected unavailable state');
    render(<ProjectionStatePanel resource="Missions" state={state} />);
    expect(screen.getByRole('alert')).toHaveTextContent('Missions unavailable');
    expect(screen.getByRole('button', { name: 'Retry' })).toBeEnabled();
    expect(screen.queryByText(/0 total/i)).not.toBeInTheDocument();
  });
});

describe('safe browser errors', () => {
  it('does not expose arbitrary technical error text', () => {
    const safe = normalizeError(new Error('TypeError: secret host / internal stack'));
    expect(safe.code).toBe('UNEXPECTED');
    expect(safe.message).not.toContain('secret host');
  });
});
