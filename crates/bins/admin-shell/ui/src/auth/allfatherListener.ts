export type AllfatherListenerStatus = "checking" | "ready" | "failed";
export type AllfatherListenerAction = "retry" | "registered" | "registration-failed";

export function allfatherListenerReducer(
  _status: AllfatherListenerStatus,
  action: AllfatherListenerAction,
): AllfatherListenerStatus {
  switch (action) {
    case "registered":
      return "ready";
    case "registration-failed":
      return "failed";
    case "retry":
      return "checking";
  }
}

/** Fail closed: desktop sign-in is enabled only after event registration. */
export function canStartAllfatherSignIn(status: AllfatherListenerStatus): boolean {
  return status === "ready";
}
