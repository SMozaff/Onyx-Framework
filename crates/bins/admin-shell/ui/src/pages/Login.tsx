import { useState } from "react";
import type { FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { apiClient } from "@/api/client";
import ConnectionSettings from "@/components/ConnectionSettings";
import { useAuthStore } from "@/stores/authStore";
import { getServerAddress } from "@/utils/serverAddress";

/**
 * Admin login retains server configuration before authentication. The separate
 * connection setup component verifies a health endpoint before persisting an
 * address, so a wrong server cannot masquerade as invalid credentials.
 */
export default function Login() {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [showServerSettings, setShowServerSettings] = useState(false);
  const navigate = useNavigate();

  async function submit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    setLoading(true);
    try {
      const response = await apiClient.post("/api/auth/login", { username, password });
      useAuthStore.getState().login(response.data);
      navigate("/", { replace: true });
    } catch (err) {
      if (isNetworkError(err)) {
        setError(`Could not reach the server at ${getServerAddress()}. Check the connection setup below, or confirm the server is running and reachable.`);
        setShowServerSettings(true);
      } else {
        setError("Invalid username or password.");
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
            <p className="text-[0.72rem] font-extrabold tracking-[0.24em] text-white">ONYX</p>
            <p className="mt-0.5 text-[0.62rem] text-sky-100/70">Administration</p>
          </div>
        </div>
        <div className="onyx-auth-copy">
          <p className="text-[0.72rem] font-extrabold tracking-[0.19em] text-sky-100/90">SECURE ADMIN ACCESS</p>
          <h2 id="admin-signin-context" className="mt-4 max-w-md text-4xl font-light leading-[1.03] tracking-[-0.045em] text-white sm:text-5xl">
            Govern operations with calm, visible control.
          </h2>
          <p className="mt-5 max-w-lg text-sm leading-6 text-sky-50/85">
            Manage organization policy, access, and staff profiles from a dedicated administrative workspace.
          </p>
        </div>
        <p className="relative z-10 text-[0.68rem] text-sky-100/75">Organization-scoped administration · Explicit server verification</p>
      </section>

      <section className="flex flex-1 items-center justify-center px-5 py-10 sm:px-10 lg:px-16">
        <form onSubmit={submit} className="onyx-auth-card p-6 sm:p-7">
          <p className="text-[0.66rem] font-extrabold tracking-[0.16em] text-onyx-accent">ADMINISTRATOR</p>
          <h1 className="mt-3 text-3xl font-medium tracking-[-0.04em] text-onyx-text">Sign in to ONYX</h1>
          <p className="mt-2 text-sm leading-5 text-onyx-text-dim">Use an administrator account for this organization.</p>

          <div className="mt-6">
            <label htmlFor="username" className="block text-xs font-bold text-onyx-text">Username</label>
            <input
              id="username"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              autoComplete="username"
              required
              className="mt-1.5 w-full rounded-lg border border-onyx-border bg-white px-3 py-2.5 text-sm text-onyx-text shadow-sm placeholder:text-slate-400 focus:border-onyx-accent focus:outline-none"
            />
          </div>
          <div className="mt-4">
            <label htmlFor="password" className="block text-xs font-bold text-onyx-text">Password</label>
            <input
              id="password"
              type="password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              autoComplete="current-password"
              required
              className="mt-1.5 w-full rounded-lg border border-onyx-border bg-white px-3 py-2.5 text-sm text-onyx-text shadow-sm placeholder:text-slate-400 focus:border-onyx-accent focus:outline-none"
            />
          </div>
          {error && <p className="mt-4 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-xs leading-5 text-onyx-status-blocked" role="alert">{error}</p>}
          <button type="submit" disabled={loading} className="mt-5 w-full rounded-lg bg-onyx-accent px-3 py-2.5 text-sm font-bold text-white shadow-sm transition-colors hover:bg-[#174d7b] disabled:cursor-not-allowed disabled:opacity-50">
            {loading ? "Signing in…" : "Sign in"}
          </button>

          <div className="mt-5 rounded-lg border border-sky-100 bg-sky-50/75 p-3">
            <p className="text-xs font-bold text-onyx-text">Connection security</p>
            <p className="mt-1 text-[0.68rem] leading-4 text-onyx-text-dim">A server address is only stored after its health endpoint responds successfully.</p>
          </div>

          <button
            type="button"
            onClick={() => setShowServerSettings((value) => !value)}
            className="mt-4 w-full text-center text-xs font-semibold text-onyx-accent underline decoration-dotted underline-offset-4 hover:text-[#174d7b]"
            aria-expanded={showServerSettings}
          >
            {showServerSettings ? "Hide connection setup" : "Server address / connection settings"}
          </button>
          {showServerSettings && <ConnectionSettings />}
        </form>
      </section>
    </div>
  );
}

function isNetworkError(error: unknown): boolean {
  return typeof error === "object" && error !== null && !("response" in error);
}
