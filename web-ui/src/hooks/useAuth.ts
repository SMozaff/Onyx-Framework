import { useMutation } from '@tanstack/react-query';
import { apiClient } from '../api/client';
import { useAuthStore } from '../stores/authStore';
import { normalizeError, showToast } from '../utils/errorHandler';
import type { LoginResponse } from '../types/api';

export function useAuth() {
  const store = useAuthStore();
  const login = useMutation({
    networkMode: 'always',
    mutationFn: async (credentials: { username: string; password: string }) =>
      (
        await apiClient.post<LoginResponse>('/api/auth/login', {
          ...credentials,
          // Additive on api-server's LoginRequest; only client_type:
          // "mobile" is ever gated (see routes/auth.rs), so this has no
          // behavioral effect on web-ui today -- sent for consistency
          // with every other first-party client (desktop-shell: "desktop",
          // admin-shell: "admin", mobile: "mobile"), found missing here
          // when re-verifying the class-based mobile access control piece.
          client_type: 'web',
        })
      ).data,
    onSuccess: (response) => store.login(response),
    onError: (error) => showToast(normalizeError(error).message, 'error'),
  });

  const logout = async () => {
    const refreshToken = store.refreshToken;
    try {
      if (refreshToken) await apiClient.post('/api/auth/logout', { refresh_token: refreshToken });
    } finally {
      store.logout();
    }
  };

  return { ...store, login, logout };
}
