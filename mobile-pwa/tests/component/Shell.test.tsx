import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { screen, waitFor } from '@testing-library/react';
import { AppRoutes } from '../../src/routes';
import { observerApi } from '../../src/api/onyx';
import { useAuthStore } from '../../src/stores/authStore';
import { renderWithProviders, authenticate, queryResponse } from '../test-utils';

vi.mock('../../src/api/onyx', async () => {
  const actual = await vi.importActual<typeof import('../../src/api/onyx')>('../../src/api/onyx');
  return {
    ...actual,
    observerApi: { ...actual.observerApi, query: vi.fn() },
  };
});

beforeEach(() => {
  vi.mocked(observerApi.query).mockResolvedValue(queryResponse([]));
  useAuthStore.setState({ accessToken: null, refreshToken: null, user: null, isAuthenticated: false });
  sessionStorage.clear();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('protected shell', () => {
  it('redirects unauthenticated visits to the login screen', () => {
    renderWithProviders(<AppRoutes />, '/dashboard');
    expect(screen.getByRole('heading', { name: 'ONYX Observer' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Sign in' })).toBeInTheDocument();
  });

  it('renders the dashboard inside the observer layout once authenticated', () => {
    authenticate();
    renderWithProviders(<AppRoutes />, '/dashboard');
    expect(screen.getByText('ONYX Observer')).toBeInTheDocument();
    expect(screen.getByText('read-only')).toBeInTheDocument();
    expect(screen.getAllByRole('heading', { name: 'Dashboard' }).length).toBeGreaterThan(0);
  });

  it('signs out and clears the session', async () => {
    authenticate();
    renderWithProviders(<AppRoutes />, '/dashboard');
    screen.getByRole('button', { name: 'Sign out' }).click();
    await waitFor(() => expect(useAuthStore.getState().isAuthenticated).toBe(false));
    expect(sessionStorage.getItem('onyx_observer_access_token')).toBeNull();
  });
});