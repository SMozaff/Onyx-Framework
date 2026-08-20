import { useEffect, useRef, useState } from 'react';
import { Outlet, useNavigate } from 'react-router-dom';
import OfflineBanner from '../OfflineBanner';
import StatusBadge from '../StatusBadge';
import Sidebar from './Sidebar';
import { useAuth } from '../../hooks/useAuth';
import { useEventStream } from '../../hooks/useEventStream';
import { useMediaQuery } from '../../hooks/useMediaQuery';
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

function organizationLabel(user: { organization_id: string; organization_display_name?: string } | null): string {
  if (!user) return 'Organization context unavailable';
  if (user.organization_display_name?.trim()) return user.organization_display_name;
  return `Organization ${user.organization_id.slice(0, 8)}`;
}

export default function MainLayout() {
  const [menuOpen, setMenuOpen] = useState(false);
  const { user, logout } = useAuth();
  const streamStatus = useEventStream();
  const navigate = useNavigate();
  const mobile = useMediaQuery('(max-width: 820px)');
  const menuButtonRef = useRef<HTMLButtonElement>(null);
  const firstLinkRef = useRef<HTMLAnchorElement>(null);
  const sidebarRef = useRef<HTMLElement>(null);
  const mainRef = useRef<HTMLElement>(null);
  const menuWasOpen = useRef(false);

  const handleLogout = async () => { await logout(); navigate('/login'); };
  const closeMenu = () => setMenuOpen(false);
  const handleNavigate = () => {
    setMenuOpen(false);
    window.setTimeout(() => mainRef.current?.focus(), 0);
  };

  useEffect(() => {
    if (!mobile) setMenuOpen(false);
  }, [mobile]);

  useEffect(() => {
    sidebarRef.current?.toggleAttribute('inert', mobile && !menuOpen);
  }, [mobile, menuOpen]);

  useEffect(() => {
    if (!mobile) {
      menuWasOpen.current = false;
      return;
    }
    if (menuOpen && !menuWasOpen.current) firstLinkRef.current?.focus();
    if (!menuOpen && menuWasOpen.current) menuButtonRef.current?.focus();
    menuWasOpen.current = menuOpen;
  }, [mobile, menuOpen]);

  useEffect(() => {
    if (!mobile || !menuOpen) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') closeMenu();
    };
    window.addEventListener('keydown', closeOnEscape);
    return () => window.removeEventListener('keydown', closeOnEscape);
  }, [mobile, menuOpen]);

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">Skip to main content</a>
      <Sidebar ref={sidebarRef} open={menuOpen} mobile={mobile} firstLinkRef={firstLinkRef} onNavigate={handleNavigate} />
      {mobile && menuOpen ? <button className="sidebar-scrim" type="button" aria-label="Close navigation" onClick={closeMenu} /> : null}
      <div className="workspace">
        <OfflineBanner />
        <header className="topbar">
          <button ref={menuButtonRef} className="menu-button" type="button" aria-label={menuOpen ? 'Close navigation' : 'Open navigation'} aria-controls="primary-navigation" aria-expanded={mobile ? menuOpen : undefined} onClick={() => setMenuOpen((value) => !value)}><span aria-hidden="true">☰</span></button>
          <div className="topbar-context" aria-label="Current organization"><span>Organization</span><strong title={user?.organization_id}>{organizationLabel(user)}</strong>{!user?.organization_display_name && <small>Verify organization before acting.</small>}</div>
          <div className="topbar-actions">
            <StatusBadge status={streamStatus} />
            <div className="user-summary"><span>{user?.username}</span><small>Remote operator</small></div>
            <button className="button-quiet" type="button" onClick={handleLogout}>Sign out</button>
          </div>
        </header>
        <main ref={mainRef} id="main-content" className="main-content" tabIndex={-1}><Outlet /></main>
      </div>
      <ToastRegion />
    </div>
  );
}
