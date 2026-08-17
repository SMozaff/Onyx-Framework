import { useState } from 'react';
import { cleanup, fireEvent, screen } from '@testing-library/react';
import { http, HttpResponse } from 'msw';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import UserPicker from '../../src/components/UserPicker';
import StaffLoansPage from '../../src/pages/StaffLoans';
import TodoTargetsPage from '../../src/pages/TodoTargets';
import { useAuthStore } from '../../src/stores/authStore';
import { server } from '../mocks/server';
import { renderWithProviders } from '../test-utils';

const currentUser = {
  id: '22222222-2222-2222-2222-222222222222',
  username: 'test-admin',
  organization_id: '11111111-1111-1111-1111-111111111111',
};

const pickerUsers = [
  currentUser,
  { id: '33333333-3333-3333-3333-333333333333', username: 'Amina Coordinator' },
  { id: '44444444-4444-4444-4444-444444444444', username: 'Boris Manager' },
];

function response(queryId: string, data: unknown[]) {
  return HttpResponse.json({
    query_id: queryId,
    data,
    has_more: false,
    next_cursor: null,
    total_count: data.length,
    freshness: { projection_version: 1, last_updated_at: '2026-08-17T00:00:00Z', is_stale: false },
  });
}

function decodeEnvelope(url: URL) {
  const encoded = url.searchParams.get('envelope') ?? '';
  const normalized = encoded.replaceAll('-', '+').replaceAll('_', '/');
  const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, '=');
  return JSON.parse(Buffer.from(padded, 'base64').toString('utf8')) as { query_id: string; query_type: string };
}

function PickerFixture() {
  const [selected, setSelected] = useState('');
  return (
    <>
      <UserPicker label="Assignee" value={selected} onChange={setSelected} />
      <output>{selected}</output>
    </>
  );
}

describe('remaining web-ui workflows', () => {
  beforeEach(() => {
    useAuthStore.getState().login({ access_token: 'test-token', refresh_token: 'test-refresh', user: currentUser });
    server.use(http.get('*/api/users', () => HttpResponse.json(pickerUsers)));
  });

  afterEach(() => {
    cleanup();
    useAuthStore.getState().logout();
  });

  it('filters and selects a reduced user identity through the shared picker', async () => {
    renderWithProviders(<PickerFixture />);

    await screen.findByRole('option', { name: 'Amina Coordinator' });
    fireEvent.change(screen.getByRole('searchbox', { name: 'Search assignee' }), { target: { value: 'amina' } });
    expect(screen.getByRole('option', { name: 'Amina Coordinator' })).toBeInTheDocument();
    expect(screen.queryByRole('option', { name: 'Boris Manager' })).not.toBeInTheDocument();

    fireEvent.change(screen.getByRole('combobox', { name: 'Assignee' }), { target: { value: pickerUsers[1].id } });
    expect(screen.getByText(pickerUsers[1].id)).toBeInTheDocument();
  });

  it('filters Todo lists escalated to the signed-in user and reaches the decision actions', async () => {
    server.use(
      http.get('*/api/query', ({ request }) => {
        const envelope = decodeEnvelope(new URL(request.url));
        if (envelope.query_type !== 'todo_list.list') return response(envelope.query_id, []);
        return response(envelope.query_id, [
          {
            id: 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
            owner: '33333333-3333-3333-3333-333333333333',
            origin: 'ManagerAssigned',
            items: [{ item_id: 'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb', description: 'Escalated work' }],
            status: 'Escalated',
            escalated_to: currentUser.id,
            version: 2,
            lifecycle_epoch: 0,
            authority_epoch: 1,
          },
          {
            id: 'cccccccc-cccc-cccc-cccc-cccccccccccc',
            owner: '33333333-3333-3333-3333-333333333333',
            origin: 'ManagerAssigned',
            items: [{ item_id: 'dddddddd-dddd-dddd-dddd-dddddddddddd', description: 'Someone else\'s work' }],
            status: 'Escalated',
            escalated_to: '44444444-4444-4444-4444-444444444444',
            version: 2,
            lifecycle_epoch: 0,
            authority_epoch: 1,
          },
        ]);
      }),
    );

    renderWithProviders(<TodoTargetsPage />);
    await screen.findByText('2 shown');
    fireEvent.change(screen.getByLabelText('Show'), { target: { value: 'escalated' } });
    expect(await screen.findByText('1 shown')).toBeInTheDocument();

    fireEvent.click(screen.getByText('1 item').closest('button')!);
    expect(await screen.findByText('This verification decision was escalated to you.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Verify' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Reject' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Escalate' })).toBeInTheDocument();
  });

  it('filters Staff Loan approvals escalated to the signed-in user and exposes approval actions', async () => {
    server.use(
      http.get('*/api/query', ({ request }) => {
        const envelope = decodeEnvelope(new URL(request.url));
        if (envelope.query_type !== 'staff_loan.list') return response(envelope.query_id, []);
        return response(envelope.query_id, [
          {
            id: 'eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee',
            staff_user_id: '33333333-3333-3333-3333-333333333333',
            real_owner_id: '44444444-4444-4444-4444-444444444444',
            borrowing_manager_id: '55555555-5555-5555-5555-555555555555',
            window: { start_at: 1_700_000_000_000_000_000, end_at: 1_700_086_400_000_000_000 },
            status: 'Requested',
            escalated_to: currentUser.id,
            version: 1,
            lifecycle_epoch: 0,
            authority_epoch: 1,
          },
          {
            id: 'ffffffff-ffff-ffff-ffff-ffffffffffff',
            staff_user_id: '33333333-3333-3333-3333-333333333333',
            real_owner_id: '44444444-4444-4444-4444-444444444444',
            borrowing_manager_id: '55555555-5555-5555-5555-555555555555',
            window: { start_at: 1_700_000_000_000_000_000, end_at: 1_700_086_400_000_000_000 },
            status: 'Requested',
            escalated_to: '44444444-4444-4444-4444-444444444444',
            version: 1,
            lifecycle_epoch: 0,
            authority_epoch: 1,
          },
        ]);
      }),
    );

    renderWithProviders(<StaffLoansPage />);
    await screen.findByText('2 shown');
    fireEvent.change(screen.getByLabelText('Show'), { target: { value: 'escalated' } });

    expect(await screen.findByText('1 shown')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Loan (escalated to you)' })).toBeInTheDocument();
    expect(screen.getByText('This approval decision was escalated to you.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Approve' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Decline' })).toBeInTheDocument();
  });
});
