import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import { NavLink } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import type { SyncStatus } from "@/types/onyx";

const NAV_ITEMS = [
  { to: "/", label: "Dashboard", end: true },
  { to: "/missions", label: "Missions", end: false },
  { to: "/tasks", label: "Tasks", end: false },
  { to: "/approvals", label: "Approvals", end: false },
  { to: "/messaging", label: "Messaging", end: false },
  { to: "/files", label: "Files", end: false },
];

const linkClasses = (isActive: boolean) =>
  `block rounded-md px-3 py-2 text-sm font-medium transition-colors ${
    isActive
      ? "bg-onyx-surface-hover text-onyx-text"
      : "text-onyx-text-dim hover:bg-onyx-surface-hover hover:text-onyx-text"
  }`;

/**
 * Header + sidebar shell wrapping every route. Polls
 * `get_sync_status` (the real Tauri command, wrapping
 * `client-composition`'s `SyncAgent::status`) on an interval and shows
 * it in the header, since sync state is relevant regardless of which
 * page the user is on — matching Team Prompt 5 §7's "Sync Status
 * Display" acceptance criterion.
 */
export default function MainLayout({ children }: { children: ReactNode }) {
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function poll() {
      try {
        const status = await invoke<SyncStatus>("get_sync_status");
        if (!cancelled) setSyncStatus(status);
      } catch {
        // get_sync_status has no documented failure mode in normal
        // operation (it only serializes an in-memory struct); a
        // transient IPC error here just means the header shows the
        // previous status until the next successful poll rather than
        // surfacing an error banner for a non-critical status readout.
      }
    }

    void poll();
    const interval = setInterval(() => void poll(), 5000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  return (
    <div className="flex h-screen">
      <aside className="w-56 shrink-0 border-r border-onyx-border bg-onyx-surface p-4">
        <div className="mb-6 px-2 text-lg font-semibold tracking-tight text-onyx-text">
          ONYX
        </div>
        <nav className="space-y-1">
          {NAV_ITEMS.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.end}
              className={({ isActive }) => linkClasses(isActive)}
            >
              {item.label}
            </NavLink>
          ))}
        </nav>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 shrink-0 items-center justify-end border-b border-onyx-border px-6">
          <SyncIndicator status={syncStatus} />
        </header>
        <main className="min-h-0 flex-1 overflow-auto p-6">{children}</main>
      </div>
    </div>
  );
}

function SyncIndicator({ status }: { status: SyncStatus | null }) {
  if (status === null) {
    return <span className="text-sm text-onyx-text-dim">Sync: —</span>;
  }

  const dotColor = status.online ? "bg-onyx-status-approved" : "bg-onyx-status-blocked";
  const label = status.online ? "Online" : "Offline";

  return (
    <div className="flex items-center gap-3 text-sm text-onyx-text-dim">
      {status.open_conflict_count > 0 && (
        <span className="text-onyx-status-blocked">
          {status.open_conflict_count} conflict{status.open_conflict_count === 1 ? "" : "s"}
        </span>
      )}
      {status.pending_outbox_count > 0 && <span>{status.pending_outbox_count} pending</span>}
      <span className="flex items-center gap-1.5">
        <span className={`h-2 w-2 rounded-full ${dotColor}`} />
        {label}
      </span>
    </div>
  );
}
