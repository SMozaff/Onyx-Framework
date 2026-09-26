/**
 * Observer projection API surface — the `mobile_observer`-permitted
 * subset of `web-ui/src/types/query.ts`, plus the Phase 1.2 read DTOs.
 * Frozen against `docs/mobile-migration/pwa-observer-contract.md`.
 */

export interface CommandError {
  code: string;
  category: 'DOMAIN' | 'AUTHORITY' | 'CONCURRENCY' | 'INFRASTRUCTURE';
  retryability: 'RETRYABLE' | 'NON_RETRYABLE' | 'TRANSIENT';
  safe_details: Record<string, string | number | boolean>;
  correlation_id: string;
}

export interface QueryEnvelope {
  query_id: string;
  query_type: string;
  schema_version: string;
  organization_id: string;
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