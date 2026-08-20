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
    <div className="flex min-h-screen items-center justify-center bg-onyx-bg px-4">
      <form onSubmit={submit} className="w-full max-w-sm rounded-lg border border-onyx-border bg-onyx-surface p-6">
        <h1 className="text-lg font-semibold text-onyx-text">ONYX Admin</h1>
        <p className="mt-1 text-sm text-onyx-text-dim">Sign in with your administrator account.</p>
        <div className="mt-4"><label htmlFor="username" className="block text-xs font-medium text-onyx-text-dim">Username</label><input id="username" value={username} onChange={(event) => setUsername(event.target.value)} autoComplete="username" required className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none" /></div>
        <div className="mt-3"><label htmlFor="password" className="block text-xs font-medium text-onyx-text-dim">Password</label><input id="password" type="password" value={password} onChange={(event) => setPassword(event.target.value)} autoComplete="current-password" required className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none" /></div>
        {error && <p className="mt-3 text-xs text-onyx-status-blocked" role="alert">{error}</p>}
        <button type="submit" disabled={loading} className="mt-4 w-full rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50">{loading ? "Signing in…" : "Sign in"}</button>
        <button type="button" onClick={() => setShowServerSettings((value) => !value)} className="mt-3 w-full text-center text-xs text-onyx-text-dim underline decoration-dotted hover:text-onyx-text">{showServerSettings ? "Hide connection setup" : "Server address / connection settings"}</button>
        {showServerSettings && <ConnectionSettings />}
      </form>
    </div>
  );
}

function isNetworkError(error: unknown): boolean {
  return typeof error === "object" && error !== null && !("response" in error);
}
