/**
 * Fail-closed authorization policy for the Cloudflare Worker.
 *
 * Policy inputs are derived from a verified ONYX session and D1-scoped
 * resources. Client labels (including client_type/mobile/mobile_observer)
 * are intentionally absent from this module.
 */
export const ONYX_ROLES = ["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"] as const;
export type OnyxRole = (typeof ONYX_ROLES)[number];

export type CommandEffect = "create" | "update";
export interface CommandPolicy {
  targetType: string;
  roles: readonly OnyxRole[];
  effect: CommandEffect;
  /** STAFF must own the existing aggregate before an update. */
  staffMustOwn?: boolean;
  /** Creation commands must name an active owner in the current tenant. */
  ownerRequiredOnCreate?: boolean;
}

/**
 * Only actions with explicit role, resource and lifecycle policy are listed.
 * Unlisted actions, especially free-form delete/remove names, are denied.
 * The Worker currently does not expose generic aggregate deletion.
 */
export const COMMAND_POLICIES: Readonly<Record<string, CommandPolicy>> = {
  "notification.Acknowledge": { targetType: "notification", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update", staffMustOwn: true },
  "Acknowledge": { targetType: "notification", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update", staffMustOwn: true },

  "CreateTask": { targetType: "task", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "create", staffMustOwn: true, ownerRequiredOnCreate: true },
  "MarkReady": { targetType: "task", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update", staffMustOwn: true },
  "ChangePriority": { targetType: "task", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update", staffMustOwn: true },
  "AddDependency": { targetType: "task", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update", staffMustOwn: true },
  "StartTask": { targetType: "task", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update", staffMustOwn: true },
  "PauseTask": { targetType: "task", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update", staffMustOwn: true },
  "BlockTask": { targetType: "task", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update", staffMustOwn: true },
  "ResumeTask": { targetType: "task", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update", staffMustOwn: true },
  "UnblockTask": { targetType: "task", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update", staffMustOwn: true },
  "SubmitCompletion": { targetType: "task", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update", staffMustOwn: true },
  "AssignOwner": { targetType: "task", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "RejectTask": { targetType: "task", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "ApproveTask": { targetType: "task", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "CloseTask": { targetType: "task", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "ReopenTask": { targetType: "task", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "CancelTask": { targetType: "task", roles: ["ORGANIZATION_ADMIN"], effect: "update" },

  "CreateMission": { targetType: "mission", roles: ["ORGANIZATION_ADMIN"], effect: "create", ownerRequiredOnCreate: true },
  "CreateBlueprintRevision": { targetType: "mission", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "SubmitBlueprint": { targetType: "mission", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "RequestApproval": { targetType: "mission", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "RejectApproval": { targetType: "mission", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "ActivateMission": { targetType: "mission", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "PauseMission": { targetType: "mission", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "OperationalHaltMission": { targetType: "mission", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "ResumeMission": { targetType: "mission", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "RestartMission": { targetType: "mission", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "TriggerReview": { targetType: "mission", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "CloseMission": { targetType: "mission", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "ArchiveMission": { targetType: "mission", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "CancelMission": { targetType: "mission", roles: ["ORGANIZATION_ADMIN"], effect: "update" },

  "approval.Approve": { targetType: "approval", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "approval.Reject": { targetType: "approval", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "todo_list.AddItem": { targetType: "todo_list", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update", staffMustOwn: true },
  "todo_list.SubmitTodoList": { targetType: "todo_list", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update", staffMustOwn: true },
  "todo_list.RecordTeamLeaderPreCheck": { targetType: "todo_list", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update" },
  "todo_list.VerifyTodoList": { targetType: "todo_list", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update" },
  "todo_list.RejectTodoList": { targetType: "todo_list", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update" },
  "todo_list.EscalateTodoList": { targetType: "todo_list", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update" },
  "target_list.SubmitTargetList": { targetType: "target_list", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update", staffMustOwn: true },
  "target_list.RecordTeamLeaderPreCheck": { targetType: "target_list", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update" },
  "target_list.VerifyTargetList": { targetType: "target_list", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update" },
  "target_list.RejectTargetList": { targetType: "target_list", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update" },
  "target_list.EscalateTargetList": { targetType: "target_list", roles: ["STAFF", "ORGANIZATION_ADMIN"], effect: "update" },
  "staff_loan.ApproveStaffLoan": { targetType: "staff_loan", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "staff_loan.DeclineStaffLoan": { targetType: "staff_loan", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "staff_loan.ExtendStaffLoan": { targetType: "staff_loan", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "staff_loan.EndStaffLoanEarly": { targetType: "staff_loan", roles: ["ORGANIZATION_ADMIN"], effect: "update" },

  "CreatePolicy": { targetType: "policy", roles: ["ORGANIZATION_ADMIN"], effect: "create" },
  "CreatePolicyVersion": { targetType: "policy", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "PublishPolicyVersion": { targetType: "policy", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "EvaluatePolicy": { targetType: "policy", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "RegisterViolation": { targetType: "policy", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "RetirePolicy": { targetType: "policy", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
  "ApplyLegalHold": { targetType: "legal_hold", roles: ["ORGANIZATION_ADMIN"], effect: "create" },
  "ReleaseLegalHold": { targetType: "legal_hold", roles: ["ORGANIZATION_ADMIN"], effect: "update" },
};

export type CommandDecision =
  | { allowed: true; policy: CommandPolicy }
  | { allowed: false; reason: "UNKNOWN_COMMAND" | "TARGET_TYPE_MISMATCH" | "ROLE_DENIED" };

export function authorizeCommand(role: string, commandType: string, targetType: string): CommandDecision {
  const policy = COMMAND_POLICIES[commandType];
  if (!policy) return { allowed: false, reason: "UNKNOWN_COMMAND" };
  if (policy.targetType !== targetType) return { allowed: false, reason: "TARGET_TYPE_MISMATCH" };
  if (!policy.roles.includes(role as OnyxRole)) return { allowed: false, reason: "ROLE_DENIED" };
  return { allowed: true, policy };
}

const QUERY_ROLES: Readonly<Record<string, readonly OnyxRole[]>> = {
  "dashboard.summary": ["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"],
  "mission.list": ["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"],
  "mission.detail": ["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"],
  "task.list": ["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"],
  "task.detail": ["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"],
  "timeline.list": ["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"],
  "notification.list": ["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"],
  "approval.list": ["ORGANIZATION_ADMIN"],
  "report.detail": ["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"],
  "policy.list": ["ORGANIZATION_ADMIN"],
  "policy.detail": ["ORGANIZATION_ADMIN"],
  "legal_hold.list": ["ORGANIZATION_ADMIN"],
  "legal_hold.detail": ["ORGANIZATION_ADMIN"],
  "todo_list.list": ["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"],
  "todo_list.detail": ["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"],
  "target_list.list": ["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"],
  "target_list.detail": ["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"],
  "staff_loan.list": ["ORGANIZATION_ADMIN"],
  "staff_loan.detail": ["ORGANIZATION_ADMIN"],
};

export function authorizeQuery(role: string, queryType: string): boolean {
  return Boolean(QUERY_ROLES[queryType]?.includes(role as OnyxRole));
}

const OWNER_FIELDS = ["owner_id", "owner_user_id", "assignee_id", "assigned_to", "user_id", "recipient_user_id"] as const;

/** Return explicit ownership IDs only; absence of ownership is not ownership. */
export function resourceOwnedBy(state: unknown, actorId: string): boolean {
  if (!state || typeof state !== "object" || Array.isArray(state)) return false;
  const value = state as Record<string, unknown>;
  const ownerIds = OWNER_FIELDS
    .map((field) => value[field])
    .filter((candidate): candidate is string => typeof candidate === "string" && candidate.length > 0);
  const nestedOwner = value.owner && typeof value.owner === "object" && !Array.isArray(value.owner)
    ? (value.owner as Record<string, unknown>).user_id
    : undefined;
  if (typeof nestedOwner === "string" && nestedOwner.length > 0) ownerIds.push(nestedOwner);
  return ownerIds.length > 0 && ownerIds.every((ownerId) => ownerId === actorId);
}

export function payloadOwnerIds(payload: unknown): string[] {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return [];
  const value = payload as Record<string, unknown>;
  return OWNER_FIELDS
    .map((field) => value[field])
    .filter((candidate): candidate is string => typeof candidate === "string" && candidate.length > 0);
}
