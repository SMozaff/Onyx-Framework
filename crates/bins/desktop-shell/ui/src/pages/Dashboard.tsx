import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import type { SyncStatus } from "@/types/onyx";
import { useSession } from "@/hooks/useSession";

/**
 * The command-center landing page. There is currently no "list all
 * missions" / "list all tasks" query anywhere in the backend —
 * `client-composition`'s `QueryRegistry` only has `GetMission`/`GetTask`
 * (single-aggregate lookup by id, per `app_state.rs`), not a projection
 * or index query. This page is honest about that rather than faking
 * list data: it shows the real sync status (the one thing genuinely
 * queryable without a target id) and points to the Missions/Tasks pages
 * for anything that needs a specific aggregate.
 */
export default function Dashboard() {
  const { organizationId } = useSession();
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
      <h1 className="text-xl font-semibold text-onyx-text">Dashboard</h1>
      <p className="mt-1 text-sm text-onyx-text-dim">
        Command center overview for this replica.
      </p>

      <div className="mt-6 grid grid-cols-1 gap-4 sm:grid-cols-3">
        <StatCard
          label="Pending outbox"
          value={status ? String(status.pending_outbox_count) : "—"}
        />
        <StatCard
          label="Open conflicts"
          value={status ? String(status.open_conflict_count) : "—"}
          warn={status !== null && status.open_conflict_count > 0}
        />
        <StatCard
          label="Sync status"
          value={status === null ? "—" : status.online ? "Online" : "Offline"}
          warn={status !== null && !status.online}
        />
      </div>

      {loadError && (
        <p className="mt-4 text-sm text-onyx-status-blocked">
          Failed to load sync status: {loadError}
        </p>
      )}

      <div className="mt-8 flex gap-3">
        <Link
          to="/missions"
          className="rounded-md bg-onyx-surface px-4 py-2 text-sm text-onyx-text hover:bg-onyx-surface-hover"
        >
          Open Missions
        </Link>
        <Link
          to="/tasks"
          className="rounded-md bg-onyx-surface px-4 py-2 text-sm text-onyx-text hover:bg-onyx-surface-hover"
        >
          Open Tasks
        </Link>
        <Link
          to="/approvals"
          className="rounded-md bg-onyx-surface px-4 py-2 text-sm text-onyx-text hover:bg-onyx-surface-hover"
        >
          Review Approvals
        </Link>
      </div>

      <p className="mt-8 text-xs text-onyx-text-dim">
        Session organization: {organizationId.slice(0, 4).join(",")}… (placeholder — no
        authentication flow exists yet; see the codebase's own
        <code className="mx-1 rounded bg-onyx-surface px-1 py-0.5">TODO(auth/org resolution)</code>
        notes).
      </p>
    </div>
  );
}

function StatCard({ label, value, warn }: { label: string; value: string; warn?: boolean }) {
  return (
    <div className="rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <div className="text-xs text-onyx-text-dim">{label}</div>
      <div
        className={`mt-1 text-2xl font-semibold ${
          warn ? "text-onyx-status-blocked" : "text-onyx-text"
        }`}
      >
        {value}
      </div>
    </div>
  );
}
