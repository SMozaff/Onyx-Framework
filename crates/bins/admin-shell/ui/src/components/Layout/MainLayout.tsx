import { NavLink, Outlet } from "react-router-dom";
import { useAuthStore } from "@/stores/authStore";

const NAV_ITEMS = [
  { to: "/users", label: "Users", end: false },
  { to: "/settings", label: "Policy & Settings", end: false },
  { to: "/profiles", label: "Staff Profiles", end: false },
];

const linkClasses = (isActive: boolean) =>
  `block rounded-md px-3 py-2 text-sm font-medium transition-colors ${
    isActive
      ? "bg-onyx-surface-hover text-onyx-text"
      : "text-onyx-text-dim hover:bg-onyx-surface-hover hover:text-onyx-text"
  }`;

/**
 * Header + sidebar shell for the Admin platform. Uses react-router's
 * `<Outlet />` (nested-route children), not a `{children}` prop like
 * `desktop-shell`'s `MainLayout` — this app's `App.tsx` nests routes
 * under this layout via `<Route element={<AdminOnlyLayout />}>` rather
 * than passing children explicitly, since `AdminOnlyLayout` itself
 * needs to conditionally render this layout vs. an access-denied
 * screen (see `App.tsx`'s own doc comment).
 *
 * No sync-status indicator (unlike `desktop-shell`'s `MainLayout`) —
 * this is a thin HTTP client with no local sync engine to report on;
 * shows the logged-in admin's identity and a logout action instead,
 * which is what actually matters for this app.
 */
export default function MainLayout() {
  const user = useAuthStore((state) => state.user);

  return (
    <div className="flex h-screen">
      <aside className="w-56 shrink-0 border-r border-onyx-border bg-onyx-surface p-4">
        <div className="mb-6 px-2 text-lg font-semibold tracking-tight text-onyx-text">
          ONYX Admin
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
        <header className="flex h-14 shrink-0 items-center justify-end gap-3 border-b border-onyx-border px-6">
          <span className="text-sm text-onyx-text-dim">{user?.username}</span>
          <button
            type="button"
            onClick={() => useAuthStore.getState().logout()}
            className="rounded-md bg-onyx-surface px-3 py-1.5 text-xs text-onyx-text hover:bg-onyx-surface-hover"
          >
            Log out
          </button>
        </header>
        <main className="min-h-0 flex-1 overflow-auto p-6">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
