import { NavLink, Outlet } from 'react-router-dom';
import { useAuthStore } from '../stores/authStore';
import { useAuth } from '../hooks/useAuth';
import { OfflineBanner } from './OfflineBanner';

const NAV_ITEMS = [
  { to: '/dashboard', label: 'Dashboard' },
  { to: '/missions', label: 'Missions' },
  { to: '/tasks', label: 'Tasks' },
  { to: '/notifications', label: 'Notifications' },
  { to: '/approvals', label: 'Approvals' },
  { to: '/reports', label: 'Reports' },
];

export function ObserverLayout() {
  const user = useAuthStore((state) => state.user);
  const { logout, isPending } = useAuth();

  return (
    <div className="min-h-screen bg-slate-100 text-slate-900">
      <header className="sticky top-0 z-10 border-b border-slate-800 bg-[#0a1e3d] text-white">
        <div className="mx-auto flex max-w-3xl items-center justify-between px-4 py-3">
          <div className="flex items-baseline gap-3">
            <span className="font-bold tracking-wide">ONYX Observer</span>
            <span className="text-xs text-slate-300">read-only</span>
          </div>
          <div className="flex items-center gap-3">
            <span className="text-sm text-slate-300">{user?.username}</span>
            <button
              type="button"
              className="rounded bg-slate-700 px-2 py-1 text-sm hover:bg-slate-600"
              onClick={() => logout()}
              disabled={isPending}
            >
              Sign out
            </button>
          </div>
        </div>
        <nav className="mx-auto max-w-3xl overflow-x-auto px-4 pb-2" aria-label="Primary">
          <ul className="flex gap-4 text-sm">
            {NAV_ITEMS.map((item) => (
              <li key={item.to}>
                <NavLink
                  to={item.to}
                  className={({ isActive }) =>
                    `whitespace-nowrap underline-offset-4 hover:underline ${
                      isActive ? 'font-semibold text-white' : 'text-slate-300'
                    }`
                  }
                >
                  {item.label}
                </NavLink>
              </li>
            ))}
          </ul>
        </nav>
      </header>
      <OfflineBanner />
      <main className="mx-auto max-w-3xl px-4 py-6">
        <Outlet />
      </main>
    </div>
  );
}