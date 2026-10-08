import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import type { SyncStatus } from "@/types/onyx";
import { useSession } from "@/hooks/useSession";
import { useI18n } from "@/i18n/I18nContext";

export default function Dashboard() {
  const { organizationIdText, serverAddress } = useSession();
  const { t } = useI18n();
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<SyncStatus>("get_sync_status")
      .then((s) => {
        if (!cancelled) setStatus(s);
      })
      .catch((e) => {
        if (!cancelled) setLoadError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="max-w-3xl">
      <h1 className="text-xl font-semibold text-onyx-text">{t("dashboard.title")}</h1>
      <p className="mt-1 text-sm text-onyx-text-dim">
        {t("dashboard.commandCenterOverview")}
      </p>

      <div className="mt-6 grid grid-cols-1 gap-4 sm:grid-cols-3">
        <StatCard
          label={t("dashboard.pendingOutbox")}
          value={status ? String(status.pending_outbox_count) : "—"}
        />
        <StatCard
          label={t("dashboard.openConflicts")}
          value={status ? String(status.open_conflict_count) : "—"}
          warn={status !== null && status.open_conflict_count > 0}
        />
        <StatCard
          label={t("dashboard.syncStatus")}
          value={status === null ? "—" : status.online ? t("common.online") : t("common.offline")}
          warn={status !== null && !status.online}
        />
      </div>

      {loadError && (
        <p className="mt-4 text-sm text-onyx-status-blocked">
          {t("dashboard.failedToLoadSyncStatus")}: {loadError}
        </p>
      )}

      <div className="mt-8 flex gap-3">
        <Link
          to="/missions"
          className="rounded-md bg-onyx-surface px-4 py-2 text-sm text-onyx-text hover:bg-onyx-surface-hover"
        >
          {t("dashboard.openMissions")}
        </Link>
        <Link
          to="/tasks"
          className="rounded-md bg-onyx-surface px-4 py-2 text-sm text-onyx-text hover:bg-onyx-surface-hover"
        >
          {t("dashboard.openTasks")}
        </Link>
        <Link
          to="/approvals"
          className="rounded-md bg-onyx-surface px-4 py-2 text-sm text-onyx-text hover:bg-onyx-surface-hover"
        >
          {t("dashboard.reviewApprovals")}
        </Link>
      </div>

      <p className="mt-8 text-xs text-onyx-text-dim">
        {t("dashboard.connectedTo")} {serverAddress} {t("dashboard.forOrganization")} {organizationIdText}.
      </p>
    </div>
  );
}

function StatCard({ label, value, warn }: { label: string; value: string; warn?: boolean }) {
  return (
    <div className="rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <div className="text-xs text-onyx-text-dim">{label}</div>
      <div
        className={`mt-1 text-2xl font-semibold ${warn ? "text-onyx-status-blocked" : "text-onyx-text"}`}
      >
        {value}
      </div>
    </div>
  );
}
