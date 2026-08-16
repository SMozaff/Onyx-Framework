import { useMutation, useQueryClient } from '@tanstack/react-query';
import { executeCommand } from '../api/command';
import type { ApprovalProjection, NotificationProjection } from '../types/query';
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
