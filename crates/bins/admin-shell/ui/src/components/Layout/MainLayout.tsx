import { NavLink, Outlet } from "react-router-dom";
import { useAuthStore } from "@/stores/authStore";

const NAV_ITEMS = [
  { to: "/users", label: "Users", end: false },
  { to: "/settings", label: "Policy & Settings", end: false },
  { to: "/profiles", label: "Staff Profiles", end: false },
];

/**
 * Header + sidebar shell for the Admin platform. The visual structure matches
 * the Staff desktop's operational workspace while retaining this app's thin
 * HTTP-client boundary: it intentionally presents stored identity/context,
 * not an invented sync or connection status.
 */
export default function MainLayout() {
  const user = useAuthStore((state) => state.user);
  const organizationLabel = user ? `Organization ${user.organization_id.slice(0, 8)}…` : "Organization context";

  return (
    <div className="flex h-screen overflow-hidden bg-onyx-bg">
      <aside className="onyx-sidebar flex w-60 shrink-0 flex-col p-3" aria-label="Admin primary navigation">
        <div className="mb-8 flex items-center gap-2.5 px-2 pt-1">
          <span className="onyx-brand-mark" aria-hidden="true">O</span>
          <div>
            <p className="text-[0.72rem] font-extrabold tracking-[0.24em] text-white">ONYX</p>
            <p className="mt-0.5 text-[0.62rem] text-sky-100/70">Administration</p>
          </div>
        </div>

        <nav className="space-y-1" aria-label="Administrative workspace">
          {NAV_ITEMS.map((item) => (
            <NavLink key={item.to} to={item.to} end={item.end} className="onyx-nav-link">
              {item.label}
            </NavLink>
          ))}
        </nav>

        <div className="mt-auto space-y-3 px-1 pb-1">
          <div className="onyx-sidebar-note">
            <p className="text-xs font-semibold text-white">Admin platform</p>
            <p className="mt-1 text-[0.68rem] leading-4">Policy, identity, and staff-profile controls are organization-scoped.</p>
          </div>
          <p className="px-2 text-[0.64rem] text-sky-100/65">Connection settings are verified before they are saved.</p>
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="onyx-workspace-header flex min-h-16 shrink-0 items-center justify-between gap-4 border-b border-onyx-border px-5 sm:px-7">
          <div className="min-w-0">
            <p className="text-[0.63rem] font-bold uppercase tracking-[0.14em] text-onyx-text-dim">Organization</p>
            <p className="truncate text-sm font-semibold text-onyx-text" title={user?.organization_id}>{organizationLabel}</p>
            <p className="mt-0.5 text-[0.65rem] text-onyx-text-dim">Administrative control plane</p>
          </div>
          <div className="flex shrink-0 items-center gap-3">
            <span className="onyx-state-chip bg-sky-50 text-onyx-accent">Admin session</span>
            <div className="hidden border-l border-onyx-border pl-3 text-right sm:block">
              <p className="text-xs font-semibold text-onyx-text">{user?.username}</p>
              <p className="text-[0.64rem] text-onyx-text-dim">Administrator</p>
            </div>
            <button
              type="button"
              onClick={() => useAuthStore.getState().logout()}
              className="rounded-md border border-onyx-border bg-white px-3 py-1.5 text-xs font-bold text-onyx-text transition-colors hover:bg-onyx-surface-hover"
            >
              Sign out
            </button>
          </div>
        </header>
        <main className="min-h-0 flex-1 overflow-auto p-5 sm:p-7 lg:p-8">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
