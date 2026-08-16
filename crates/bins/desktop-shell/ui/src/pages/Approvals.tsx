import { useState } from "react";
import type { Id16, LoadedAggregate } from "@/types/onyx";
import { useQuery } from "@/hooks/useQuery";
import { useSession } from "@/hooks/useSession";
import StatusBadge from "@/components/StatusBadge";
import ApprovalDialog from "@/components/ApprovalDialog";

/**
 * Approval review page. Same "no list query" caveat as `Missions.tsx`/
 * `Tasks.tsx` — a real "queue of everything Submitted" view needs a
 * projection query the backend doesn't have yet, so this page works
 * against one task at a time by id, same as the other two pages, and
 * surfaces `ApprovalDialog` when that task is in the `Submitted` state.
 */
export default function Approvals() {
  const session = useSession();
  const [taskIdRaw, setTaskIdRaw] = useState("");
  const [targetId, setTargetId] = useState<Id16 | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);

  const { data, loading, error, refetch } = useQuery<LoadedAggregate>("GetTask", targetId);

  function lookup() {
    try {
      const parsed = JSON.parse(taskIdRaw);
      if (!Array.isArray(parsed) || parsed.length !== 16) {
        throw new Error("expected a JSON array of 16 numbers");
      }
      setTargetId(parsed as Id16);
    } catch {
      setTargetId(null);
    }
  }

  const isSubmitted = data?.aggregate.status === "Submitted";

  return (
    <div className="max-w-3xl">
      <h1 className="text-xl font-semibold text-onyx-text">Approvals</h1>
      <p className="mt-1 text-sm text-onyx-text-dim">
        Look up a task by id to review it. Only tasks in the{" "}
        <code className="rounded bg-onyx-surface px-1 py-0.5">Submitted</code> state can be
        approved or rejected.
      </p>

      <div className="mt-4 flex gap-2">
        <input
          value={taskIdRaw}
          onChange={(e) => setTaskIdRaw(e.target.value)}
          placeholder="[12,34,...]"
          className="flex-1 rounded-md border border-onyx-border bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          onClick={lookup}
          className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover"
        >
          Look up
        </button>
      </div>

      {targetId && (
        <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
          {loading && <p className="text-sm text-onyx-text-dim">Loading…</p>}
          {error && <p className="text-sm text-onyx-status-blocked">{error.message}</p>}
          {!loading && !error && data === null && (
            <p className="text-sm text-onyx-text-dim">No task found for this id.</p>
          )}
          {data && (
            <div>
              <div className="flex items-center justify-between">
                <h2 className="text-base font-medium text-onyx-text">
                  {String(data.aggregate.title ?? "(untitled)")}
                </h2>
                <StatusBadge status={String(data.aggregate.status ?? "Unknown")} />
              </div>

              {isSubmitted ? (
                <button
                  type="button"
                  onClick={() => setDialogOpen(true)}
                  className="mt-4 rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white"
                >
                  Review
                </button>
              ) : (
                <p className="mt-4 text-sm text-onyx-text-dim">
                  This task is not awaiting approval.
                </p>
              )}
            </div>
          )}
        </div>
      )}

      {data && targetId && (
        <ApprovalDialog
          open={dialogOpen}
          onClose={() => setDialogOpen(false)}
          onDecided={() => {
            setDialogOpen(false);
            void refetch();
          }}
          taskId={targetId}
          taskTitle={String(data.aggregate.title ?? "(untitled)")}
          taskVersion={data.version}
          organizationId={session.organizationId}
          userId={session.userId}
          deviceId={session.deviceId}
        />
      )}
    </div>
  );
}
