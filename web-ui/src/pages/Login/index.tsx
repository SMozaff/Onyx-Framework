import { FormEvent, useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '../../hooks/useAuth';
import styles from './Login.module.css';

export default function LoginPage() {
  // Audit fix H-01: these fields previously shipped prefilled with the
  // demo credentials `operator`/`onyx`, which published a working login to
  // anyone who opened the page. They now start empty.
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const { isAuthenticated, login } = useAuth();
  const navigate = useNavigate();
  useEffect(() => { if (isAuthenticated) navigate('/', { replace: true }); }, [isAuthenticated, navigate]);
  const submit = (event: FormEvent) => { event.preventDefault(); login.mutate({ username, password }); };
  return (
    <div className={styles.page}>
      <section className={styles.hero} aria-label="ONYX product introduction">
        <div className="brand-block brand-block-light"><span className="brand-mark">O</span><div><strong>ONYX</strong><small>Mission Operations</small></div></div>
        <div><p className="eyebrow eyebrow-light">Secure browser access</p><h1>Operational clarity, from any authorized browser.</h1><p>Review missions, monitor critical work, acknowledge notifications, and make permitted approval decisions without installing a native replica.</p></div>
        <small>Thin client · Server-authoritative · No offline command queue</small>
      </section>
      <section className={styles.panel}>
        <form className={styles.card} onSubmit={submit}>
          <p className="eyebrow">Remote Operator</p><h1>Sign in to ONYX</h1><p className="muted">Use your organization credentials. This browser stores tokens for this session only.</p>
          <label htmlFor="username">Username</label><input id="username" value={username} onChange={(e) => setUsername(e.target.value)} autoComplete="username" required />
          <label htmlFor="password">Password</label><input id="password" type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" required />
          <button className="button-primary button-full" type="submit" disabled={login.isPending}>{login.isPending ? 'Signing in…' : 'Sign in'}</button>
          <div className="security-note" role="note"><strong>Session security</strong><span>Expired access requires a new sign-in. Refresh rotation is intentionally disabled in v1.</span></div>
        </form>
      </section>
    </div>
  );
}
