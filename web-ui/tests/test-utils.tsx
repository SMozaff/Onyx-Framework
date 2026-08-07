import type { PropsWithChildren } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router-dom';
import { render } from '@testing-library/react';
import { useAuthStore } from '../src/stores/authStore';

export function authenticate() {
  useAuthStore.getState().login({ access_token: 'mock.access.token', refresh_token: 'mock.refresh.token', user: { id: '22222222-2222-2222-2222-222222222222', username: 'test-admin', organization_id: '11111111-1111-1111-1111-111111111111' } });
}

export function createTestClient() { return new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 }, mutations: { retry: false } } }); }

export function TestProviders({ children, path = '/' }: PropsWithChildren<{ path?: string }>) {
  const client = createTestClient();
  return <QueryClientProvider client={client}><MemoryRouter initialEntries={[path]}>{children}</MemoryRouter></QueryClientProvider>;
}

export function renderWithProviders(ui: React.ReactElement, path = '/') { return render(<TestProviders path={path}>{ui}</TestProviders>); }
