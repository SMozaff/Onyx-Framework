import { useState } from "react";
import type { Id16 } from "@/types/onyx";
import { useCommand } from "@/hooks/useCommand";

export interface ApprovalDialogProps {
  open: boolean;
  onClose: () => void;
  onDecided: () => void;
  taskId: Id16;
  taskTitle: string;
  /** The task's currently-known version — required for the optimistic-
   * concurrency `expected_version` field on `ApproveTask`/`RejectTask`.
   * Callers get this from the `Tasks` page's own `useQuery` result. */
  taskVersion: number;
  organizationId: Id16;
  userId: Id16;
  deviceId: Id16;
}

/**
 * A modal for approving or rejecting a submitted task, via the real
 * `ApproveTask { reason }` / `RejectTask { reason }` commands
 * (`work_domain::TaskCommand`, both valid only from `Submitted` per
 * that aggregate's ruled state machine — see `DECISIONS.md` "Team 1
 * DECISIONS" ruling B, `RejectTask`).
 */
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
  const { execute, loading, error } = useCommand();

  if (!open) return null;

  async function decide(commandType: "ApproveTask" | "RejectTask") {
    if (reason.trim().length === 0) return;
    await execute({
      commandType,
      targetId: taskId,
      targetType: "task",
      organizationId,
      userId,
      deviceId,
      expectedVersion: taskVersion,
      payload: { [commandType]: { reason } },
    });
    setReason("");
    onDecided();
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="w-full max-w-md rounded-lg border border-onyx-border bg-onyx-surface p-5">
        <h2 className="text-base font-semibold text-onyx-text">Review submission</h2>
        <p className="mt-1 text-sm text-onyx-text-dim">{taskTitle}</p>

        <label className="mt-4 block text-xs font-medium text-onyx-text-dim">
          Reason
          <textarea
            className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-2 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
            rows={3}
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            placeholder="Required for both approval and rejection"
          />
        </label>

        {error && <p className="mt-2 text-sm text-onyx-status-blocked">{error.message}</p>}

        <div className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            disabled={loading}
            className="rounded-md px-3 py-1.5 text-sm text-onyx-text-dim hover:bg-onyx-surface-hover"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => void decide("RejectTask")}
            disabled={loading || reason.trim().length === 0}
            className="rounded-md border border-onyx-status-blocked px-3 py-1.5 text-sm text-onyx-status-blocked hover:bg-onyx-status-blocked/10 disabled:opacity-50"
          >
            Reject
          </button>
          <button
            type="button"
            onClick={() => void decide("ApproveTask")}
            disabled={loading || reason.trim().length === 0}
            className="rounded-md bg-onyx-status-approved px-3 py-1.5 text-sm font-medium text-onyx-bg hover:opacity-90 disabled:opacity-50"
          >
            Approve
          </button>
        </div>
      </div>
    </div>
  );
}
