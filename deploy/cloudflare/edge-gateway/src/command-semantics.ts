/**
 * Server-side command semantics for the Cloudflare edge gateway.
 *
 * Role policy alone is not enough: every allowed command must also validate
 * the resource's current lifecycle state and a strict payload schema. Commands
 * requiring domain-specific relationship checks that this gateway cannot yet
 * prove are deliberately disabled rather than approximated.
 */
type PayloadKind = "string" | "boolean" | "array" | "object" | "timestamp";
interface PayloadField {
  kind: PayloadKind;
  required?: boolean;
  nonEmpty?: boolean;
  oneOf?: readonly string[];
}
type PayloadSchema = Readonly<Record<string, PayloadField>>;
type CommandSemantics = {
  payload: PayloadSchema;
  createStatus?: string;
  from?: readonly string[];
  to?: string;
  stay?: readonly string[];
  disabled?: string;
  special?: "create-blueprint-revision" | "submit-blueprint" | "request-approval" | "reopen-task" | "task-add-dependency" | "todo-add-item" | "team-precheck" |
    "policy-create-version" | "policy-publish-version" | "policy-requires-published" | "policy-scope" | "policy-evaluate" | "policy-register-violation" | "todo-escalation";
  createPatch?: Record<string, unknown>;
};
export type SemanticDecision =
  | { allowed: true; nextStatus: string; statePatch?: Record<string, unknown>; commandResult?: Record<string, unknown> }
  | { allowed: false; reason: string };

const string = (required = true): PayloadField => ({ kind: "string", required, nonEmpty: required });
const bool = (required = true): PayloadField => ({ kind: "boolean", required });
const array = (required = true, nonEmpty = false): PayloadField => ({ kind: "array", required, nonEmpty });
const object = (required = true): PayloadField => ({ kind: "object", required });
const timestamp = (): PayloadField => ({ kind: "timestamp", required: true });
const noPayload: PayloadSchema = {};

const TASK_NON_TERMINAL = ["Draft", "Ready", "Active", "Paused", "Blocked", "Reopened"] as const;
const TASK_SELF_LOOP = ["Draft", "Ready", "Active", "Reopened"] as const;
const MISSION_NON_TERMINAL = ["Draft", "Planning", "AwaitingApproval", "Active", "Paused", "Halted", "Review"] as const;
const MISSION_REVISION_ALLOWED = MISSION_NON_TERMINAL;
const LIST_PENDING = ["Submitted", "TeamLeaderPreChecked", "Escalated"] as const;

