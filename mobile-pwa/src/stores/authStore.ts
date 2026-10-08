import { create } from 'zustand';
import {
  clearSession,
  getAccessToken,
  getRefreshToken,
  getStoredUser,
  storeSession,
} from '../utils/auth';
import type { AuthUser } from '../types/api';

interface AuthState {
  accessToken: string | null;
  refreshToken: string | null;
  user: AuthUser | null;
  isAuthenticated: boolean;
  login: (response: { access_token: string; refresh_token: string; user: AuthUser }) => void;
  logout: () => void;
}

export const useAuthStore = create<AuthState>((set) => ({
  accessToken: getAccessToken(),
  refreshToken: getRefreshToken(),
  user: getStoredUser(),
  isAuthenticated: Boolean(getAccessToken() && getStoredUser()),
  login: ({ access_token, refresh_token, user }) => {
    storeSession(access_token, refresh_token, user);
    set({ accessToken: access_token, refreshToken: refresh_token, user, isAuthenticated: true });
  },
  logout: () => {
    clearSession();
    set({ accessToken: null, refreshToken: null, user: null, isAuthenticated: false });
  },
}));