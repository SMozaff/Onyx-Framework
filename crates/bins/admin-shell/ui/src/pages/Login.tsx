import { useState } from "react";
import type { FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { apiClient } from "@/api/client";
import { useAuthStore } from "@/stores/authStore";
import { getServerAddress, setServerAddress, isPlausibleServerAddress } from "@/utils/serverAddress";

/**
 * Admin platform login. Calls `/api/auth/login` directly (no
 * react-query wrapper — this app is small enough that a plain
 * `useState`/`fetch` flow is simpler than adding a query library for
 * one mutation) and stores the session via `useAuthStore.login`, which
 * now also carries `is_admin`/`class` (see `App.tsx`'s doc comment) so
 * the post-login gate can immediately tell a non-admin they're in the
 * wrong place.
 *
 * Also hosts a collapsed "Server address" section. The full
 * connection-settings UI lives in `Settings.tsx`, but that page is
 * behind the login wall (`ProtectedLayout`) — someone whose app is
 * still pointed at the wrong address can't log in yet, so they'd never
 * be able to reach it. This lets the address be fixed before ever
 * successfully logging in, without weakening the auth gate itself.
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
      // Distinguish "server unreachable" from "server rejected the
      // credentials" — previously both showed the same generic
      // message, which made a wrong server address look identical to
      // a wrong password and was genuinely misleading to debug.
      if (isNetworkError(err)) {
        setError(
          `Could not reach the server at ${getServerAddress()}. ` +
            `Check the address below, or confirm the server is running and reachable.`,
        );
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
      <form
        onSubmit={submit}
        className="w-full max-w-sm rounded-lg border border-onyx-border bg-onyx-surface p-6"
      >
        <h1 className="text-lg font-semibold text-onyx-text">ONYX Admin</h1>
        <p className="mt-1 text-sm text-onyx-text-dim">
          Sign in with your administrator account.
        </p>

        <div className="mt-4">
          <label htmlFor="username" className="block text-xs font-medium text-onyx-text-dim">
            Username
          </label>
          <input
            id="username"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            autoComplete="username"
            required
            className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
          />
        </div>

        <div className="mt-3">
          <label htmlFor="password" className="block text-xs font-medium text-onyx-text-dim">
            Password
          </label>
          <input
            id="password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="current-password"
            required
            className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
          />
        </div>

        {error && <p className="mt-3 text-xs text-onyx-status-blocked">{error}</p>}

        <button
          type="submit"
          disabled={loading}
          className="mt-4 w-full rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
        >
          {loading ? "Signing in…" : "Sign in"}
        </button>

        <button
          type="button"
          onClick={() => setShowServerSettings((v) => !v)}
          className="mt-3 w-full text-center text-xs text-onyx-text-dim underline decoration-dotted hover:text-onyx-text"
        >
          {showServerSettings ? "Hide server address" : "Server address / connection settings"}
        </button>

        {showServerSettings && <ServerAddressField />}
      </form>
    </div>
  );
}

/**
 * True for a network-level failure (DNS/refused/timeout — i.e. no
 * response ever came back), false for an actual HTTP error response
 * from the server (e.g. 401 for bad credentials). Axios sets
 * `error.response` only when a response was received.
 */
function isNetworkError(err: unknown): boolean {
  return (
    typeof err === "object" &&
    err !== null &&
    "response" in err === false
  );
}

function ServerAddressField() {
  const [value, setValue] = useState(() => getServerAddress());
  const [status, setStatus] = useState<"idle" | "testing" | "ok" | "error">("idle");
  const [message, setMessage] = useState<string | null>(null);

  async function testConnection(address: string): Promise<boolean> {
    try {
      const response = await fetch(`${address.replace(/\/+$/, "")}/health`, {
        signal: AbortSignal.timeout(5_000),
      });
      return response.ok;
    } catch {
      return false;
    }
  }

  async function handleSave() {
    if (!isPlausibleServerAddress(value)) {
      setStatus("error");
      setMessage("Enter a full address including http:// or https://");
      return;
    }
    setStatus("testing");
    setMessage(null);
    const reachable = await testConnection(value);
    if (!reachable) {
      setStatus("error");
      setMessage("Could not reach a server at this address. Not saved.");
      return;
    }
    setServerAddress(value);
    setStatus("ok");
    setMessage("Saved. Try signing in again.");
  }

  return (
    <div className="mt-3 rounded-md border border-onyx-border bg-onyx-bg p-3">
      <label className="block text-xs font-medium text-onyx-text-dim">
        Server address
      </label>
      <p className="mt-1 text-[11px] text-onyx-text-dim">
        e.g. http://192.168.0.250:3000 for a server on another PC on your network.
      </p>
      <div className="mt-2 flex gap-2">
        <input
          value={value}
          onChange={(e) => {
            setValue(e.target.value);
            setStatus("idle");
          }}
          placeholder="http://192.168.0.250:3000"
          className="flex-1 rounded-md border border-onyx-border bg-onyx-surface px-2 py-1 text-xs text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          onClick={() => void handleSave()}
          disabled={status === "testing"}
          className="rounded-md bg-onyx-surface-hover px-2 py-1 text-xs font-medium text-onyx-text disabled:opacity-50"
        >
          {status === "testing" ? "Testing…" : "Test & Save"}
        </button>
      </div>
      {message && (
        <p
          className={`mt-2 text-[11px] ${
            status === "error" ? "text-onyx-status-blocked" : "text-onyx-text-dim"
          }`}
        >
          {message}
        </p>
      )}
    </div>
  );
}
