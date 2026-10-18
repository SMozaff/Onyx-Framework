import axios, { type AxiosRequestConfig } from 'axios';
import { useAuthStore } from '../stores/authStore';
import { getServerAddress } from '../utils/serverAddress';

type RetriableRequestConfig = AxiosRequestConfig & { _onyxRetried?: boolean };
type RefreshResponse = { access_token: string; refresh_token: string; user: NonNullable<ReturnType<typeof useAuthStore.getState>['user']> };
let refreshInFlight: Promise<boolean> | null = null;

/** All Admin requests use the single deployed ONYX Cloud API. */
export const apiClient = axios.create({
  timeout: 30_000,
  headers: { 'Content-Type': 'application/json' },
});

apiClient.interceptors.request.use((config) => {
  const address = getServerAddress();
  config.baseURL = address;
  const token = useAuthStore.getState().accessToken;
  if (token && !config.headers.get('Authorization')) {
    config.headers.set('Authorization', `Bearer ${token}`);
  }
  return config;
});

async function refreshSession(): Promise<boolean> {
  const refreshToken = useAuthStore.getState().refreshToken;
  const address = getServerAddress().replace(/\/+$/, '');
  if (!refreshToken) return false;
  try {
    const response = await axios.post<RefreshResponse>(
      `${address}/api/auth/refresh`,
      { refresh_token: refreshToken },
      { timeout: 30_000, headers: { 'Content-Type': 'application/json' } },
    );
    if (!response.data?.access_token || !response.data?.refresh_token || !response.data?.user) return false;
    useAuthStore.getState().login(response.data);
    return true;
  } catch {
    return false;
  }
}

apiClient.interceptors.response.use(
  (response) => response,
  async (error) => {
    const request = error.config as RetriableRequestConfig | undefined;
    const authEndpoint = request?.url?.startsWith('/api/auth/') ?? false;
    if (error.response?.status === 401 && request && !authEndpoint && !request._onyxRetried) {
      request._onyxRetried = true;
      if (!refreshInFlight) {
        refreshInFlight = refreshSession().finally(() => { refreshInFlight = null; });
      }
      const refreshed = await refreshInFlight;
      const token = useAuthStore.getState().accessToken;
      if (refreshed && token) {
        const retryHeaders = axios.AxiosHeaders.from(request.headers as never);
        retryHeaders.set('Authorization', `Bearer ${token}`);
        request.headers = retryHeaders;
        return apiClient.request(request);
      }
      useAuthStore.getState().logout();
      if (window.location.pathname !== '/login') window.location.assign('/login');
    }
    return Promise.reject(error);
  },
);
