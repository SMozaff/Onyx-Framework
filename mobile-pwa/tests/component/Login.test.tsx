import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, screen, waitFor } from '@testing-library/react';
import { apiClient } from '../../src/api/client';
import { useAuthStore } from '../../src/stores/authStore';
import { LoginPage } from '../../src/pages/Login';
import { renderWithProviders } from '../test-utils';

function axiosError(status: number) {
  return {
    isAxiosError: true,
    response: { status, data: { error: { code: 'TEST', safe_details: {} } } },
  };
}

describe('LoginPage', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    sessionStorage.clear();
    useAuthStore.setState({ accessToken: null, refreshToken: null, user: null, isAuthenticated: false });
  });

  it('posts the observer client type on submit', async () => {
    const post = vi.spyOn(apiClient, 'post').mockResolvedValue({
      data: {
        access_token: 'a',
        refresh_token: 'r',
        expires_in: 3600,
        user: { id: 'u', username: 'obs', organization_id: 'org' },
      },
    });
    renderWithProviders(<LoginPage />);
    fireEvent.change(screen.getByLabelText('Username'), { target: { value: 'obs' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'secret' } });
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));

    await waitFor(() => expect(post).toHaveBeenCalledTimes(1));
    expect(post).toHaveBeenCalledWith(
      '/api/auth/login',
      { username: 'obs', password: 'secret', client_type: 'mobile_observer' },
    );
    await waitFor(() => expect(useAuthStore.getState().isAuthenticated).toBe(true));
    expect(sessionStorage.getItem('onyx_observer_access_token')).toBe('a');
  });

  it('surfaces a friendly error and does not authenticate on failure', async () => {
    vi.spyOn(apiClient, 'post').mockRejectedValueOnce(axiosError(401));
    renderWithProviders(<LoginPage />);
    fireEvent.change(screen.getByLabelText('Username'), { target: { value: 'obs' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'wrong' } });
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent(/Sign in again/i));
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
  });
});