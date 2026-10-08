import axios from 'axios';
import { useAuthStore } from '../stores/authStore';
import { getServerAddress, isSecureEnoughForProduction } from '../utils/serverAddress';

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
  const address = getServerAddress();
  // H4(b) backstop: both connection-settings forms (Login's
  // ConnectionSettings and Settings' ServerConnectionSettings) already
  // refuse to *save* an insecure address, but this also covers an address
  // saved by an older build before this check existed, or a value edited
  // directly in localStorage — nothing reaches the network with
  // credentials over an insecure link regardless of how the address got
  // stored.
  if (!isSecureEnoughForProduction(address)) {
    return Promise.reject(
      new Error(
        `Refusing to send a request to ${address}: only https:// (or http://127.0.0.1) is allowed. Update the server address in Settings.`,
      ),
    );
  }
  config.baseURL = address;
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
