import { renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { useOnyxQuery } from '../../src/hooks/useQuery';
import type { MissionSummary } from '../../src/types/query';
import { authenticate, TestProviders } from '../test-utils';

describe('query flow', () => {
  it('loads and caches a mission projection', async () => {
    authenticate();
    const { result } = renderHook(() => useOnyxQuery<MissionSummary>('mission.list'), { wrapper: TestProviders });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data?.data[0].status).toBe('active');
    expect(result.current.data?.freshness.is_stale).toBe(false);
  });
});
