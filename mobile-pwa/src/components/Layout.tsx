import { NavLink, Outlet } from 'react-router-dom';
import { useAuthStore } from '../stores/authStore';
import { useAuth } from '../hooks/useAuth';
import { OfflineBanner } from './OfflineBanner';
import { LanguageSwitcher } from '../i18n/LanguageSwitcher';
import { useI18n } from '../i18n/I18nContext';

const NAV_ITEMS = [
  { to: '/dashboard', label: 'dashboard.title' },
  { to: '/missions', label: 'nav.missions' },
  { to: '/tasks', label: 'nav.tasks' },
  { to: '/notifications', label: 'nav.notifications' },
  { to: '/approvals', label: 'nav.approvals' },
  { to: '/reports', label: 'nav.reports' },
];

export function ObserverLayout() {
  const { t } = useI18n();
  const user = useAuthStore((state) => state.user);
  const { logout, isPending } = useAuth();

  return (
    <div className="min-h-screen bg-slate-100 text-slate-900">
      <header className="sticky top-0 z-10 border-b border-slate-800 bg-[#0a1e3d] text-white">
        <div className="mx-auto flex max-w-3xl items-center justify-between px-4 py-3">
          <div className="flex items-baseline gap-3">
            <span className="font-bold tracking-wide">ONYX Observer</span>
            <span className="text-xs text-slate-300"">{t("common.readOnly")}</span>
          </div>
          <div className="flex items-center gap-3">
            <LanguageSwitcher />
            <span className="text-sm text-slate-300">{user?.username}</span>
            <button
              type="button"
              className="rounded bg-slate-700 px-2 py-1 text-sm hover:bg-slate-600"
              onClick={() => logout()}
              disabled={isPending}
            >
              {t("auth.signOut")}
            </button>
          </div>
        </div>
        <nav className="mx-auto max-w-3xl overflow-x-auto px-4 pb-2" aria-label={t("nav.primary")}>
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
                  {t(item.label)}
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