export const COMMAND_SEMANTICS: Readonly<Record<string, CommandSemantics>> = {
  "notification.Acknowledge": { payload: noPayload, from: ["unacknowledged"], to: "acknowledged" },
  "Acknowledge": { payload: noPayload, from: ["unacknowledged"], to: "acknowledged" },

  "CreateTask": { payload: { mission_id: string(), title: string(), owner_id: string(), description: string(false) }, createStatus: "Draft", createPatch: { priority: "Normal", dependencies: [] } },
  "MarkReady": { payload: { reason: string() }, from: ["Draft"], to: "Ready" },
  "AssignOwner": { payload: { owner_id: string(), reason: string() }, stay: TASK_SELF_LOOP },
  "ChangePriority": { payload: { priority: { kind: "string", required: true, nonEmpty: true, oneOf: ["Low", "Normal", "High", "Urgent"] }, reason: string() }, stay: TASK_SELF_LOOP },
  "AddDependency": { payload: { dependency: object() }, stay: TASK_SELF_LOOP, special: "task-add-dependency" },
  "StartTask": { payload: { reason: string() }, from: ["Ready", "Reopened"], to: "Active" },
  "PauseTask": { payload: { reason: string() }, from: ["Active"], to: "Paused" },
  "BlockTask": { payload: { reason: string(), blocking_ref: string(false) }, from: ["Active"], to: "Blocked" },
  "ResumeTask": { payload: { reason: string() }, from: ["Paused"], to: "Active" },
  "UnblockTask": { payload: { reason: string() }, from: ["Blocked"], to: "Active" },
  "SubmitCompletion": { payload: { evidence: array(true, true) }, from: ["Active"], to: "Submitted" },
  "RejectTask": { payload: { reason: string() }, from: ["Submitted"], to: "Active" },
  "ApproveTask": { payload: { reason: string() }, from: ["Submitted"], to: "Approved" },
  "CloseTask": { payload: { reason: string() }, from: ["Approved"], to: "Closed" },
  "ReopenTask": { payload: { reason: string() }, from: ["Closed"], to: "Reopened", special: "reopen-task" },
  "CancelTask": { payload: { reason: string() }, from: TASK_NON_TERMINAL, to: "Cancelled" },

  "CreateMission": { payload: { name: string(), owner_id: string(), description: string(false) }, createStatus: "Draft", createPatch: { blueprint_revision_id: null, submitted_blueprint_revision_id: null } },
  "CreateBlueprintRevision": {
    payload: { blueprint_data: string(), version: string() }, stay: MISSION_REVISION_ALLOWED,
    special: "create-blueprint-revision",
  },
  "SubmitBlueprint": {
    payload: { revision_id: string() }, from: ["Draft"], to: "Planning", special: "submit-blueprint",
  },
  "RequestApproval": {
    payload: { reason: string(), evidence: array() }, from: ["Planning"], to: "AwaitingApproval", special: "request-approval",
  },
  "RejectApproval": { payload: { reason: string() }, from: ["AwaitingApproval"], to: "Planning" },
  "ActivateMission": { payload: { reason: string() }, from: ["Planning", "AwaitingApproval", "Review"], to: "Active" },
  "PauseMission": { payload: { reason: string() }, from: ["Planning", "Active"], to: "Paused" },
  "OperationalHaltMission": { payload: { reason: string(), emergency_code: string() }, from: ["Active", "Paused"], to: "Halted" },
  "ResumeMission": { payload: { reason: string() }, from: ["Paused"], to: "Active" },
  "RestartMission": { payload: { reason: string(), checklist_completed: bool() }, from: ["Halted"] },
  "TriggerReview": { payload: { reason: string() }, from: ["Active"], to: "Review" },
  "CloseMission": { payload: { reason: string(), closure_evidence: array(true, true) }, from: ["Active", "Review"], to: "Closed" },
  "ArchiveMission": { payload: { reason: string() }, from: ["Closed", "Cancelled"], to: "Archived" },
  "CancelMission": { payload: { reason: string() }, from: MISSION_NON_TERMINAL, to: "Cancelled" },

  "approval.Approve": { payload: { reason: string(false) }, from: ["pending"], to: "approved" },
  "approval.Reject": { payload: { reason: string() }, from: ["pending"], to: "rejected" },

  "todo_list.AddItem": { payload: { item: object() }, stay: ["Draft"], special: "todo-add-item" },
  "todo_list.SubmitTodoList": { payload: noPayload, from: ["Draft"], to: "Submitted" },
  "todo_list.RecordTeamLeaderPreCheck": { payload: { notes: string() }, from: ["Submitted"], to: "TeamLeaderPreChecked", special: "team-precheck" },
  "todo_list.VerifyTodoList": { payload: { outcome: { kind: "string", required: true, nonEmpty: true, oneOf: ["Flawless", "WithDeficiencies"] }, comment: string(false) }, from: LIST_PENDING, to: "Verified" },
  "todo_list.RejectTodoList": { payload: { reason: string() }, from: LIST_PENDING, to: "Rejected" },
  "todo_list.EscalateTodoList": { payload: { reason: string(), escalated_to: string(false) }, stay: LIST_PENDING, special: "todo-escalation" },

  "target_list.SubmitTargetList": { payload: noPayload, from: ["Draft"], to: "Submitted" },
  "target_list.RecordTeamLeaderPreCheck": { payload: { notes: string() }, from: ["Submitted"], to: "TeamLeaderPreChecked", special: "team-precheck" },
  "target_list.VerifyTargetList": { payload: { outcome: { kind: "string", required: true, nonEmpty: true, oneOf: ["Flawless", "WithDeficiencies"] }, comment: string(false) }, from: LIST_PENDING, to: "Verified" },
  "target_list.RejectTargetList": { payload: { reason: string() }, from: LIST_PENDING, to: "Rejected" },
  "target_list.EscalateTargetList": { payload: { reason: string(), escalated_to: string(false) }, stay: LIST_PENDING, special: "todo-escalation" },

  // These commands have actor-specific real-owner / borrowed-staff approval
  // requirements in the native domain model. The Worker cannot safely prove
  // those relationships from its present schema, so fail closed until parity
  // is implemented; never substitute the broad ORGANIZATION_ADMIN role.
  "staff_loan.ApproveStaffLoan": { payload: noPayload, stay: ["Requested", "Active", "Extended"], special: "staff-loan-approve" },
  "staff_loan.DeclineStaffLoan": { payload: { reason: string() }, stay: ["Requested", "Active", "Extended"], special: "staff-loan-decline" },
  "staff_loan.ExtendStaffLoan": { payload: { new_end_at: timestamp() }, stay: ["Active", "Extended"], special: "staff-loan-extend" },
  "staff_loan.EndStaffLoanEarly": { payload: noPayload, from: ["Active", "Extended"], to: "Ended", special: "staff-loan-end-early" },

  "CreatePolicy": { payload: { name: string(), scope: object() }, createStatus: "Active", createPatch: { versions: [] }, special: "policy-scope" },
  "CreatePolicyVersion": { payload: { rules: array() }, stay: ["Active"], special: "policy-create-version" },
  "PublishPolicyVersion": { payload: noPayload, stay: ["Active"], special: "policy-publish-version" },
  "EvaluatePolicy": { payload: { rule_key: string() }, stay: ["Active"], special: "policy-evaluate" },
  "RegisterViolation": { payload: { rule_key: string(), reason: string() }, stay: ["Active"], special: "policy-register-violation" },
  "RetirePolicy": { payload: noPayload, from: ["Active"], to: "Retired" },

  "ApplyLegalHold": { payload: { target: object(), reason: string(), authorizing_policy: string(false) }, createStatus: "Applied" },
  "ReleaseLegalHold": { payload: { reason: string() }, from: ["Applied"], to: "Released" },
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function validatePayload(schema: PayloadSchema, payload: unknown): string | null {
  // Commands with no payload fields are represented by arbitrary serde_json::Value
  // on the Axum API; the Worker must not reject string/number/bool/null solely
  // because the transport value is not an object.
  if (Object.keys(schema).length === 0) return null;
  if (!isRecord(payload)) return "INVALID_COMMAND_PAYLOAD";
  for (const key of Object.keys(payload)) {
    if (!Object.prototype.hasOwnProperty.call(schema, key)) return "INVALID_COMMAND_PAYLOAD";
  }
  for (const [key, rule] of Object.entries(schema)) {
    const value = payload[key];
    if (value === undefined || value === null) {
      if (rule.required) return "INVALID_COMMAND_PAYLOAD";
      continue;
    }
    const typeOkay =
      rule.kind === "array" ? Array.isArray(value) :
      rule.kind === "object" ? isRecord(value) :
      rule.kind === "timestamp" ? (typeof value === "number" && Number.isFinite(value) && value >= 0) ||
        (typeof value === "string" && Number.isFinite(Date.parse(value))) :
      rule.kind === "string" ? typeof value === "string" :
      typeof value === "boolean";
    if (!typeOkay) return "INVALID_COMMAND_PAYLOAD";
    if (rule.kind === "string" && rule.nonEmpty && !(value as string).trim()) return "INVALID_COMMAND_PAYLOAD";
    if (rule.kind === "array" && rule.nonEmpty && (value as unknown[]).length === 0) return "INVALID_COMMAND_PAYLOAD";
    if (rule.oneOf && !rule.oneOf.includes(String(value))) return "INVALID_COMMAND_PAYLOAD";
  }
  return null;
}

function sameReference(left: unknown, right: unknown): boolean {
  if (typeof left === "string" && typeof right === "string") return left === right;
  // Rust ObjectId/UserId values may round-trip in JSON as a 16-byte array.
  if (Array.isArray(left) && Array.isArray(right) && left.length === 16 && right.length === 16)
    return left.every((value, index) => value === right[index]);
  return false;
}

export function validateCommandSemantics(
  commandType: string,
  existingState: unknown,
  payload: unknown,
  actorId?: string,
): SemanticDecision {
  const semantics = COMMAND_SEMANTICS[commandType];
  if (!semantics) return { allowed: false, reason: "UNKNOWN_COMMAND" };
  if (semantics.disabled) return { allowed: false, reason: "COMMAND_PRECONDITION_UNSUPPORTED" };
  const payloadError = validatePayload(semantics.payload, payload);
  if (payloadError) return { allowed: false, reason: payloadError };

  const current = isRecord(existingState) ? existingState : null;
  if (semantics.createStatus) {
    if (current) return { allowed: false, reason: "RESOURCE_ALREADY_EXISTS" };
    if (semantics.special === "policy-scope") {
      const scope = (payload as Record<string, unknown>).scope;
      const scopeKeys = isRecord(scope) ? Object.keys(scope) : [];
      if (scopeKeys.length !== 1 || !["Organization", "Conversation"].includes(scopeKeys[0]) ||
          !(typeof (scope as Record<string, unknown>)[scopeKeys[0]] === "string" ||
            (Array.isArray((scope as Record<string, unknown>)[scopeKeys[0]]) && ((scope as Record<string, unknown>)[scopeKeys[0]] as unknown[]).length === 16))) {
        return { allowed: false, reason: "INVALID_COMMAND_PAYLOAD" };
      }
    }
    return { allowed: true, nextStatus: semantics.createStatus, statePatch: semantics.createPatch };
  }
  if (!current || typeof current.status !== "string") {
    return { allowed: false, reason: "RESOURCE_STATE_UNVERIFIED" };
  }
  const status = current.status;
  if (semantics.from && !semantics.from.includes(status)) {
    return { allowed: false, reason: "INVALID_STATE_TRANSITION" };
  }
  if (semantics.stay && !semantics.stay.includes(status)) {
    return { allowed: false, reason: "INVALID_STATE_TRANSITION" };
  }
  if (commandType === "approval.Approve" || commandType === "approval.Reject") {
    if (current.web_action_permitted !== true) return { allowed: false, reason: "COMMAND_PRECONDITION_UNSUPPORTED" };
  }
  if (semantics.special === "reopen-task") {
    if (!actorId) return { allowed: false, reason: "COMMAND_PRECONDITION_UNSUPPORTED" };
    return { allowed: true, nextStatus: "Reopened", statePatch: { authorized_by: actorId } };
  }
  if (semantics.special === "task-add-dependency") {
    const dependency = (payload as Record<string, unknown>).dependency;
    if (!isRecord(dependency) || Object.keys(dependency).length !== 2 ||
        !Object.prototype.hasOwnProperty.call(dependency, "task_id") ||
        !["FinishToStart", "StartToStart", "FinishToFinish"].includes(String(dependency.dependency_type)) ||
        !(typeof dependency.task_id === "string" || (Array.isArray(dependency.task_id) && dependency.task_id.length === 16))) {
      return { allowed: false, reason: "INVALID_COMMAND_PAYLOAD" };
    }
    const dependencies = Array.isArray(current.dependencies) ? current.dependencies : [];
    if (dependencies.some((existingDependency) => JSON.stringify(existingDependency) === JSON.stringify(dependency))) {
      return { allowed: false, reason: "INVALID_STATE_TRANSITION" };
    }
    return { allowed: true, nextStatus: status, statePatch: { dependencies: [...dependencies, dependency] } };
  }
  if (semantics.special === "todo-add-item") {
    const item = (payload as Record<string, unknown>).item;
    if (!isRecord(item) || Object.keys(item).some((key) => key !== "item_id" && key !== "description") ||
        typeof item.item_id !== "string" || !item.item_id.trim() ||
        typeof item.description !== "string" || !item.description.trim() ||
        !Array.isArray(current.items)) {
      return { allowed: false, reason: "COMMAND_PRECONDITION_UNSUPPORTED" };
    }
    if ((current.items as unknown[]).some((existingItem) => isRecord(existingItem) && existingItem.item_id === item.item_id)) {
      return { allowed: false, reason: "INVALID_STATE_TRANSITION" };
    }
    return { allowed: true, nextStatus: status, statePatch: { items: [...current.items, item] } };
  }
  if (semantics.special === "staff-loan-approve") {
    if (status === "Requested") return {
      allowed: true, nextStatus: "Active",
      statePatch: { approved_by: actorId ?? null, approved_at: new Date().toISOString() },
    };
    const pendingEnd = current.pending_extension_end_at;
    if ((status === "Active" || status === "Extended") &&
        (typeof pendingEnd === "number" || (typeof pendingEnd === "string" && pendingEnd.trim()))) {
      const window = isRecord(current.window) ? current.window : {};
      return {
        allowed: true, nextStatus: "Extended",
        statePatch: {
          window: { ...window, end_at: pendingEnd },
          pending_extension_end_at: null,
          extension_approved_by: actorId ?? null,
          extension_approved_at: new Date().toISOString(),
        },
      };
    }
    return { allowed: false, reason: "INVALID_STATE_TRANSITION" };
  }
  if (semantics.special === "staff-loan-decline") {
    const reason = (payload as Record<string, unknown>).reason as string;
    if (status === "Requested") return {
      allowed: true, nextStatus: "Declined",
      statePatch: { decline_reason: reason, declined_by: actorId ?? null, declined_at: new Date().toISOString() },
    };
    if ((status === "Active" || status === "Extended") && current.pending_extension_end_at != null) return {
      allowed: true, nextStatus: status,
      statePatch: {
        pending_extension_end_at: null,
        extension_decline_reason: reason,
        extension_declined_by: actorId ?? null,
        extension_declined_at: new Date().toISOString(),
      },
    };
    return { allowed: false, reason: "INVALID_STATE_TRANSITION" };
  }
  if (semantics.special === "staff-loan-extend") {
    const requestedEnd = (payload as Record<string, unknown>).new_end_at;
    const requestedNs = typeof requestedEnd === "number" ? requestedEnd :
      typeof requestedEnd === "string" ? Date.parse(requestedEnd) * 1_000_000 : NaN;
    const window = isRecord(current.window) ? current.window : null;
    const existingEnd = window?.end_at;
    const existingNs = typeof existingEnd === "number" ? existingEnd :
      typeof existingEnd === "string" ? Date.parse(existingEnd) * 1_000_000 : NaN;
    if (!Number.isFinite(requestedNs) || !Number.isFinite(existingNs) || requestedNs <= existingNs ||
        current.pending_extension_end_at != null)
      return { allowed: false, reason: "INVALID_COMMAND_PAYLOAD" };
    return {
      allowed: true, nextStatus: status,
      statePatch: {
        pending_extension_end_at: requestedNs,
        extension_requested_by: actorId ?? null,
        extension_requested_at: new Date().toISOString(),
      },
    };
  }
  if (semantics.special === "staff-loan-end-early") {
    return {
      allowed: true, nextStatus: "Ended",
      statePatch: { ended_by: actorId ?? null, ended_at: new Date().toISOString() },
    };
  }
  if (semantics.special === "team-precheck") {
    return { allowed: true, nextStatus: "TeamLeaderPreChecked", statePatch: { team_leader_precheck_notes: (payload as Record<string, unknown>).notes } };
  }
  if (semantics.special === "todo-escalation") {
    const escalatedTo = (payload as Record<string, unknown>).escalated_to;
    if (typeof escalatedTo !== "string" || !escalatedTo.trim()) {
      return { allowed: false, reason: "COMMAND_PRECONDITION_UNSUPPORTED" };
    }
    return { allowed: true, nextStatus: "Escalated", statePatch: { escalated_to: escalatedTo } };
  }
  if (commandType === "AssignOwner") {
    return { allowed: true, nextStatus: status, statePatch: { owner_id: (payload as Record<string, unknown>).owner_id } };
  }
  if (commandType === "ChangePriority") {
    return { allowed: true, nextStatus: status, statePatch: { priority: (payload as Record<string, unknown>).priority } };
  }
  if (commandType === "BlockTask") {
    const blockingRef = (payload as Record<string, unknown>).blocking_ref;
    return { allowed: true, nextStatus: status, statePatch: blockingRef ? { blocking_ref: blockingRef } : {} };
  }
  if (commandType === "SubmitCompletion") {
    return { allowed: true, nextStatus: "Submitted", statePatch: { completion_evidence: (payload as Record<string, unknown>).evidence } };
  }
  if (commandType === "RequestApproval") {
    return { allowed: true, nextStatus: "AwaitingApproval", statePatch: { approval_reason: (payload as Record<string, unknown>).reason, approval_evidence: (payload as Record<string, unknown>).evidence } };
  }
  if (commandType === "CloseMission") {
    return { allowed: true, nextStatus: "Closed", statePatch: { closure_reason: (payload as Record<string, unknown>).reason, closure_evidence: (payload as Record<string, unknown>).closure_evidence } };
  }
  if (commandType === "todo_list.VerifyTodoList" || commandType === "target_list.VerifyTargetList") {
    return { allowed: true, nextStatus: "Verified", statePatch: { outcome: (payload as Record<string, unknown>).outcome, verification_comment: (payload as Record<string, unknown>).comment } };
  }
  if (semantics.special === "create-blueprint-revision") {
    return {
      allowed: true,
      nextStatus: status,
      statePatch: {
        blueprint_revision_id: crypto.randomUUID(),
        blueprint_data: (payload as Record<string, unknown>).blueprint_data,
        blueprint_version: (payload as Record<string, unknown>).version,
      },
    };
  }
  if (semantics.special === "submit-blueprint") {
    if (!sameReference((payload as Record<string, unknown>).revision_id, current.blueprint_revision_id))
      return { allowed: false, reason: "COMMAND_PRECONDITION_UNSUPPORTED" };
    return { allowed: true, nextStatus: "Planning", statePatch: { submitted_blueprint_revision_id: (payload as Record<string, unknown>).revision_id } };
  }
  if (semantics.special === "request-approval" && !current.submitted_blueprint_revision_id) {
    return { allowed: false, reason: "COMMAND_PRECONDITION_UNSUPPORTED" };
  }
  if (semantics.special === "policy-create-version") {
    const versions = Array.isArray(current.versions) ? current.versions.filter(isRecord) : null;
    if (!versions || versions.some((version) => version.status === "Draft")) {
      return { allowed: false, reason: "COMMAND_PRECONDITION_UNSUPPORTED" };
    }
    return {
      allowed: true,
      nextStatus: status,
      statePatch: { versions: [...versions, {
        version_number: versions.length + 1,
        status: "Draft",
        rules: (payload as Record<string, unknown>).rules,
      }] },
    };
  }
  if (semantics.special === "policy-publish-version") {
    const versions = Array.isArray(current.versions) ? current.versions.filter(isRecord) : null;
    const draftIndex = versions?.findIndex((version) => version.status === "Draft") ?? -1;
    if (!versions || draftIndex < 0) return { allowed: false, reason: "COMMAND_PRECONDITION_UNSUPPORTED" };
    const nextVersions = versions.map((version, index) => index === draftIndex ? { ...version, status: "Published" } : version);
    return { allowed: true, nextStatus: status, statePatch: { versions: nextVersions } };
  }
  if (semantics.special === "policy-evaluate" || semantics.special === "policy-register-violation") {
    const versions = Array.isArray(current.versions) ? current.versions.filter(isRecord) : [];
    const published = [...versions].reverse().find((version) => version.status === "Published");
    if (!published) return { allowed: false, reason: "NO_PUBLISHED_VERSION" };
    const ruleKey = (payload as Record<string, unknown>).rule_key as string;
    if (semantics.special === "policy-evaluate") {
      const rules = Array.isArray(published.rules) ? published.rules.filter(isRecord) : [];
      const matchingRule = rules.find((rule) => rule.key === ruleKey);
      return {
        allowed: true,
        nextStatus: status,
        commandResult: {
          rule_key: ruleKey,
          result: matchingRule && Object.prototype.hasOwnProperty.call(matchingRule, "value") ? matchingRule.value : null,
          evaluated_version: published.version_number,
        },
      };
    }
    // Rust records violations against the active published policy without
    // requiring the supplied key to exist in that policy's rule list.
    return {
      allowed: true,
      nextStatus: status,
      commandResult: {
        recorded: true,
        rule_key: ruleKey,
        reason: (payload as Record<string, unknown>).reason,
        policy_version: published.version_number,
      },
    };
  }
  if (semantics.special === "policy-requires-published") {
    const versions = Array.isArray(current.versions) ? current.versions.filter(isRecord) : [];
    if (!versions.some((version) => version.status === "Published")) {
      return { allowed: false, reason: "COMMAND_PRECONDITION_UNSUPPORTED" };
    }
  }
  let nextStatus = semantics.to ?? status;
  if (commandType === "RestartMission") {
    nextStatus = (payload as Record<string, unknown>).checklist_completed === true ? "Active" : "Paused";
  }
  return { allowed: true, nextStatus };
}
