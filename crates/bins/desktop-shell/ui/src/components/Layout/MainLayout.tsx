import type { ReactNode } from "react";
import { useEffect, useRef, useState } from "react";
import { NavLink, useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import type { SyncStatus } from "@/types/onyx";
import { useSession } from "@/hooks/useSession";
import { userFacingMessage } from "@/utils/userFacingError";
import onyxLogoHorizontal from "@/assets/onyx-logo-horizontal.png";

const NAV_ITEMS = [
  { to: "/", label: "Overview", end: true },
  { to: "/missions", label: "Missions", end: false },
  { to: "/tasks", label: "Tasks", end: false },
  { to: "/approvals", label: "Approvals", end: false },
  { to: "/messaging", label: "Messaging", end: false },
  { to: "/files", label: "Files", end: false },
  { to: "/notifications", label: "Notifications", end: false },
  { to: "/settings", label: "Settings", end: false },
];

const NARROW_VIEWPORT = "(max-width: 960px)";

export default function MainLayout({ children, onLogout }: { children: ReactNode; onLogout: () => Promise<void> }) {
  const session = useSession();
  const navigate = useNavigate();
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  const [loggingOut, setLoggingOut] = useState(false);
  const [logoutError, setLogoutError] = useState<string | null>(null);
  const [narrow, setNarrow] = useState(() => window.matchMedia(NARROW_VIEWPORT).matches);
  const [navigationOpen, setNavigationOpen] = useState(false);
  const navRef = useRef<HTMLElement>(null);
  const menuButtonRef = useRef<HTMLButtonElement>(null);
  const firstLinkRef = useRef<HTMLAnchorElement>(null);
  const mainRef = useRef<HTMLElement>(null);
  const previouslyOpen = useRef(false);

  useEffect(() => {
    const media = window.matchMedia(NARROW_VIEWPORT);
    const onChange = () => setNarrow(media.matches);
    onChange();
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, []);

  useEffect(() => {
    if (!narrow) setNavigationOpen(false);
    navRef.current?.toggleAttribute("inert", narrow && !navigationOpen);
  }, [narrow, navigationOpen]);

  useEffect(() => {
    if (!narrow) {
      previouslyOpen.current = false;
      return;
    }
    if (navigationOpen && !previouslyOpen.current) firstLinkRef.current?.focus();
    if (!navigationOpen && previouslyOpen.current) menuButtonRef.current?.focus();
    previouslyOpen.current = navigationOpen;
  }, [narrow, navigationOpen]);

  useEffect(() => {
    if (!narrow || !navigationOpen) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setNavigationOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [narrow, navigationOpen]);

  useEffect(() => {
    let cancelled = false;
    async function poll() {
      try {
        const status = await invoke<SyncStatus>("get_sync_status");
        if (!cancelled) setSyncStatus(status);
      } catch {
        // Retain the previous connectivity state while a transient IPC call is retried.
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
      setLogoutError(userFacingMessage(error));
      setLoggingOut(false);
    }
  }

  function handleNavigate() {
    setNavigationOpen(false);
    window.setTimeout(() => mainRef.current?.focus(), 0);
  }

  const closedDrawer = narrow && !navigationOpen;
  const organizationLabel = `Organization ${session.organizationIdText.slice(0, 8)}…`;
  const offline = syncStatus !== null && !syncStatus.online;

  return (
    <div className="flex h-screen overflow-hidden bg-onyx-bg">
      {narrow && navigationOpen && (
        <button
          type="button"
          className="fixed inset-0 z-30 bg-slate-950/55"
          aria-label="Close navigation"
          onClick={() => setNavigationOpen(false)}
        />
      )}
      <aside
        ref={navRef}
        id="staff-primary-navigation"
        aria-label="Primary navigation"
        aria-hidden={closedDrawer || undefined}
        className={`onyx-sidebar z-40 flex w-60 shrink-0 flex-col p-3 ${narrow ? "fixed inset-y-0 left-0 transition-transform duration-200" : "static"} ${closedDrawer ? "-translate-x-full" : "translate-x-0"}`}
      >
        <div className="mb-8 flex items-center gap-2.5 px-2 pt-1">
          <span className="onyx-brand-mark" aria-hidden="true">O</span>
          <div>
            <p className="text-[0.72rem] font-extrabold tracking-[0.24em] text-white">ONYX</p>
            <p className="mt-0.5 text-[0.62rem] text-sky-100/70">Staff operations</p>
          </div>
        </div>

        <nav className="space-y-1" aria-label="Staff workspace">
          {NAV_ITEMS.map((item, index) => (
            <NavLink
              key={item.to}
              ref={index === 0 ? firstLinkRef : undefined}
              to={item.to}
              end={item.end}
              tabIndex={closedDrawer ? -1 : undefined}
              onClick={handleNavigate}
              className="onyx-nav-link"
            >
              {item.label}
            </NavLink>
          ))}
        </nav>

        <div className="mt-auto space-y-3 px-1 pb-1">
          <div className="onyx-sidebar-note">
            <p className="text-xs font-semibold text-white">Staff desktop</p>
            <p className="mt-1 text-[0.68rem] leading-4">Local replica with explicit sync state and protected commands.</p>
          </div>
          <div className="border-t border-white/15 px-2 pt-3">
            <p className="truncate text-xs font-semibold text-white" title={session.username}>{session.username}</p>
            <p className="mt-1 truncate text-[0.64rem] text-sky-100/65" title={session.serverAddress}>{session.serverAddress}</p>
            <button
              type="button"
              onClick={() => void logout()}
              disabled={loggingOut}
              className="mt-3 text-left text-xs font-semibold text-sky-100/80 underline decoration-sky-100/30 underline-offset-4 hover:text-white disabled:opacity-50"
            >
              {loggingOut ? "Signing out…" : "Sign out"}
            </button>
            {logoutError && <p className="mt-2 text-[0.68rem] leading-4 text-rose-200" role="alert">{logoutError}</p>}
          </div>
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        {offline && (
          <div className="onyx-connection-banner" role="status">
            <strong>Connection lost.</strong>
            <span>Commands are not queued. Reconnect, then retry the action.</span>
          </div>
        )}
        <header className="onyx-workspace-header flex min-h-16 shrink-0 items-center justify-between gap-4 border-b border-onyx-border px-4 sm:px-6">
          <div className="flex min-w-0 items-center gap-4">
            {narrow && (
              <button
                ref={menuButtonRef}
                type="button"
                aria-label={navigationOpen ? "Close navigation" : "Open navigation"}
                aria-controls="staff-primary-navigation"
                aria-expanded={navigationOpen}
                onClick={() => setNavigationOpen((open) => !open)}
                className="rounded-md border border-onyx-border bg-white px-3 py-1.5 text-xs font-semibold text-onyx-text hover:bg-onyx-surface-hover"
              >
                Menu
              </button>
            )}
            <img src={onyxLogoHorizontal} alt="ONYX" className="hidden h-6 w-auto shrink-0 sm:block" />
            <div className="min-w-0 sm:border-l sm:border-onyx-border sm:pl-4">
              <p className="text-[0.63rem] font-bold uppercase tracking-[0.14em] text-onyx-text-dim">Organization</p>
              <p className="truncate text-sm font-semibold text-onyx-text" title={session.organizationIdText}>{organizationLabel}</p>
              <p className="mt-0.5 text-[0.65rem] text-onyx-text-dim">Native operational workspace</p>
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-3">
            <SyncIndicator status={syncStatus} />
            <div className="hidden border-l border-onyx-border pl-3 text-right sm:block">
              <p className="text-xs font-semibold text-onyx-text">{session.username}</p>
              <p className="text-[0.64rem] text-onyx-text-dim">Staff operator</p>
            </div>
          </div>
        </header>
        <main ref={mainRef} tabIndex={-1} className="min-h-0 flex-1 overflow-auto p-4 sm:p-6 lg:p-8">{children}</main>
      </div>
    </div>
  );
}

function SyncIndicator({ status }: { status: SyncStatus | null }) {
  if (status === null) return <span className="onyx-state-chip bg-slate-100 text-onyx-text-dim">Checking connection</span>;
  const online = status.online;
  return (
    <div className="flex items-center gap-2">
      {status.open_conflict_count > 0 && <span className="hidden text-xs font-semibold text-onyx-status-blocked lg:inline">{status.open_conflict_count} conflict{status.open_conflict_count === 1 ? "" : "s"}</span>}
      {status.pending_outbox_count > 0 && <span className="hidden text-xs text-onyx-text-dim lg:inline">{status.pending_outbox_count} pending</span>}
      <span className={`onyx-state-chip ${online ? "onyx-state-chip--online" : "onyx-state-chip--offline"}`}>
        <span className={`h-1.5 w-1.5 rounded-full ${online ? "bg-onyx-status-approved" : "bg-onyx-status-blocked"}`} />
        {online ? "Connected" : "Disconnected"}
      </span>
    </div>
  );
}
