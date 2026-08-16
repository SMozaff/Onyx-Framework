import { useMutation, useQueryClient } from '@tanstack/react-query';
import { apiClient } from '../api/client';
import { executeCommand } from '../api/command';
import type {
  ApprovalProjection,
  ListOrigin,
  NotificationProjection,
  TargetListProjection,
  TodoListProjection,
  VerificationOutcome,
} from '../types/query';
import { normalizeError, showToast } from '../utils/errorHandler';

const organizationId = () => {
  const raw = sessionStorage.getItem('onyx_user');
  return raw ? (JSON.parse(raw) as { organization_id: string }).organization_id : '';
};

export function useAcknowledgeNotification() {
  const client = useQueryClient();
  return useMutation({
    networkMode: 'always',
    mutationFn: (notification: NotificationProjection) =>
      executeCommand({
        command_type: 'notification.Acknowledge',
        target: { id: notification.id, type: 'notification', organization_id: organizationId() },
        expected_version: notification.version,
        expected_lifecycle_epoch: notification.lifecycle_epoch,
        expected_authority_epoch: notification.authority_epoch,
        payload: {},
      }),
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: ['notification.list'] });
      await client.invalidateQueries({ queryKey: ['dashboard.summary'] });
      showToast('Notification acknowledged.', 'success');
    },
    onError: (error) => showToast(normalizeError(error).message, 'error'),
  });
}

export function useApprovalDecision() {
  const client = useQueryClient();
  return useMutation({
    networkMode: 'always',
    mutationFn: ({ approval, decision, reason }: { approval: ApprovalProjection; decision: 'approve' | 'reject'; reason: string }) =>
      executeCommand({
        command_type: decision === 'approve' ? 'approval.Approve' : 'approval.Reject',
        target: { id: approval.id, type: 'approval', organization_id: organizationId() },
        expected_version: approval.version,
        expected_lifecycle_epoch: approval.lifecycle_epoch,
        expected_authority_epoch: approval.authority_epoch,
        payload: { reason },
      }),
    onSuccess: async (_, variables) => {
      await client.invalidateQueries({ queryKey: ['approval.list'] });
      await client.invalidateQueries({ queryKey: ['dashboard.summary'] });
      showToast(`Approval ${variables.decision === 'approve' ? 'granted' : 'rejected'}.`, 'success');
    },
    onError: (error) => showToast(normalizeError(error).message, 'error'),
  });
}

/**
 * `TodoList`/`TargetList` creation — routed through the dedicated REST
 * endpoints (`POST /api/todo/lists`, `POST /api/todo/targets`), not
 * `/api/command`: `CreateTodoList`/`CreateTargetList` are
 * `create()`-routed commands `api_server::routes::command` cannot
 * express — see `api_server::routes::todo_admin`'s module doc comment.
 * Same reason `Settings.tsx`'s `CreatePolicyForm` (admin-shell) posts
 * to `/api/admin/policies` directly rather than through `useCommand`.
 */
export function useCreateTodoList() {
  const client = useQueryClient();
  return useMutation({
    networkMode: 'always',
    mutationFn: (input: { owner: string; origin: ListOrigin; items: { description: string }[] }) =>
      apiClient.post<{ todo_list_id: string }>('/api/todo/lists', input).then((r) => r.data),
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: ['todo_list.list'] });
      showToast('Todo list created.', 'success');
    },
    onError: (error) => showToast(normalizeError(error).message, 'error'),
  });
}

export function useCreateTargetList() {
  const client = useQueryClient();
  return useMutation({
    networkMode: 'always',
    mutationFn: (input: {
      owner: string;
      origin: ListOrigin;
      description: string;
      time_window: { start_at_ms: number; end_at_ms: number };
    }) => apiClient.post<{ target_list_id: string }>('/api/todo/targets', input).then((r) => r.data),
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: ['target_list.list'] });
      showToast('Target created.', 'success');
    },
    onError: (error) => showToast(normalizeError(error).message, 'error'),
  });
}

