import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, screen, waitFor } from '@testing-library/react';
import { PushNotificationsCard } from '../../src/components/PushNotificationsCard';
import { usePushStore } from '../../src/stores/pushStore';
import { renderWithProviders } from '../test-utils';

vi.mock('../../src/push/push', async () => {
  const actual = await vi.importActual<typeof import('../../src/push/push')>('../../src/push/push');
  return {
    ...actual,
    isPushSupported: vi.fn(() => true),
    getVapidKey: vi.fn(() => 'test-vapid-key'),
    readStoredPush: vi.fn(() => null),
    subscribeToPush: vi.fn(),
    unsubscribeFromPush: vi.fn(),
  };
});

import { subscribeToPush, unsubscribeFromPush } from '../../src/push/push';

beforeEach(() => {
  usePushStore.setState({ status: 'idle', subscriptionId: null, endpoint: null, pending: false, message: null });
  vi.clearAllMocks();
});

describe('PushNotificationsCard', () => {
  it('renders the supported idle state with an actionable enable button', async () => {
    renderWithProviders(<PushNotificationsCard />);
    const button = await screen.findByRole('button', { name: 'Enable notifications' });
    expect(button).toBeEnabled();
  });

  it('enables push and then offers disable', async () => {
    vi.mocked(subscribeToPush).mockResolvedValue({
      subscriptionId: 'sub-1',
      endpoint: 'https://push.example.test/deterministic-endpoint',
    });
    renderWithProviders(<PushNotificationsCard />);
    fireEvent.click(await screen.findByRole('button', { name: 'Enable notifications' }));

    await screen.findByRole('button', { name: 'Disable' });
    expect(screen.getByText('https://push.example.test/deterministic-endpoint')).toBeInTheDocument();
    expect(subscribeToPush).toHaveBeenCalledOnce();
    expect(localStorage.getItem('onyx_observer_push')).toContain('sub-1');
  });

  it('disables push and removes the server row', async () => {
    usePushStore.setState({
      status: 'subscribed',
      subscriptionId: 'sub-1',
      endpoint: 'https://push.example.test/x',
      pending: false,
      message: null,
    });
    renderWithProviders(<PushNotificationsCard />);
    fireEvent.click(await screen.findByRole('button', { name: 'Disable' }));

    await waitFor(() => expect(unsubscribeFromPush).toHaveBeenCalledWith('sub-1'));
    await screen.findByRole('button', { name: 'Enable notifications' });
    expect(localStorage.getItem('onyx_observer_push')).toBeNull();
  });

  it('reports a denied permission request without crashing', async () => {
    const { PushSetupError } = await import('../../src/push/push');
    vi.mocked(subscribeToPush).mockRejectedValue(
      new PushSetupError('denied', 'Notification permission was not granted.'),
    );
    renderWithProviders(<PushNotificationsCard />);
    fireEvent.click(await screen.findByRole('button', { name: 'Enable notifications' }));

    expect(
      await screen.findByText(/allow notifications for this site in your browser settings/i),
    ).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Disable' })).toBeNull();
  });
});