import { forwardRef, type Ref } from 'react';
import { NavLink } from 'react-router-dom';
import { useI18n } from '../../i18n/I18nContext';
import { LanguageSwitcher } from '../../i18n/LanguageSwitcher';

const linkDefs = [
  ['/', 'nav.overview'],
  ['/missions', 'nav.missions'],
  ['/tasks', 'nav.tasks'],
  ['/todos', 'nav.todos'],
  ['/staff-loans', 'nav.staffLoans'],
  ['/notifications', 'nav.notifications'],
  ['/approvals', 'nav.approvals'],
  ['/reports', 'nav.reports'],
] as const;

interface SidebarProps {
  open: boolean;
  mobile: boolean;
  firstLinkRef: Ref<HTMLAnchorElement>;
  onNavigate: () => void;
}

const Sidebar = forwardRef<HTMLElement, SidebarProps>(function Sidebar({ open, mobile, firstLinkRef, onNavigate }, ref) {
  const closedMobileDrawer = mobile && !open;
  const { t } = useI18n();
  return (
    <aside
      ref={ref}
      id="primary-navigation"
      className={`sidebar ${open ? 'sidebar-open' : ''}`}
      aria-label={t('nav.primary')}
      aria-hidden={closedMobileDrawer || undefined}
    >
      <div className="brand-block" aria-label={t('app.remoteOperator')}>
        <span className="brand-mark" aria-hidden="true">O</span>
        <div><strong>{t('app.name')}</strong><small>{t('app.remoteOperator')}</small></div>
      </div>
      <nav>
        {linkDefs.map(([to, key], index) => (
          <NavLink
            key={to}
            ref={index === 0 ? firstLinkRef : undefined}
            to={to}
            end={to === '/'}
            tabIndex={closedMobileDrawer ? -1 : undefined}
            onClick={onNavigate}
            className={({ isActive }) => isActive ? 'nav-link nav-link-active' : 'nav-link'}
          >
            <span className="nav-indicator" aria-hidden="true" />{t(key)}
          </NavLink>
        ))}
      </nav>
      <div className="sidebar-note"><strong>{t('app.thinClient')}</strong><span>{t('app.thinClientNote')}</span></div>
      <div style={{ marginTop: 12 }}><LanguageSwitcher /></div>
    </aside>
  );
});

export default Sidebar;
