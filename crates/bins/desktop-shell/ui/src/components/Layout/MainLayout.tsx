import type { ReactNode } from "react";
import { useEffect, useRef, useState } from "react";
import { NavLink, useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import type { SyncStatus } from "@/types/onyx";
import { useSession } from "@/hooks/useSession";
import { userFacingMessage } from "@/utils/userFacingError";

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

const NARROW_VIEWPORT = "(max-width: 960px)";
const linkClasses = (isActive: boolean) => `block rounded-md px-3 py-2 text-sm font-medium transition-colors ${isActive ? "bg-onyx-surface-hover text-onyx-text" : "text-onyx-text-dim hover:bg-onyx-surface-hover hover:text-onyx-text"}`;

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
  return (
    <div className="flex h-screen overflow-hidden">
      {narrow && navigationOpen && <button type="button" className="fixed inset-0 z-30 bg-black/60" aria-label="Close navigation" onClick={() => setNavigationOpen(false)} />}
      <aside ref={navRef} id="staff-primary-navigation" aria-label="Primary navigation" aria-hidden={closedDrawer || undefined} className={`z-40 flex w-56 shrink-0 flex-col border-r border-onyx-border bg-onyx-surface p-4 ${narrow ? "fixed inset-y-0 left-0 transition-transform duration-200" : "static"} ${closedDrawer ? "-translate-x-full" : "translate-x-0"}`}>
        <div className="mb-6 px-2 text-lg font-semibold tracking-tight text-onyx-text">ONYX</div>
        <nav className="space-y-1">
          {NAV_ITEMS.map((item, index) => <NavLink key={item.to} ref={index === 0 ? firstLinkRef : undefined} to={item.to} end={item.end} tabIndex={closedDrawer ? -1 : undefined} onClick={handleNavigate} className={({ isActive }) => linkClasses(isActive)}>{item.label}</NavLink>)}
        </nav>
        <div className="mt-auto border-t border-onyx-border pt-4">
          <p className="truncate px-2 text-xs font-medium text-onyx-text" title={session.username}>{session.username}</p>
          <p className="mt-1 truncate px-2 text-[11px] text-onyx-text-dim" title={session.serverAddress}>{session.serverAddress}</p>
          <button type="button" onClick={() => void logout()} disabled={loggingOut} className="mt-3 w-full rounded-md px-3 py-1.5 text-left text-sm font-medium text-onyx-text-dim hover:bg-onyx-surface-hover hover:text-onyx-text disabled:opacity-50">{loggingOut ? "Signing out…" : "Sign out"}</button>
          {logoutError && <p className="mt-2 px-2 text-[11px] text-onyx-status-blocked" role="alert">{logoutError}</p>}
        </div>
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 shrink-0 items-center justify-between border-b border-onyx-border px-4 sm:px-6">
          {narrow ? <button ref={menuButtonRef} type="button" aria-label={navigationOpen ? "Close navigation" : "Open navigation"} aria-controls="staff-primary-navigation" aria-expanded={navigationOpen} onClick={() => setNavigationOpen((open) => !open)} className="rounded-md px-3 py-1.5 text-sm font-medium text-onyx-text hover:bg-onyx-surface-hover">Menu</button> : <span />}
          <SyncIndicator status={syncStatus} />
        </header>
        <main ref={mainRef} tabIndex={-1} className="min-h-0 flex-1 overflow-auto p-4 sm:p-6">{children}</main>
      </div>
    </div>
  );
}

function SyncIndicator({ status }: { status: SyncStatus | null }) {
  if (status === null) return <span className="text-sm text-onyx-text-dim">Sync: —</span>;
  const dotColor = status.online ? "bg-onyx-status-approved" : "bg-onyx-status-blocked";
  const label = status.online ? "Online" : "Offline";
  return <div className="flex items-center gap-3 text-sm text-onyx-text-dim">{status.open_conflict_count > 0 && <span className="text-onyx-status-blocked">{status.open_conflict_count} conflict{status.open_conflict_count === 1 ? "" : "s"}</span>}{status.pending_outbox_count > 0 && <span>{status.pending_outbox_count} pending</span>}<span className="flex items-center gap-1.5"><span className={`h-2 w-2 rounded-full ${dotColor}`} />{label}</span></div>;
}
