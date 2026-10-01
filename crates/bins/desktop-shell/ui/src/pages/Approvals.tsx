import { useState } from "react";
import type { Id16, LoadedAggregate } from "@/types/onyx";
import { useQuery } from "@/hooks/useQuery";
import { useSession } from "@/hooks/useSession";
import { useI18n } from "@/i18n/I18nContext";
import StatusBadge from "@/components/StatusBadge";
import ApprovalDialog from "@/components/ApprovalDialog";

export default function Approvals() {
  const session = useSession();
  const { t } = useI18n();
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
      <h1 className="text-xl font-semibold text-onyx-text">{t("approvals.title")}</h1>
      <p className="mt-1 text-sm text-onyx-text-dim">
        {t("approvals.lookupTaskToReview")}{" "}
        <code className="rounded bg-onyx-surface px-1 py-0.5">{t("status.submitted")}</code>{" "}
        {t("approvals.submittedOnlyNote")}
      </p>

      <div className="mt-4 flex gap-2">
        <input
          value={taskIdRaw}
          onChange={(e) => setTaskIdRaw(e.target.value)}
          placeholder={t("common.id16Placeholder")}
          className="flex-1 rounded-md border border-onyx-border bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          onClick={lookup}
          className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover"
        >
          {t("common.lookup")}
        </button>
      </div>

      {targetId && (
        <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
          {loading && <p className="text-sm text-onyx-text-dim">{t("common.loading")}</p>}
          {error && <p className="text-sm text-onyx-status-blocked">{error.message}</p>}
          {!loading && !error && data === null && (
            <p className="text-sm text-onyx-text-dim">{t("tasks.noTaskFoundForId")}</p>
          )}
          {data && (
            <div>
              <div className="flex items-center justify-between">
                <h2 className="text-base font-medium text-onyx-text">
                  {String(data.aggregate.title ?? t("common.untitled"))}
                </h2>
                <StatusBadge status={String(data.aggregate.status ?? "Unknown")} />
              </div>

              {isSubmitted ? (
                <button
                  type="button"
                  onClick={() => setDialogOpen(true)}
                  className="mt-4 rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white"
                >
                  {t("common.review")}
                </button>
              ) : (
                <p className="mt-4 text-sm text-onyx-text-dim">
                  {t("approvals.notAwaitingApproval")}
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
          taskTitle={String(data.aggregate.title ?? t("common.untitled"))}
          taskVersion={data.version}
          organizationId={session.organizationId}
          userId={session.userId}
          deviceId={session.deviceId}
        />
      )}
    </div>
  );
}
