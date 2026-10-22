import { useState } from "react";
import type { FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { apiClient } from "@/api/client";
import { loginWithOnyxCredentials } from "@/auth/credentials";
import {
  classifyLoginFailure,
  createLoginRequestId,
  formatLoginFailureMessage,
  getLoginConfigurationFailure,
  loginFailureForCode,
  recordLoginDiagnostic,
} from "@/auth/loginDiagnostics";
import { getServerAddress } from "@/utils/serverAddress";
import { notifyNetworkError } from "@/utils/errorHandler";
import { LanguageSwitcher } from "@/i18n/LanguageSwitcher";
import { useI18n } from "@/i18n/I18nContext";
import { useAuthStore } from "@/stores/authStore";

export default function Login() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    setLoading(true);
    const startedAt = Date.now();
    const requestId = createLoginRequestId();

    try {
      const configurationFailure = getLoginConfigurationFailure(getServerAddress());
      if (configurationFailure) {
        const failure = loginFailureForCode(configurationFailure, requestId);
        recordLoginDiagnostic({
          outcome: "failure", code: failure.code, stage: failure.stage, severity: failure.severity,
          requestId, durationMs: Date.now() - startedAt, retryable: failure.retryable,
        });
        setError(formatLoginFailureMessage(failure, requestId));
        return;
      }

      let session: Awaited<ReturnType<typeof loginWithOnyxCredentials>>;
      try {
        session = await loginWithOnyxCredentials(
          (url, body, options) => apiClient.post(url, body, options),
          username,
          password,
          requestId,
        );
      } catch (err) {
        const failure = classifyLoginFailure(err, requestId);
        if (failure.stage === "transport") notifyNetworkError();
        recordLoginDiagnostic({
          outcome: "failure", code: failure.code, stage: failure.stage, severity: failure.severity,
          requestId: failure.requestId, httpStatus: failure.httpStatus, transportCode: failure.transportCode,
          serverCode: failure.serverCode, durationMs: Date.now() - startedAt, retryable: failure.retryable,
        });
        setError(formatLoginFailureMessage(
          failure,
          requestId,
          failure.code === "LOGIN_AUTH_REJECTED" ? t("auth.adminInvalidCredentials") : undefined,
        ));
        return;
      }

      try {
        useAuthStore.getState().login(session);
      } catch {
        // Best-effort cleanup if storage accepted some session fields before
        // another field failed; never let cleanup hide the original failure.
        try {
          useAuthStore.getState().logout();
        } catch {
          // Session storage itself may be unavailable.
        }
        const failure = loginFailureForCode("LOGIN_SESSION_PERSISTENCE_FAILED", requestId);
        recordLoginDiagnostic({
          outcome: "failure", code: failure.code, stage: failure.stage, severity: failure.severity,
          requestId, durationMs: Date.now() - startedAt, retryable: failure.retryable,
        });
        setError(formatLoginFailureMessage(failure, requestId));
        return;
      }

      try {
        await navigate("/", { replace: true });
      } catch {
        const failure = loginFailureForCode("LOGIN_NAVIGATION_FAILED", requestId);
        recordLoginDiagnostic({
          outcome: "failure", code: failure.code, stage: failure.stage, severity: failure.severity,
          requestId, durationMs: Date.now() - startedAt, retryable: failure.retryable,
        });
        setError(formatLoginFailureMessage(failure, requestId));
        return;
      }

      recordLoginDiagnostic({
        outcome: "success", code: "LOGIN_SUCCESS", stage: "navigation", severity: "info",
        requestId, durationMs: Date.now() - startedAt, retryable: false,
      });
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
          <p className="mt-5 max-w-lg text-sm leading-6 text-sky-50/85">{t("auth.adminCredentialsDescription")}</p>
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
          </form>
        </div>
      </section>
    </div>
  );
}

