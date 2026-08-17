import type { ActorContext, AuthorityProof } from './command';

export interface QueryEnvelope {
  query_id: string;
  query_type: string;
  schema_version: string;
  organization_id: string;
  actor?: ActorContext;
  authority_proof?: AuthorityProof;
  filters: Record<string, unknown>;
  limit?: number;
  cursor?: string;
  sort_by?: string;
  sort_order?: 'asc' | 'desc';
}

export interface QueryFreshness {
  projection_version: number;
  last_updated_at: string;
  is_stale: boolean;
}

export interface QueryResponse<T = unknown> {
  query_id: string;
  data: T[];
  has_more: boolean;
  next_cursor: string | null;
  total_count?: number;
  freshness: QueryFreshness;
}

export interface VersionedProjection {
  id: string;
  version: number;
  lifecycle_epoch: number;
  authority_epoch: number;
}

export interface MissionSummary extends VersionedProjection {
  name: string;
  summary: string;
  status: string;
  owner: string;
  priority: string;
  progress: number;
  updated_at: string;
}

export interface TaskSummary extends VersionedProjection {
  mission_id: string;
  title: string;
  status: string;
  owner: string;
  priority: string;
  due_at: string;
  updated_at: string;
}

export interface TimelineEntry extends VersionedProjection {
  subject_id: string;
  subject_type: string;
  label: string;
  kind: string;
  at: string;
  status: string;
  updated_at: string;
}

export interface NotificationProjection extends VersionedProjection {
  title: string;
  message: string;
  priority: string;
  status: 'unacknowledged' | 'acknowledged';
  source_id: string;
  source_type: string;
  created_at: string;
  acknowledged_at: string | null;
}

export interface ApprovalProjection extends VersionedProjection {
  title: string;
  description: string;
  status: 'pending' | 'approved' | 'rejected';
  requested_by: string;
  target_id: string;
  target_type: string;
  created_at: string;
  decided_at: string | null;
  decision_reason: string | null;
  web_action_permitted: boolean;
}

export interface ReportProjection extends VersionedProjection {
  title: string;
  status: string;
  subject_id: string;
  subject_type: string;
  author: string;
  submitted_at: string;
  summary: string;
  evidence: Array<{ label: string; file_name: string }>;
  updated_at: string;
}

export interface ActivityItem {
  id: string;
  type: string;
  title: string;
  status: string;
  updated_at: string;
}

export interface DashboardProjection {
  missions: number;
  active_missions: number;
  tasks: number;
  blocked_tasks: number;
  unread_notifications: number;
  pending_approvals: number;
  activity: ActivityItem[];
}

/**
 * Shared status values for `TodoList`/`TargetList` — see
 * `todo_domain::state_machine::ListStatus`. Serialized PascalCase, same
 * as every other Rust enum this workspace's projections expose over
 * HTTP (confirmed live: `"status":"Verified"`, not `"verified"`) —
 * kept as the literal wire values rather than lowercased, since
 * `StatusBadge` normalizes case itself.
 */
export type TodoListStatus =
  | 'Draft'
  | 'Submitted'
  | 'TeamLeaderPreChecked'
  | 'Verified'
  | 'Rejected'
  | 'Escalated';

export type ListOrigin = 'ManagerAssigned' | 'StaffAuthored';

export type VerificationOutcome = 'Flawless' | 'WithDeficiencies';

export interface TodoItemProjection {
  item_id: string;
  description: string;
}

/**
 * A Team Leader pre-check as it appears over `/api/query`. `notes` is
 * `string | undefined` rather than `string | null` because the server
 * *omits* the key entirely for a Staff viewer of their own list (D.5
 * redaction — see `api_server::query_handler::redact_team_leader_pre_check_for_viewer`),
 * rather than sending `null` — the UI must not assume the field's
 * absence means no pre-check happened; `existsButHidden` distinguishes
 * that from "no pre-check recorded at all" (the whole
 * `team_leader_pre_check` object being absent).
 */
export interface TeamLeaderPreCheckProjection {
  checked_by: string;
  checked_at: number;
  notes?: string;
}

export interface TodoListProjection extends VersionedProjection {
  owner: string;
  origin: ListOrigin;
  items: TodoItemProjection[];
  status: TodoListStatus;
  team_leader_pre_check?: TeamLeaderPreCheckProjection;
}

export interface TargetListProjection extends VersionedProjection {
  owner: string;
  origin: ListOrigin;
  description: string;
  time_window: { start_at: number; end_at: number };
  status: TodoListStatus;
  team_leader_pre_check?: TeamLeaderPreCheckProjection;
}

/**
 * Wire status values for `StaffLoan` — see
 * `todo_domain::state_machine::StaffLoanStatus`. Same PascalCase
 * convention as `TodoListStatus` above.
 */
export type StaffLoanStatus = 'Requested' | 'Declined' | 'Active' | 'Extended' | 'Ended' | 'Expired';

export interface StaffLoanProjection extends VersionedProjection {
  staff_user_id: string;
  real_owner_id: string;
  borrowing_manager_id: string;
  window: { start_at: number; end_at: number };
  status: StaffLoanStatus;
}
