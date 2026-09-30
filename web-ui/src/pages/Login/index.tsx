import { useI18n } from '../../i18n/I18nContext';
import { FormEvent, useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '../../hooks/useAuth';
import styles from './Login.module.css';

export default function LoginPage() {
  const { t } = useI18n();
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
        <div className="brand-block brand-block-light"><span className="brand-mark">O</span><div><strong>{t('app.name')}</strong><small>{t('app.tagline')}</small></div></div>
        <div><p className="eyebrow eyebrow-light">{t('auth.secureBrowserAccess')}</p><h1>{t('auth.heroTitle')}</h1><p>{t('auth.heroBody')}</p></div>
        <small>Thin client · Server-authoritative · No offline command queue</small>
      </section>
      <section className={styles.panel}>
        <form className={styles.card} onSubmit={submit}>
          <p className="eyebrow">{t('app.remoteOperator')}</p><h1>{t('auth.signInToOnyx')}</h1><p className="muted">{t('auth.useOrgCredentials')}</p>
          <label htmlFor="username">{t('auth.username')}</label><input id="username" value={username} onChange={(e) => setUsername(e.target.value)} autoComplete="username" required />
          <label htmlFor="password">{t('auth.password')}</label><input id="password" type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" required />
          <button className="button-primary button-full" type="submit" disabled={login.isPending}>{login.isPending ? 'Signing in…' : 'Sign in'}</button>
          <div className="security-note" role="note"><strong>{t('auth.sessionSecurity')}</strong><span>{t('auth.sessionNote')}</span></div>
        </form>
      </section>
    </div>
  );
}
