import axios from 'axios';
import { useAuthStore } from '../stores/authStore';
import { getServerAddress } from '../utils/serverAddress';

/**
 * No longer a fixed value read once at module load. The server
 * address is user-configurable at runtime (see Settings page +
 * `utils/serverAddress.ts`), so `baseURL` must be resolved fresh on
 * every request via the interceptor below — otherwise a saved change
 * wouldn't take effect until the app was restarted.
 */
export const apiClient = axios.create({
  timeout: 30_000,
  headers: { 'Content-Type': 'application/json' },
});

apiClient.interceptors.request.use((config) => {
  config.baseURL = getServerAddress();
  const token = useAuthStore.getState().accessToken;
  if (token) config.headers.Authorization = `Bearer ${token}`;
  return config;
});

apiClient.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.response?.status === 401) {
      useAuthStore.getState().logout();
      if (window.location.pathname !== '/login') window.location.assign('/login');
    }
    return Promise.reject(error);
  },
);