/**
 * Submit a `TodoList`/`TargetList` — the owner (Staff, or the Manager
 * who assigned it) moves it from Draft to Submitted. Both aggregates
 * share this same command shape (design doc §4.0.2's confirmed shared
 * verification structure), so one hook parameterized by `kind` serves
 * both rather than two near-identical copies.
 */
export function useSubmitList(kind: 'todo_list' | 'target_list') {
  const client = useQueryClient();
  const commandType = kind === 'todo_list' ? 'todo_list.SubmitTodoList' : 'target_list.SubmitTargetList';
  const payloadKey = kind === 'todo_list' ? 'SubmitTodoList' : 'SubmitTargetList';
  return useMutation({
    networkMode: 'always',
    mutationFn: (list: TodoListProjection | TargetListProjection) =>
      executeCommand({
        command_type: commandType,
        target: { id: list.id, type: kind, organization_id: organizationId() },
        expected_version: list.version,
        expected_lifecycle_epoch: list.lifecycle_epoch,
        expected_authority_epoch: list.authority_epoch,
        payload: payloadKey,
      }),
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: [`${kind}.list`] });
      await client.invalidateQueries({ queryKey: [`${kind}.detail`] });
      showToast('Submitted for verification.', 'success');
    },
    onError: (error) => showToast(normalizeError(error).message, 'error'),
  });
}

/**
 * Verify, reject, or escalate a `TodoList`/`TargetList` — the
 * decision-gated actions D.4's `verifier_resolution` module actually
 * authorizes (see `api_server::routes::command::require_verifier_authority`).
 * A caller who isn't an authorized verifier gets a real domain-error
 * response from the server; this hook surfaces it via the existing
 * toast/error path rather than trying to pre-validate authorization
 * client-side (the server is the source of truth here, per D.4 — the
 * UI does not duplicate that resolution logic).
 */
export function useDecideList(kind: 'todo_list' | 'target_list') {
  const client = useQueryClient();
  return useMutation({
    networkMode: 'always',
    mutationFn: ({
      list,
      decision,
      outcome,
      comment,
      reason,
    }: {
      list: TodoListProjection | TargetListProjection;
      decision: 'verify' | 'reject' | 'escalate';
      outcome?: VerificationOutcome;
      comment?: string;
      reason?: string;
    }) => {
      const commandType =
        decision === 'verify'
          ? kind === 'todo_list'
            ? 'todo_list.VerifyTodoList'
            : 'target_list.VerifyTargetList'
          : decision === 'reject'
            ? kind === 'todo_list'
              ? 'todo_list.RejectTodoList'
              : 'target_list.RejectTargetList'
            : kind === 'todo_list'
              ? 'todo_list.EscalateTodoList'
              : 'target_list.EscalateTargetList';
      const payloadKey =
        decision === 'verify'
          ? kind === 'todo_list'
            ? 'VerifyTodoList'
            : 'VerifyTargetList'
          : decision === 'reject'
            ? kind === 'todo_list'
              ? 'RejectTodoList'
              : 'RejectTargetList'
            : kind === 'todo_list'
              ? 'EscalateTodoList'
              : 'EscalateTargetList';
      const payload =
        decision === 'verify'
          ? { [payloadKey]: { outcome, comment: comment?.trim() ? comment : null } }
          : { [payloadKey]: { reason: reason ?? '' } };
      return executeCommand({
        command_type: commandType,
        target: { id: list.id, type: kind, organization_id: organizationId() },
        expected_version: list.version,
        expected_lifecycle_epoch: list.lifecycle_epoch,
        expected_authority_epoch: list.authority_epoch,
        payload,
      });
    },
    onSuccess: async (_, variables) => {
      await client.invalidateQueries({ queryKey: [`${kind}.list`] });
      await client.invalidateQueries({ queryKey: [`${kind}.detail`] });
      const verb =
        variables.decision === 'verify' ? 'Verified' : variables.decision === 'reject' ? 'Rejected' : 'Escalated';
      showToast(`${verb}.`, 'success');
    },
    onError: (error) => showToast(normalizeError(error).message, 'error'),
  });
}
