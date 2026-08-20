import { useRef, useState } from "react";
import type { Id16 } from "@/types/onyx";
import { useCommand } from "@/hooks/useCommand";
import AccessibleDialog from "@/components/Dialog/AccessibleDialog";
import { userFacingMessage } from "@/utils/userFacingError";

export interface ApprovalDialogProps {
  open: boolean;
  onClose: () => void;
  onDecided: () => void;
  taskId: Id16;
  taskTitle: string;
  taskVersion: number;
  organizationId: Id16;
  userId: Id16;
  deviceId: Id16;
}

const reasonPolicy = {
  approve: { required: false, label: "Decision note (optional)" },
  reject: { required: true, label: "Reason (required)" },
} as const;

export default function ApprovalDialog({
  open,
  onClose,
  onDecided,
  taskId,
  taskTitle,
  taskVersion,
  organizationId,
  userId,
  deviceId,
}: ApprovalDialogProps) {
  const [reason, setReason] = useState("");
  const reasonRef = useRef<HTMLTextAreaElement>(null);
  const { execute, loading, error } = useCommand();

  async function decide(commandType: "ApproveTask" | "RejectTask") {
    const policy = commandType === "ApproveTask" ? reasonPolicy.approve : reasonPolicy.reject;
    if (policy.required && reason.trim().length === 0) return;
    await execute({
      commandType,
      targetId: taskId,
      targetType: "task",
      organizationId,
      userId,
      deviceId,
      expectedVersion: taskVersion,
      payload: { [commandType]: { reason: reason.trim() } },
    });
    setReason("");
    onDecided();
  }

  return (
    <AccessibleDialog
      open={open}
      title="Review submission"
      description={taskTitle}
      initialFocusRef={reasonRef}
      busy={loading}
      onRequestClose={onClose}
    >
      <label className="mt-4 block text-xs font-medium text-onyx-text-dim" htmlFor="approval-reason">
        {reasonPolicy.approve.label}
      </label>
      <textarea
        ref={reasonRef}
        id="approval-reason"
        className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-2 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        rows={3}
        value={reason}
        onChange={(event) => setReason(event.target.value)}
      />
      {error && <p className="mt-2 text-sm text-onyx-status-blocked" role="alert">{userFacingMessage(error)}</p>}
      <div className="mt-4 flex justify-end gap-2">
        <button type="button" onClick={onClose} disabled={loading} className="rounded-md px-3 py-1.5 text-sm text-onyx-text-dim hover:bg-onyx-surface-hover">Cancel</button>
        <button type="button" onClick={() => void decide("RejectTask")} disabled={loading || reason.trim().length === 0} className="rounded-md border border-onyx-status-blocked px-3 py-1.5 text-sm text-onyx-status-blocked hover:bg-onyx-status-blocked/10 disabled:opacity-50">Reject</button>
        <button type="button" onClick={() => void decide("ApproveTask")} disabled={loading} className="rounded-md bg-onyx-status-approved px-3 py-1.5 text-sm font-medium text-onyx-bg hover:opacity-90 disabled:opacity-50">Approve</button>
      </div>
    </AccessibleDialog>
  );
}
