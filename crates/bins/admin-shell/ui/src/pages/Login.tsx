import { useEffect, useState } from "react";
import { useI18n } from "@/i18n/I18nContext";
import type { FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { apiClient } from "@/api/client";
import { LanguageSwitcher } from "@/i18n/LanguageSwitcher";
import { useAuthStore } from "@/stores/authStore";
import {
  AllfatherOAuthError,
  beginAllfatherSignIn,
  completeAllfatherCallback,
  parseCallback,
  stripOAuthParams,
} from "@/auth/allfatherOAuth";


type LoginMode = "account" | "allfather";

export default function Login() {
  const { t } = useI18n();
  const navigate = useNavigate();

  const [mode, setMode] = useState<LoginMode>(() => new URLSearchParams(window.location.search).get("auth") === "allfather" ? "allfather" : "account");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [googleAuthReady, setGoogleAuthReady] = useState(false);

  useEffect(() => {
    if (mode !== "allfather") return;
    let cancelled = false;
    async function finishOAuthCallback() {
      // Capture the full callback URL (code + state) before rewriting the
      // address bar, then strip the sensitive parameters immediately so the
      // authorization code never lingers in history, a referrer, or a
      // forwarded URL.
      const callbackUrl = window.location.href;
      const params = parseCallback(callbackUrl);
      if (!params.code && !params.error && !params.legacyTokenInFragment) {
        setGoogleAuthReady(true);
        return;
      }
      window.history.replaceState({}, document.title, stripOAuthParams(callbackUrl));
      setLoading(true);

      const supabaseUrl = (import.meta.env.VITE_SUPABASE_URL as string | undefined)?.replace(/\/+$/, "");
      const publishableKey = import.meta.env.VITE_SUPABASE_PUBLISHABLE_KEY as string | undefined;
      if (!supabaseUrl || !publishableKey) {
        throw new AllfatherOAuthError("MISSING_CONFIG", "Supabase is not configured in this build");
      }

      // PKCE: the stored verifier + state are validated here, then the code is
      // exchanged for a Supabase session using only the publishable key.
      const { accessToken } = await completeAllfatherCallback({
        supabaseUrl,
        publishableKey,
        callbackUrl,
      });
      // The Worker — not the browser — decides whether this Supabase subject is
      // mapped to the provisioned ONYX All-Father principal (AUTH-01).
      const response = await apiClient.post("/api/auth/supabase", null, {
        headers: { Authorization: `Bearer ${accessToken}` },
      });
      if (cancelled) return;
      useAuthStore.getState().login(response.data);
      navigate("/", { replace: true });
    }

    void finishOAuthCallback().catch((err: unknown) => {
      // Generic, localized message only: the underlying cause (state mismatch,
      // expired code, provider error body) is deliberately not surfaced.
      if (!cancelled) setError(oauthErrorMessage(err, t));
    }).finally(() => {
      if (!cancelled) setLoading(false);
    });

    return () => { cancelled = true; };
  }, [mode, navigate, t]);

  function beginGoogleSignIn() {
    const supabaseUrl = (import.meta.env.VITE_SUPABASE_URL as string | undefined)?.replace(/\/+$/, "");
    const publishableKey = import.meta.env.VITE_SUPABASE_PUBLISHABLE_KEY as string | undefined;
    if (!supabaseUrl || !publishableKey) {
      setError(t("auth.googleSignInNotConfigured"));
      return;
    }
    setError(null);
    // Authorization Code Flow with PKCE: the verifier, its S256 challenge and
    // the CSRF nonce are created here and stored for this tab only. No secret
    // key is involved — the exchange later uses just the publishable key.
    void beginAllfatherSignIn({
      supabaseUrl,
      publishableKey,
      redirectTo: window.location.origin + window.location.pathname,
    }).then(({ authorizeUrl }) => {
      window.location.assign(authorizeUrl);
    }).catch((err: unknown) => {
      setError(oauthErrorMessage(err, t));
    });
  }

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
        setError("Could not connect to the ONYX Cloud service. Check your internet connection and try again.");
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
              <button type="button" onClick={() => { setError(null); setGoogleAuthReady(false); setMode("allfather"); }} className="mt-4 w-full text-center text-xs font-semibold text-onyx-accent underline decoration-dotted underline-offset-4">
                {t("auth.allFatherGoogleSignIn")}
              </button>
            </form>
          ) : (
            <div>
              <p className="text-[0.66rem] font-extrabold tracking-[0.16em] text-onyx-accent">{t("auth.allFatherAuthority")}</p>
              <h2 className="mt-3 text-3xl font-medium tracking-[-0.04em] text-onyx-text">{t("auth.googleThroughSupabase")}</h2>
              <p className="mt-2 text-sm leading-5 text-onyx-text-dim">{t("auth.allFatherReservedPath")}</p>
              <p className="mt-4 text-xs text-onyx-text-dim">{googleAuthReady ? t("auth.googleIdentityPrompt") : t("auth.checkingGoogleSignIn")}</p>
              <button type="button" onClick={beginGoogleSignIn} disabled={loading} className="mt-4 w-full rounded-lg bg-onyx-accent px-3 py-2.5 text-sm font-bold text-white shadow-sm disabled:opacity-50">{loading ? t("auth.signingIn") : t("auth.continueWithGoogle")}</button>
              <button type="button" onClick={() => { setError(null); setMode("account"); }} className="mt-4 w-full text-center text-xs font-semibold text-onyx-accent underline decoration-dotted underline-offset-4">
                {t("auth.backToAdminLogin")}
              </button>
            </div>
          )}

        </div>
      </section>
    </div>
  );
}

function isNetworkError(error: unknown): boolean {
  return typeof error === "object" && error !== null && !("response" in error);
}

/**
 * Maps PKCE failure codes to localized copy. Every branch is generic: the
 * server-side reason (state mismatch, expired code, provider response body) is
 * never shown to the client, matching the Worker's stable-error-code contract.
 */
function oauthErrorMessage(error: unknown, t: (key: string) => string): string {
  if (!(error instanceof AllfatherOAuthError)) return t("auth.googleSignInFailed");
  if (error.code === "MISSING_CONFIG") return t("auth.googleSignInNotConfigured");
  if (error.code === "PROVIDER_DENIED") return t("auth.googleSignInCancelled");
  return t("auth.googleSignInFailed");
}
