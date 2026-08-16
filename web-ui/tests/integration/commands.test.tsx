import { act, renderHook, waitFor } from '@testing-library/react';
import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import { useAcknowledgeNotification } from '../../src/hooks/useCommand';
import type { NotificationProjection } from '../../src/types/query';
import { server } from '../mocks/server';
import { authenticate, TestProviders } from '../test-utils';

const notification: NotificationProjection = {
  id: 'dddddddd-dddd-4ddd-8ddd-ddddddddddd1', title: 'Alert', message: 'Message', priority: 'high', status: 'unacknowledged',
  source_id: 'a', source_type: 'mission', created_at: '2026-08-05T00:00:00Z', acknowledged_at: null,
  version: 1, lifecycle_epoch: 0, authority_epoch: 0,
};

describe('command flow', () => {
  it('acknowledges through the API and never mutates the input projection', async () => {
    authenticate();
    const { result } = renderHook(() => useAcknowledgeNotification(), { wrapper: TestProviders });
    // mutateAsync's returned promise resolves with the mutationFn's result,
    // but the mutation's isSuccess flag is a separate React Query state
    // transition that lands slightly after — reading it synchronously here
    // races that transition. waitFor is the pattern this repo already uses
    // for the same reason in queries.test.tsx.
    await act(async () => { await result.current.mutateAsync(notification); });
    expect(notification.status).toBe('unacknowledged');
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });

  it('fails visibly when the network is lost', async () => {
    authenticate();
    server.use(http.post('*/api/command', () => HttpResponse.error()));
    const networkEvent = new Promise<void>((resolve) => window.addEventListener('onyx:network-error', () => resolve(), { once: true }));
    const { result } = renderHook(() => useAcknowledgeNotification(), { wrapper: TestProviders });
    act(() => result.current.mutate(notification));
    await waitFor(() => expect(result.current.isError).toBe(true));
    await expect(networkEvent).resolves.toBeUndefined();
    expect(notification.status).toBe('unacknowledged');
  });
});
