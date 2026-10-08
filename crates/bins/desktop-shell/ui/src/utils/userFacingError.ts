import { isShellError, type ShellError } from "@/types/onyx";

export type UiErrorCode =
  | "NETWORK_UNAVAILABLE"
  | "SESSION_UNAVAILABLE"
  | "INVALID_CREDENTIALS"
  | "PERMISSION_DENIED"
  | "CONFLICT"
  | "VALIDATION_FAILED"
  | "SERVICE_UNAVAILABLE"
  | "UNEXPECTED";

export interface UserFacingError {
  code: UiErrorCode;
  title: string;
  message: string;
  retryable: boolean;
  action: "retry" | "sign-in" | "refresh" | "edit-input" | "none";
}

const unexpected: UserFacingError = {
  code: "UNEXPECTED",
  title: "Something went wrong",
  message: "The action could not be completed. Try again, or contact an administrator if the problem continues.",
  retryable: true,
  action: "retry",
};

function fromShellError(error: ShellError): UserFacingError {
  switch (error.kind) {
    case "storage":
      return { code: "SESSION_UNAVAILABLE", title: "Secure session unavailable", message: "The secure session store is unavailable on this device. Confirm the device credential service is available, then retry.", retryable: true, action: "retry" };
    case "auth":
      return { code: "INVALID_CREDENTIALS", title: "Sign-in could not be completed", message: "Check your username, password, and server address, then try again.", retryable: true, action: "edit-input" };
    case "invalidArgument":
      return { code: "VALIDATION_FAILED", title: "Check the information provided", message: "The information could not be validated. Review it and try again.", retryable: false, action: "edit-input" };
    case "query":
      return { code: "SERVICE_UNAVAILABLE", title: "Information unavailable", message: "The current information could not be loaded. Check the connection and retry.", retryable: true, action: "retry" };
    case "command":
      return { code: "SERVICE_UNAVAILABLE", title: "Action could not be completed", message: "The server could not complete this action. Refresh and try again.", retryable: true, action: "refresh" };
    default:
      return unexpected;
  }
}

export function toUserFacingError(error: unknown): UserFacingError {
  return isShellError(error) ? fromShellError(error) : unexpected;
}

export function userFacingMessage(error: unknown): string {
  return toUserFacingError(error).message;
}
