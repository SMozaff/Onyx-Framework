import { NavLink } from 'react-router-dom';

const links = [
  ['/', 'Overview'],
  ['/missions', 'Missions'],
  ['/tasks', 'Tasks'],
  ['/todos', 'Todos & Targets'],
  ['/notifications', 'Notifications'],
  ['/approvals', 'Approvals'],
  ['/reports', 'Reports'],
] as const;

export default function Sidebar({ open, onNavigate }: { open: boolean; onNavigate: () => void }) {
  return (
    <aside className={`sidebar ${open ? 'sidebar-open' : ''}`} aria-label="Primary navigation">
      <div className="brand-block" aria-label="ONYX Remote Operator">
        <span className="brand-mark" aria-hidden="true">O</span>
        <div><strong>ONYX</strong><small>Remote Operator</small></div>
      </div>
      <nav>
        {links.map(([to, label]) => (
          <NavLink key={to} to={to} end={to === '/'} onClick={onNavigate} className={({ isActive }) => isActive ? 'nav-link nav-link-active' : 'nav-link'}>
            <span className="nav-indicator" aria-hidden="true" />{label}
          </NavLink>
        ))}
      </nav>
      <div className="sidebar-note"><strong>Thin client</strong><span>No offline commands or local domain state.</span></div>
    </aside>
  );
}
