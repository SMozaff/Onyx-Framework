import axios from "axios";
import type { CommandError } from "@/types/command";

export type ToastTone = "info" | "success" | "warning" | "error";
export type UiErrorCode = "NETWORK_UNAVAILABLE" | "SESSION_EXPIRED" | "PERMISSION_DENIED" | "CONFLICT" | "VALIDATION_FAILED" | "RESOURCE_NOT_FOUND" | "SERVICE_UNAVAILABLE" | "UNEXPECTED";

export interface ToastDetail { message: string; tone: ToastTone; }
export interface UserFacingError {
  code: UiErrorCode;
  title: string;
  message: string;
  retryable: boolean;
  action: "retry" | "sign-in" | "refresh" | "edit-input" | "none";
  commandError?: CommandError;
}

export function showToast(message: string, tone: ToastTone = "info"): void {
  window.dispatchEvent(new CustomEvent<ToastDetail>("onyx:toast", { detail: { message, tone } }));
}

export function notifyNetworkError(): void {
  window.dispatchEvent(new Event("onyx:network-error"));
}

const byStatus: Record<number, Omit<UserFacingError, "commandError">> = {
  400: { code: "VALIDATION_FAILED", title: "Check the information provided", message: "The request could not be validated. Review the information and try again.", retryable: false, action: "edit-input" },
  401: { code: "SESSION_EXPIRED", title: "Sign in again", message: "Your session has expired. Sign in again to continue.", retryable: false, action: "sign-in" },
  403: { code: "PERMISSION_DENIED", title: "Permission required", message: "You do not have permission to perform this action.", retryable: false, action: "none" },
  404: { code: "RESOURCE_NOT_FOUND", title: "Resource unavailable", message: "The requested resource is no longer available. Refresh and try again.", retryable: true, action: "refresh" },
  409: { code: "CONFLICT", title: "Information changed", message: "This item changed while you were working. Refresh and retry.", retryable: true, action: "refresh" },
  429: { code: "SERVICE_UNAVAILABLE", title: "Try again later", message: "The organization quota has been reached. Try again later.", retryable: true, action: "retry" },
  500: { code: "SERVICE_UNAVAILABLE", title: "Service unavailable", message: "The server could not complete the request. Try again later.", retryable: true, action: "retry" },
};

export function normalizeError(error: unknown): UserFacingError {
  const fallback: UserFacingError = { code: "UNEXPECTED", title: "Something went wrong", message: "The action could not be completed. Try again, or contact an administrator if the problem continues.", retryable: true, action: "retry" };
  if (!axios.isAxiosError(error)) return fallback;
  const status = error.response?.status;
  const commandError = error.response?.data?.error as CommandError | undefined;
  if (!error.response) {
    notifyNetworkError();
    return { code: "NETWORK_UNAVAILABLE", title: "Connection lost", message: "Check your network connection, then try again.", retryable: true, action: "retry", commandError };
  }
  const base = byStatus[status ?? 500] ?? fallback;
  const safeMessage = commandError?.safe_details?.message;
  return { ...base, message: typeof safeMessage === "string" ? safeMessage : base.message, commandError };
}

/**
 * For pages that keep error state as a plain string (not a full
 * `UserFacingError`): the server's own error `code` prefixed onto its
 * message, e.g. `"USERNAME_TAKEN: That username already exists"`, so the
 * displayed text is diagnosable rather than generic prose.
 */
export function describeError(error: unknown): string {
  const { commandError, message } = normalizeError(error);
  return commandError ? `${commandError.code}: ${message}` : message;
}
