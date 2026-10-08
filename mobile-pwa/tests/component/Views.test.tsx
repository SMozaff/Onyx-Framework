import { afterEach, describe, expect, it, vi } from 'vitest';
import { screen, within } from '@testing-library/react';
import { observerApi } from '../../src/api/onyx';
import { MissionsPage } from '../../src/pages/Missions';
import { NotificationsPage } from '../../src/pages/Notifications';
import { ApprovalsPage } from '../../src/pages/Approvals';
import { renderWithProviders, queryResponse, authenticate } from '../test-utils';

vi.mock('../../src/api/onyx', async () => {
  const actual = await vi.importActual<typeof import('../../src/api/onyx')>('../../src/api/onyx');
  return {
    ...actual,
    observerApi: { ...actual.observerApi, query: vi.fn() },
  };
});

afterEach(() => {
  vi.clearAllMocks();
});

const mission = {
  id: 'mission-1',
  version: 1,
  lifecycle_epoch: 0,
  authority_epoch: 0,
  name: 'Mission Alpha',
  summary: 'Synthetic mission.',
  status: 'active',
  owner: 'audit.operator',
  priority: 'normal',
  progress: 40,
  updated_at: '2026-09-24T00:00:00.000Z',
};

describe('observer views render projections', () => {
  it('lists missions and links into the read-only detail', async () => {
    vi.mocked(observerApi.query).mockResolvedValue(queryResponse([mission]));
    authenticate();
    const { container } = renderWithProviders(<MissionsPage />, '/missions');

    await screen.findByText('Mission Alpha');
    expect(screen.getByRole('heading', { name: 'Missions' })).toBeInTheDocument();
    expect(screen.getByText('audit.operator')).toBeInTheDocument();
    const link = within(container).getByRole('link', { name: /Mission Alpha/ });
    expect(link).toHaveAttribute('href', '/mission/mission-1');
  });

  it('never offers an acknowledge action on notifications', async () => {
    vi.mocked(observerApi.query).mockResolvedValue(
      queryResponse([
        {
          id: 'n-1',
          version: 1,
          lifecycle_epoch: 0,
          authority_epoch: 0,
          title: 'Service reconfigured',
          message: 'All good.',
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
    renderWithProviders(<NotificationsPage />, '/notifications');

    await screen.findByText('Service reconfigured');
    expect(screen.getByLabelText(/Status: Unacknowledged/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /acknowledge/i })).toBeNull();
  });

  it('never offers an approve or reject action on approvals', async () => {
    vi.mocked(observerApi.query).mockResolvedValue(
      queryResponse([
        {
          id: 'a-1',
          version: 1,
          lifecycle_epoch: 0,
          authority_epoch: 0,
          title: 'Approve evidence',
          description: 'Review set.',
          status: 'pending',
          requested_by: 'audit.operator',
          target_id: 'task-1',
          target_type: 'task',
          created_at: '2026-09-24T00:00:00.000Z',
          decided_at: null,
          decision_reason: null,
          web_action_permitted: true,
        },
      ]),
    );
    authenticate();
    renderWithProviders(<ApprovalsPage />, '/approvals');

    await screen.findByText('Approve evidence');
    expect(screen.queryByRole('button', { name: /approve/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /reject/i })).toBeNull();
    expect(screen.getByText(/decision-free by contract/i)).toBeInTheDocument();
  });
});