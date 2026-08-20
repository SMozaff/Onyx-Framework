import { forwardRef, type Ref } from 'react';
import { NavLink } from 'react-router-dom';

const links = [
  ['/', 'Overview'],
  ['/missions', 'Missions'],
  ['/tasks', 'Tasks'],
  ['/todos', 'Todos & Targets'],
  ['/staff-loans', 'Staff Loans'],
  ['/notifications', 'Notifications'],
  ['/approvals', 'Approvals'],
  ['/reports', 'Reports'],
] as const;

interface SidebarProps {
  open: boolean;
  mobile: boolean;
  firstLinkRef: Ref<HTMLAnchorElement>;
  onNavigate: () => void;
}

const Sidebar = forwardRef<HTMLElement, SidebarProps>(function Sidebar({ open, mobile, firstLinkRef, onNavigate }, ref) {
  const closedMobileDrawer = mobile && !open;
  return (
    <aside
      ref={ref}
      id="primary-navigation"
      className={`sidebar ${open ? 'sidebar-open' : ''}`}
      aria-label="Primary navigation"
      aria-hidden={closedMobileDrawer || undefined}
    >
      <div className="brand-block" aria-label="ONYX Remote Operator">
        <span className="brand-mark" aria-hidden="true">O</span>
        <div><strong>ONYX</strong><small>Remote Operator</small></div>
      </div>
      <nav>
        {links.map(([to, label], index) => (
          <NavLink
            key={to}
            ref={index === 0 ? firstLinkRef : undefined}
            to={to}
            end={to === '/'}
            tabIndex={closedMobileDrawer ? -1 : undefined}
            onClick={onNavigate}
            className={({ isActive }) => isActive ? 'nav-link nav-link-active' : 'nav-link'}
          >
            <span className="nav-indicator" aria-hidden="true" />{label}
          </NavLink>
        ))}
      </nav>
      <div className="sidebar-note"><strong>Thin client</strong><span>No offline commands or local domain state.</span></div>
    </aside>
  );
});

export default Sidebar;
