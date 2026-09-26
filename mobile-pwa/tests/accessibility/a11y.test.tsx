import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { screen } from '@testing-library/react';
import { StatusBadge } from '../../src/components/StatusBadge';
import { Freshness } from '../../src/components/Freshness';
import { LoginPage } from '../../src/pages/Login';
import { NotificationsPage } from '../../src/pages/Notifications';
import { observerApi } from '../../src/api/onyx';
import { renderWithProviders, queryResponse, authenticate } from '../test-utils';

vi.mock('../../src/api/onyx', async () => {
  const actual = await vi.importActual<typeof import('../../src/api/onyx')>('../../src/api/onyx');
  return {
    ...actual,
    observerApi: { ...actual.observerApi, query: vi.fn() },
  };
});

describe('accessibility', () => {
  it('status badge renders with no axe violations', async () => {
    const { container } = renderWithProviders(<StatusBadge status="pending" />);
    const results = await axe(container);
    expect(results.violations).toHaveLength(0);
  });

  it('freshness stars render with no axe violations', async () => {
    const { container } = renderWithProviders(<Freshness state={{ kind: 'stale' }} />);
    const results = await axe(container);
    expect(results.violations).toHaveLength(0);
  });

  it('login screen passes axe', async () => {
    const { container } = renderWithProviders(<LoginPage />);
    const results = await axe(container);
    expect(results.violations).toHaveLength(0);
  });

  it('notifications list passes axe with real content', async () => {
    vi.mocked(observerApi.query).mockResolvedValue(
      queryResponse([
        {
          id: 'n-1',
          version: 1,
          lifecycle_epoch: 0,
          authority_epoch: 0,
          title: 'Axe notification',
          message: 'Checking contrast.',
          priority: 'normal',
          status: 'unacknowledged',
          source_id: 'm-1',
          source_type: 'mission',
          created_at: '2026-09-24T00:00:00.000Z',
          acknowledged_at: null,
        },
      ]),
    );
    authenticate();
    const { container } = renderWithProviders(<NotificationsPage />, '/notifications');
    await screen.findByText('Axe notification');
    const results = await axe(container);
    expect(results.violations).toHaveLength(0);
  });
});