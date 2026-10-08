import { useMutation, useQueryClient } from '@tanstack/react-query';
import { observerApi } from '../api/onyx';
import { normalizeError, showToast } from '../utils/errorHandler';
import { useAuthStore } from '../stores/authStore';
import type { AuthUser, LoginResponse } from '../types/api';

interface AuthMutationResult {
  title: string;
  message: string;
}

export function useAuth(): {
  login: (options: { username: string; password: string }) => Promise<AuthMutationResult>;
  logout: () => Promise<void>;
  isPending: boolean;
  isAuthenticated: boolean;
  user: AuthUser | null;
} {
  const queryClient = useQueryClient();
  const { login: setSession, logout: clearSession } = useAuthStore();
  const isAuthenticated = useAuthStore((state) => state.isAuthenticated);
  const user = useAuthStore((state) => state.user);

  const loginMutation = useMutation({
    mutationFn: ({ username, password }: { username: string; password: string }) =>
      observerApi.authenticate(username, password).then((response) => response.data),
    onSuccess: (response: LoginResponse) => {
      setSession(response);
      queryClient.clear();
    },
    onError: (error) => {
      const normalized = normalizeError(error);
      showToast(normalized.title, 'error');
    },
  });

  const logoutMutation = useMutation({
    mutationFn: async () => {
      const refreshToken = useAuthStore.getState().refreshToken;
      if (refreshToken) {
        try {
          await observerApi.logout({ refresh_token: refreshToken });
        } catch {
          /* ignore — the local session is cleared regardless */
        }
      }
      clearSession();
      queryClient.clear();
    },
  });

  return {
    login: async ({ username, password }) => {
      await loginMutation.mutateAsync({ username, password });
      return { title: 'Signed in', message: 'You are now signed in as an observer.' };
    },
    logout: async () => logoutMutation.mutateAsync(),
    isPending: loginMutation.isPending || logoutMutation.isPending,
    isAuthenticated,
    user,
  };
}