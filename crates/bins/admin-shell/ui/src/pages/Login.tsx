import { useEffect, useRef, useState } from "react";
import { useI18n } from "@/i18n/I18nContext";
import type { FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { apiClient } from "@/api/client";
import ConnectionSettings from "@/components/ConnectionSettings";
import { LanguageSwitcher } from "@/i18n/LanguageSwitcher";
import { useAuthStore } from "@/stores/authStore";
import { getServerAddress } from "@/utils/serverAddress";
import { loadClerk } from "@/auth/clerk";

type LoginMode = "account" | "allfather";

export default function Login() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const signInRef = useRef<HTMLDivElement>(null);
  const [mode, setMode] = useState<LoginMode>("account");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [showServerSettings, setShowServerSettings] = useState(false);
  const [clerkReady, setClerkReady] = useState(false);

  useEffect(() => {
    if (mode !== "allfather") return;
    let cancelled = false;
    let cleanup: (() => void) | undefined;

    void loadClerk()
      .then((clerk) => {
        if (cancelled || !signInRef.current) return;
        clerk.mountSignIn(signInRef.current, {
          appearance: { elements: { rootBox: "w-full" } },
        });
        setClerkReady(true);

        const exchange = async (session: { getToken: () => Promise<string | null> } | null) => {
          if (!session || cancelled) return;
          setLoading(true);
          setError(null);
          try {
            const token = await session.getToken();
            if (!token) throw new Error("Clerk did not provide a session token.");
            const response = await apiClient.post("/api/auth/clerk", null, {
              headers: { Authorization: `Bearer ${token}` },
            });
            useAuthStore.getState().login(response.data);
            navigate("/", { replace: true });
          } catch (err) {
            const status = (err as { response?: { status?: number } }).response?.status;
            setError(
              status === 403
                ? "This Google identity is not the designated ONYX All-Father."
                : status === 401
                  ? "The Clerk session could not be verified by ONYX."
                  : "Could not exchange the Clerk session with the ONYX API.",
            );
          } finally {
            if (!cancelled) setLoading(false);
          }
        };

        cleanup = clerk.addListener(({ session }) => void exchange(session));
        void exchange(clerk.session);
      })
      .catch((err: Error) => {
        if (!cancelled) setError(err.message);
      });

    return () => {
      cancelled = true;
      cleanup?.();
      setClerkReady(false);
      if (signInRef.current) {
        void loadClerk().then((clerk) => {
          if (signInRef.current) clerk.unmountSignIn(signInRef.current);
        });
      }
    };
  }, [mode, navigate]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    setLoading(true);
    try {
      const response = await apiClient.post("/api/auth/login", {
        username,
        password,
        client_type: "admin",
      });
      useAuthStore.getState().login(response.data);
      navigate("/", { replace: true });
    } catch (err) {
      if (isNetworkError(err)) {
        setError(`Could not reach the server at ${getServerAddress()}. Check the connection setup below, or confirm the server is running and reachable.`);
        setShowServerSettings(true);
      } else {
        setError(t("auth.adminInvalidCredentials"));
      }
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="onyx-auth-shell flex flex-col lg:flex-row">
      <section className="onyx-auth-aside" aria-labelledby="admin-signin-context">
        <div className="relative z-10 flex items-center gap-2.5">
          <span className="onyx-brand-mark" aria-hidden="true">O</span>
          <div>
            <p className="text-[0.72rem] font-extrabold tracking-[0.24em] text-white">{t("app.name")}</p>
            <p className="mt-0.5 text-[0.62rem] text-sky-100/70">{t("common.administration")}</p>
          </div>
        </div>
        <div className="onyx-auth-copy">
          <p className="text-[0.72rem] font-extrabold tracking-[0.19em] text-sky-100/90">{t("auth.adminSecureAccess")}</p>
          <h2 id="admin-signin-context" className="mt-4 max-w-md text-4xl font-light leading-[1.03] tracking-[-0.045em] text-white sm:text-5xl">
            {t("auth.adminHeroHeadline")}
          </h2>
          <p className="mt-5 max-w-lg text-sm leading-6 text-sky-50/85">
            {t("auth.adminCredentialsDescription")}
          </p>
        </div>
        <p className="relative z-10 text-[0.68rem] text-sky-100/75">{t("auth.adminOrganizationAdministration")}</p>
      </section>

      <section className="flex flex-1 items-center justify-center px-5 py-10 sm:px-10 lg:px-16">
        <div className="onyx-auth-card w-full max-w-md p-6 sm:p-7">
          <div className="mb-5 flex items-center justify-between">
            <h1 className="text-lg font-semibold text-onyx-text">{t("auth.adminTitleBar")}</h1>
            <LanguageSwitcher />
          </div>

          {error && <p className="mb-4 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-xs leading-5 text-onyx-status-blocked" role="alert">{error}</p>}

          {mode === "account" ? (
            <form onSubmit={submit}>
              <p className="text-[0.66rem] font-extrabold tracking-[0.16em] text-onyx-accent">{t("auth.adminTitle")}</p>
              <h2 className="mt-3 text-3xl font-medium tracking-[-0.04em] text-onyx-text">{t("auth.adminSignIn")}</h2>
              <p className="mt-2 text-sm leading-5 text-onyx-text-dim">{t("auth.useAssignedCredentials")}</p>

              <div className="mt-6">
                <label htmlFor="username" className="block text-xs font-bold text-onyx-text">{t("auth.username")}</label>
                <input id="username" value={username} onChange={(event) => setUsername(event.target.value)} autoComplete="username" required className="mt-1.5 w-full rounded-lg border border-onyx-border bg-white px-3 py-2.5 text-sm text-onyx-text shadow-sm placeholder:text-slate-400 focus:border-onyx-accent focus:outline-none" />
              </div>
              <div className="mt-4">
                <label htmlFor="password" className="block text-xs font-bold text-onyx-text">{t("auth.password")}</label>
                <input id="password" type="password" value={password} onChange={(event) => setPassword(event.target.value)} autoComplete="current-password" required className="mt-1.5 w-full rounded-lg border border-onyx-border bg-white px-3 py-2.5 text-sm text-onyx-text shadow-sm placeholder:text-slate-400 focus:border-onyx-accent focus:outline-none" />
              </div>
              <button type="submit" disabled={loading} className="mt-5 w-full rounded-lg bg-onyx-accent px-3 py-2.5 text-sm font-bold text-white shadow-sm disabled:cursor-not-allowed disabled:opacity-50">
                {loading ? t("auth.signingIn") : t("auth.signIn")}
              </button>
              <button type="button" onClick={() => { setError(null); setMode("allfather"); }} className="mt-4 w-full text-center text-xs font-semibold text-onyx-accent underline decoration-dotted underline-offset-4">
                All-Father · Sign in with Google
              </button>
            </form>
          ) : (
            <div>
              <p className="text-[0.66rem] font-extrabold tracking-[0.16em] text-onyx-accent">{t("auth.allFatherAuthority")}</p>
              <h2 className="mt-3 text-3xl font-medium tracking-[-0.04em] text-onyx-text">{t("auth.googleThroughClerk")}</h2>
              <p className="mt-2 text-sm leading-5 text-onyx-text-dim">{t("auth.allFatherReservedPath")}</p>
              {!clerkReady && <p className="mt-4 text-xs text-onyx-text-dim">{t("auth.initializingGoogleAuth")}</p>}
              <div ref={signInRef} className="mt-4 min-h-[300px]" />
              <button type="button" onClick={() => { setError(null); setMode("account"); }} className="mt-4 w-full text-center text-xs font-semibold text-onyx-accent underline decoration-dotted underline-offset-4">
                {t("auth.backToAdminLogin")}
              </button>
            </div>
          )}

          <button
            type="button"
            className="mt-5 w-full text-center text-xs font-semibold text-onyx-accent underline decoration-dotted underline-offset-4"
            onClick={() => setShowServerSettings((value) => !value)}
            aria-expanded={showServerSettings}
          >
            {showServerSettings ? t("auth.hideServerAddress") : t("auth.serverConnectionSettings")}
          </button>
          {showServerSettings && <ConnectionSettings />}
          <p className="mt-3 text-xs text-onyx-text-dim">Current API: {getServerAddress()}</p>
        </div>
      </section>
    </div>
  );
}

function isNetworkError(error: unknown): boolean {
  return typeof error === "object" && error !== null && !("response" in error);
}
