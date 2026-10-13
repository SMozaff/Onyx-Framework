import axios from 'axios';
import type { CommandError } from '../types/command';

export type ToastTone = 'info' | 'success' | 'warning' | 'error';
export type UiErrorCode =
  | 'NETWORK_UNAVAILABLE'
  | 'SESSION_EXPIRED'
  | 'INVALID_CREDENTIALS'
  | 'PERMISSION_DENIED'
  | 'CONFLICT'
  | 'VALIDATION_FAILED'
  | 'RESOURCE_NOT_FOUND'
  | 'SERVICE_UNAVAILABLE'
  | 'FEATURE_UNAVAILABLE'
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
  403: { code: 'PERMISSION_DENIED', title: 'Permission required', message: 'You do not have permission to perform this action.', retryable: false, action: 'none' },
  404: { code: 'RESOURCE_NOT_FOUND', title: 'Resource unavailable', message: 'The requested resource is no longer available. Refresh and try again.', retryable: true, action: 'refresh' },
  409: { code: 'CONFLICT', title: 'Information changed', message: 'This item changed while you were working. Refresh and retry.', retryable: true, action: 'refresh' },
  429: { code: 'SERVICE_UNAVAILABLE', title: 'Try again later', message: 'The organization quota has been reached. Try again later.', retryable: true, action: 'retry' },
  500: { code: 'SERVICE_UNAVAILABLE', title: 'Service unavailable', message: 'The server could not complete the request. Try again later.', retryable: true, action: 'retry' },
};

function unexpectedError(): UserFacingError {
  return { code: 'UNEXPECTED', title: 'Something went wrong', message: 'The action could not be completed. Try again, or contact an administrator if the problem continues.', retryable: true, action: 'retry' };
}

export function normalizeError(error: unknown): UserFacingError {
  // The API client converts only explicit ROUTE_NOT_MIGRATED 501s into this
  // typed error. Treat it as a permanent feature gap, never as a retryable outage.
  if (error && typeof error === 'object' &&
      (error as { code?: unknown }).code === 'ROUTE_NOT_MIGRATED') {
    const unavailable = error as { route?: unknown; message?: unknown };
    const route = typeof unavailable.route === 'string' ? unavailable.route : 'requested route';
    return {
      code: 'FEATURE_UNAVAILABLE',
      title: 'Feature not available on this server',
      message: typeof unavailable.message === 'string'
        ? unavailable.message
        : `The feature for ${route} is not available on this server yet.`,
      retryable: false,
      action: 'none',
    };
  }
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

  if (status === 501 && error.response?.data?.error === 'ROUTE_NOT_MIGRATED') {
    const route = typeof error.response.data.route === 'string' ? error.response.data.route : 'requested route';
    return {
      code: 'FEATURE_UNAVAILABLE',
      title: 'Feature not available on this server',
      message: typeof error.response.data.message === 'string'
        ? error.response.data.message
        : `The feature for ${route} is not available on this server yet.`,
      retryable: false,
      action: 'none',
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
