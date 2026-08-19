import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import { NavLink, useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import type { SyncStatus } from "@/types/onyx";
import { useSession } from "@/hooks/useSession";

const NAV_ITEMS = [
  { to: "/", label: "Dashboard", end: true },
  { to: "/missions", label: "Missions", end: false },
  { to: "/tasks", label: "Tasks", end: false },
  { to: "/approvals", label: "Approvals", end: false },
  { to: "/messaging", label: "Messaging", end: false },
  { to: "/files", label: "Files", end: false },
  { to: "/notifications", label: "Notifications", end: false },
  { to: "/settings", label: "Settings", end: false },
];

const linkClasses = (isActive: boolean) =>
  `block rounded-md px-3 py-2 text-sm font-medium transition-colors ${
    isActive
      ? "bg-onyx-surface-hover text-onyx-text"
      : "text-onyx-text-dim hover:bg-onyx-surface-hover hover:text-onyx-text"
  }`;

/**
 * Header + sidebar shell wrapping every authenticated route. It polls the
 * existing local sync-status command and displays the real native session with
 * a logout action; App owns the actual session clearing so a successful logout
 * immediately returns to the pre-login route gate.
 */
export default function MainLayout({
  children,
  onLogout,
}: {
  children: ReactNode;
  onLogout: () => Promise<void>;
}) {
  const session = useSession();
  const navigate = useNavigate();
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  const [loggingOut, setLoggingOut] = useState(false);
  const [logoutError, setLogoutError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function poll() {
      try {
        const status = await invoke<SyncStatus>("get_sync_status");
        if (!cancelled) setSyncStatus(status);
      } catch {
        // A transient IPC error only leaves the previous sync indicator in
        // place; it is non-critical compared with the page's own actions.
      }
    }

    void poll();
    const interval = setInterval(() => void poll(), 5000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  async function logout() {
    setLogoutError(null);
    setLoggingOut(true);
    try {
      await onLogout();
      navigate("/", { replace: true });
    } catch (error) {
      setLogoutError(String(error));
      setLoggingOut(false);
    }
  }

  return (
    <div className="flex h-screen">
      <aside className="flex w-56 shrink-0 flex-col border-r border-onyx-border bg-onyx-surface p-4">
        <div className="mb-6 px-2 text-lg font-semibold tracking-tight text-onyx-text">ONYX</div>
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
        <div className="mt-auto border-t border-onyx-border pt-4">
          <p className="truncate px-2 text-xs font-medium text-onyx-text" title={session.username}>
            {session.username}
          </p>
          <p className="mt-1 truncate px-2 text-[11px] text-onyx-text-dim" title={session.serverAddress}>
            {session.serverAddress}
          </p>
          <button
            type="button"
            onClick={() => void logout()}
            disabled={loggingOut}
            className="mt-3 w-full rounded-md px-3 py-1.5 text-left text-sm font-medium text-onyx-text-dim hover:bg-onyx-surface-hover hover:text-onyx-text disabled:opacity-50"
          >
            {loggingOut ? "Signing out…" : "Sign out"}
          </button>
          {logoutError && <p className="mt-2 px-2 text-[11px] text-onyx-status-blocked">{logoutError}</p>}
        </div>
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
