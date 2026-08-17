import { useMutation, useQueryClient } from '@tanstack/react-query';
import { apiClient } from '../api/client';
import { executeCommand } from '../api/command';
import type {
  ApprovalProjection,
  ListOrigin,
  NotificationProjection,
  StaffLoanProjection,
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
 * Adds an item to a `Draft` `TodoList`. `TargetList` has no equivalent
 * — a target is described by `description`, not discrete items (design
 * doc §4.0.2), so this hook is `TodoList`-only, unlike `useSubmitList`/
 * `useDecideList` above which serve both kinds. Server rejects this
 * once the list is no longer `Draft` (`todo_domain`'s own
 * `AddItem` doc comment: "Rejected once the list has been submitted").
 */
export function useAddTodoItem() {
  const client = useQueryClient();
  return useMutation({
    networkMode: 'always',
    mutationFn: ({ list, description }: { list: TodoListProjection; description: string }) =>
      executeCommand({
        command_type: 'todo_list.AddItem',
        target: { id: list.id, type: 'todo_list', organization_id: organizationId() },
        expected_version: list.version,
        expected_lifecycle_epoch: list.lifecycle_epoch,
        expected_authority_epoch: list.authority_epoch,
        payload: { AddItem: { item: { item_id: crypto.randomUUID(), description } } },
      }),
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: ['todo_list.list'] });
      showToast('Item added.', 'success');
    },
    onError: (error) => showToast(normalizeError(error).message, 'error'),
  });
}

/**
 * Records a Team Leader pre-check on a `Submitted` `TodoList`/
 * `TargetList`. Gated server-side to the `team_leader` class (or
 * Admin) — see `api_server::routes::command::require_team_leader_or_admin`,
 * added 2026-08-16 to close a real authorization gap this command
 * previously had none of. Not pre-validated client-side for the same
 * reason `useDecideList`/`useDecideStaffLoan` don't pre-validate their
 * own gates: the server is the source of truth, and an unauthorized
 * caller gets a real domain error surfaced via the existing toast path.
 */
export function useRecordPreCheck(kind: 'todo_list' | 'target_list') {
  const client = useQueryClient();
  const commandType =
    kind === 'todo_list' ? 'todo_list.RecordTeamLeaderPreCheck' : 'target_list.RecordTeamLeaderPreCheck';
  return useMutation({
    networkMode: 'always',
    mutationFn: ({ list, notes }: { list: TodoListProjection | TargetListProjection; notes: string }) =>
      executeCommand({
        command_type: commandType,
        target: { id: list.id, type: kind, organization_id: organizationId() },
        expected_version: list.version,
        expected_lifecycle_epoch: list.lifecycle_epoch,
        expected_authority_epoch: list.authority_epoch,
        payload: { RecordTeamLeaderPreCheck: { notes } },
      }),
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: [`${kind}.list`] });
      await client.invalidateQueries({ queryKey: [`${kind}.detail`] });
      showToast('Pre-check recorded.', 'success');
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

/**
 * Request a new `StaffLoan` — routed through the dedicated REST route
 * (`POST /api/todo/staff-loans`), not `/api/command`: `RequestStaffLoan`
 * is `create()`-routed, same reason `useCreateTodoList`/
 * `useCreateTargetList` above bypass `executeCommand`. Starts in
 * `Requested`, awaiting the real owner's approval via
 * `useDecideStaffLoan` below — design doc §2.1.
 */
export function useRequestStaffLoan() {
  const client = useQueryClient();
  return useMutation({
    networkMode: 'always',
    mutationFn: (input: {
      staff_user_id: string;
      real_owner_id: string;
      borrowing_manager_id: string;
      start_at_ms: number;
      end_at_ms: number;
    }) => apiClient.post<{ staff_loan_id: string }>('/api/todo/staff-loans', input).then((r) => r.data),
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: ['staff_loan.list'] });
      showToast('Staff loan requested.', 'success');
    },
    onError: (error) => showToast(normalizeError(error).message, 'error'),
  });
}

/**
 * Approve/decline/extend/end a `StaffLoan`. Deliberately does not
 * cover `ExpireStaffLoan` — per `todo_domain::command::StaffLoanCommand`'s
 * own doc comment, that command is invoked only by the scheduled
 * background job (design doc §2.1's advance-warning/expiry job), never
 * by a user action, so no UI surface exists for it here.
 *
 * Design doc §2.1's three distinct approval gates, resolved
 * 2026-08-16, are not enforced client-side — the same rule this file's
 * other decision hooks follow: the server is the source of truth
 * (`ApproveStaffLoan`/`DeclineStaffLoan` are real-owner-only,
 * `ExtendStaffLoan` requires the staff member's own approval,
 * `EndStaffLoanEarly` requires no approval from anyone), and an
 * unauthorized caller gets a real domain error back, surfaced via the
 * existing toast path.
 */
export function useDecideStaffLoan() {
  const client = useQueryClient();
  return useMutation({
    networkMode: 'always',
    mutationFn: ({
      loan,
      decision,
      reason,
      newEndAtMs,
    }: {
      loan: StaffLoanProjection;
      decision: 'approve' | 'decline' | 'extend' | 'end';
      reason?: string;
      newEndAtMs?: number;
    }) => {
      const commandType =
        decision === 'approve'
          ? 'staff_loan.ApproveStaffLoan'
          : decision === 'decline'
            ? 'staff_loan.DeclineStaffLoan'
            : decision === 'extend'
              ? 'staff_loan.ExtendStaffLoan'
              : 'staff_loan.EndStaffLoanEarly';
      const payload =
        decision === 'approve' || decision === 'end'
          ? commandType.split('.')[1]
          : decision === 'decline'
            ? { DeclineStaffLoan: { reason: reason ?? '' } }
            : { ExtendStaffLoan: { new_end_at: Math.round((newEndAtMs ?? 0) * 1_000_000) } };
      return executeCommand({
        command_type: commandType,
        target: { id: loan.id, type: 'staff_loan', organization_id: organizationId() },
        expected_version: loan.version,
        expected_lifecycle_epoch: loan.lifecycle_epoch,
        expected_authority_epoch: loan.authority_epoch,
        payload,
      });
    },
    onSuccess: async (_, variables) => {
      await client.invalidateQueries({ queryKey: ['staff_loan.list'] });
      const verbs: Record<typeof variables.decision, string> = {
        approve: 'Approved', decline: 'Declined', extend: 'Extended', end: 'Ended',
      };
      showToast(`${verbs[variables.decision]}.`, 'success');
    },
    onError: (error) => showToast(normalizeError(error).message, 'error'),
  });
}
