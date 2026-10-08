import axios from 'axios';
import type { CommandError } from '../types/query';

export type ToastTone = 'info' | 'success' | 'warning' | 'error';
export type UiErrorCode =
  | 'NETWORK_UNAVAILABLE'
  | 'SESSION_EXPIRED'
  | 'INVALID_CREDENTIALS'
  | 'PERMISSION_DENIED'
  | 'VALIDATION_FAILED'
  | 'RESOURCE_NOT_FOUND'
  | 'SERVICE_UNAVAILABLE'
  | 'UNEXPECTED';

export interface ToastDetail {
  message: string;
  tone: ToastTone;
}

export interface UserFacingError {
  code: UiErrorCode;
  title: string;
  message: string;
  retryable: boolean;
  action: 'retry' | 'sign-in' | 'refresh' | 'edit-input' | 'none';
  diagnosticId?: string;
  commandError?: CommandError;
}

export function showToast(message: string, tone: ToastTone = 'info'): void {
  window.dispatchEvent(new CustomEvent<ToastDetail>('onyx:toast', { detail: { message, tone } }));
}

export function notifyNetworkError(): void {
  window.dispatchEvent(new Event('onyx:network-error'));
}

const statusErrors: Record<number, Omit<UserFacingError, 'diagnosticId' | 'commandError'>> = {
  400: { code: 'VALIDATION_FAILED', title: 'Check the information provided', message: 'The request could not be validated. Review the information and try again.', retryable: false, action: 'edit-input' },
  401: { code: 'SESSION_EXPIRED', title: 'Sign in again', message: 'Your session has expired. Sign in again to continue.', retryable: false, action: 'sign-in' },
  403: { code: 'PERMISSION_DENIED', title: 'Permission required', message: 'The backend refused this action for this client class or account.', retryable: false, action: 'none' },
  404: { code: 'RESOURCE_NOT_FOUND', title: 'Resource unavailable', message: 'The requested resource is no longer available. Refresh and try again.', retryable: true, action: 'refresh' },
  429: { code: 'SERVICE_UNAVAILABLE', title: 'Try again later', message: 'Rate limited. Try again later.', retryable: true, action: 'retry' },
  500: { code: 'SERVICE_UNAVAILABLE', title: 'Service unavailable', message: 'The server could not complete the request. Try again later.', retryable: true, action: 'retry' },
};

function unexpectedError(): UserFacingError {
  return { code: 'UNEXPECTED', title: 'Something went wrong', message: 'The action could not be completed. Try again, or contact an administrator if the problem continues.', retryable: true, action: 'retry' };
}

export function normalizeError(error: unknown): UserFacingError {
  if (!axios.isAxiosError(error)) return unexpectedError();

  const status = error.response?.status;
  const commandError = error.response?.data?.error as CommandError | undefined;
  const diagnosticId = typeof commandError?.correlation_id === 'string' ? commandError.correlation_id : undefined;
  if (!error.response) {
    notifyNetworkError();
    return {
      code: 'NETWORK_UNAVAILABLE',
      title: 'Connection lost',
      message: 'Check your network connection, then try again.',
      retryable: true,
      action: 'retry',
      diagnosticId,
      commandError,
    };
  }

  const base = statusErrors[status ?? 500] ?? unexpectedError();
  const safeMessage = commandError?.safe_details?.message;
  return {
    ...base,
    message: typeof safeMessage === 'string' ? safeMessage : base.message,
    diagnosticId,
    commandError,
  };
}