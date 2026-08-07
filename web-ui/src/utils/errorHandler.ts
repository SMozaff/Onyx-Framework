import axios from 'axios';
import type { CommandError } from '../types/command';

export type ToastTone = 'info' | 'success' | 'warning' | 'error';

export interface ToastDetail {
  message: string;
  tone: ToastTone;
}

export function showToast(message: string, tone: ToastTone = 'info'): void {
  window.dispatchEvent(new CustomEvent<ToastDetail>('onyx:toast', { detail: { message, tone } }));
}

export function notifyNetworkError(): void {
  window.dispatchEvent(new Event('onyx:network-error'));
}

export function normalizeError(error: unknown): { message: string; status?: number; commandError?: CommandError } {
  if (axios.isAxiosError(error)) {
    const status = error.response?.status;
    const commandError = error.response?.data?.error as CommandError | undefined;
    if (!error.response) {
      notifyNetworkError();
      return { message: 'Connection lost. Check your network and try again.' };
    }
    const fallback: Record<number, string> = {
      400: 'The request could not be validated.',
      401: 'Your session has expired. Sign in again.',
      403: 'You do not have permission to perform this action.',
      404: 'The requested resource was not found.',
      409: 'This item changed. Refresh and retry.',
      429: 'The organization quota has been reached. Try again later.',
      500: 'The server could not complete the request. Try again later.',
    };
    const safeMessage = commandError?.safe_details?.message;
    return {
      message: typeof safeMessage === 'string' ? safeMessage : fallback[status ?? 500] ?? 'Request failed.',
      status,
      commandError,
    };
  }
  return { message: error instanceof Error ? error.message : 'Unexpected error.' };
}
