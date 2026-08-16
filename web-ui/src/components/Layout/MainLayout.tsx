import { useEffect, useState } from 'react';
import { Outlet, useNavigate } from 'react-router-dom';
import OfflineBanner from '../OfflineBanner';
import StatusBadge from '../StatusBadge';
import Sidebar from './Sidebar';
import { useAuth } from '../../hooks/useAuth';
import { useEventStream } from '../../hooks/useEventStream';
import type { ToastDetail } from '../../utils/errorHandler';

function ToastRegion() {
  const [toast, setToast] = useState<ToastDetail | null>(null);
  useEffect(() => {
    const handler = (event: Event) => {
      setToast((event as CustomEvent<ToastDetail>).detail);
      window.setTimeout(() => setToast(null), 4500);
    };
    window.addEventListener('onyx:toast', handler);
    return () => window.removeEventListener('onyx:toast', handler);
  }, []);
  return toast ? <div className={`toast toast-${toast.tone}`} role="status">{toast.message}</div> : null;
}

export default function MainLayout() {
  const [menuOpen, setMenuOpen] = useState(false);
  const { user, logout } = useAuth();
  const streamStatus = useEventStream();
  const navigate = useNavigate();
  const handleLogout = async () => { await logout(); navigate('/login'); };

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">Skip to main content</a>
      <Sidebar open={menuOpen} onNavigate={() => setMenuOpen(false)} />
      {menuOpen ? <button className="sidebar-scrim" aria-label="Close navigation" onClick={() => setMenuOpen(false)} /> : null}
      <div className="workspace">
        <OfflineBanner />
        <header className="topbar">
          <button className="menu-button" type="button" aria-label="Open navigation" aria-expanded={menuOpen} onClick={() => setMenuOpen((value) => !value)}>☰</button>
          <div className="topbar-context"><span>Organization</span><strong>ONYX Test Operations</strong></div>
          <div className="topbar-actions">
            <StatusBadge status={streamStatus} />
            <div className="user-summary"><span>{user?.username}</span><small>Remote operator</small></div>
            <button className="button-quiet" type="button" onClick={handleLogout}>Sign out</button>
          </div>
        </header>
        <main id="main-content" className="main-content" tabIndex={-1}><Outlet /></main>
      </div>
      <ToastRegion />
    </div>
  );
}
