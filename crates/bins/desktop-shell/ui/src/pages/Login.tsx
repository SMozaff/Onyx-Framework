import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNavigate } from "react-router-dom";
import { isShellError, type ShellError } from "@/types/onyx";
import type { SessionWire } from "@/hooks/useSession";

const DEFAULT_SERVER_ADDRESS = "http://127.0.0.1:3000";

type ConnectionStatus = "idle" | "testing" | "reachable" | "unreachable";

/**
 * Staff desktop sign-in. Unlike the thin Admin client, this shell stores the
 * chosen server address inside its native, token-bearing session; a successful
 * login is therefore the only operation that persists a new address. The
 * collapsed connection section remains available before login so an install
 * pointed at a different PC never becomes unrecoverable behind an auth gate.
 */
export default function Login({
  initialServerAddress,
  onAuthenticated,
}: {
  initialServerAddress: string;
  onAuthenticated: (session: SessionWire) => void;
}) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [serverAddress, setServerAddress] = useState(initialServerAddress || DEFAULT_SERVER_ADDRESS);
  const [showServerSettings, setShowServerSettings] = useState(false);
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>("idle");
  const [connectionMessage, setConnectionMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const navigate = useNavigate();

  useEffect(() => {
    setServerAddress(initialServerAddress || DEFAULT_SERVER_ADDRESS);
  }, [initialServerAddress]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setError(null);

    const normalizedAddress = normalizeServerAddress(serverAddress);
    if (!isPlausibleServerAddress(normalizedAddress)) {
      setError("Enter a full server address including http:// or https://.");
      setShowServerSettings(true);
      return;
    }

    setLoading(true);
    try {
      const session = await invoke<SessionWire>("login", {
        serverAddress: normalizedAddress,
        username,
        password,
      });
      onAuthenticated(session);
      navigate("/", { replace: true });
    } catch (caught: unknown) {
      const shellError = isShellError(caught) ? caught : null;
      if (shellError?.kind === "auth" && !isCredentialRejection(shellError)) {
        setError(
          `Could not reach the server at ${normalizedAddress}. Check the address below, or confirm the server is running and reachable.`,
        );
        setShowServerSettings(true);
      } else if (shellError?.kind === "auth") {
        setError("Invalid username or password.");
      } else {
        setError(`Sign-in failed: ${String(caught)}`);
      }
    } finally {
      setLoading(false);
    }
  }

  async function testConnection() {
    const normalizedAddress = normalizeServerAddress(serverAddress);
    if (!isPlausibleServerAddress(normalizedAddress)) {
      setConnectionStatus("unreachable");
      setConnectionMessage("Enter a full address including http:// or https://.");
      return;
    }

    setConnectionStatus("testing");
    setConnectionMessage(null);
    try {
      const response = await fetch(`${normalizedAddress}/health`, {
        signal: AbortSignal.timeout(5_000),
      });
      if (!response.ok) {
        throw new Error(`server returned HTTP ${response.status}`);
      }
      setConnectionStatus("reachable");
      setConnectionMessage("Server reachable. This address will be saved when you sign in.");
    } catch {
      setConnectionStatus("unreachable");
      setConnectionMessage("Could not reach a server at this address. It was not saved.");
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-onyx-bg px-4">
      <form
        onSubmit={submit}
        className="w-full max-w-sm rounded-lg border border-onyx-border bg-onyx-surface p-6"
      >
        <h1 className="text-lg font-semibold text-onyx-text">ONYX</h1>
        <p className="mt-1 text-sm text-onyx-text-dim">
          Sign in to the staff desktop application.
        </p>

        <div className="mt-4">
          <label htmlFor="username" className="block text-xs font-medium text-onyx-text-dim">
            Username
          </label>
          <input
            id="username"
            value={username}
            onChange={(event) => setUsername(event.target.value)}
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
            onChange={(event) => setPassword(event.target.value)}
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
          onClick={() => setShowServerSettings((open) => !open)}
          className="mt-3 w-full text-center text-xs text-onyx-text-dim underline decoration-dotted hover:text-onyx-text"
        >
          {showServerSettings ? "Hide server address" : "Server address / connection settings"}
        </button>

        {showServerSettings && (
          <div className="mt-3 rounded-md border border-onyx-border bg-onyx-bg p-3">
            <label htmlFor="serverAddress" className="block text-xs font-medium text-onyx-text-dim">
              Server address
            </label>
            <p className="mt-1 text-[11px] text-onyx-text-dim">
              For example, use http://192.168.0.250:3000 for a server on another PC on your network.
            </p>
            <div className="mt-2 flex gap-2">
              <input
                id="serverAddress"
                value={serverAddress}
                onChange={(event) => {
                  setServerAddress(event.target.value);
                  setConnectionStatus("idle");
                  setConnectionMessage(null);
                }}
                placeholder={DEFAULT_SERVER_ADDRESS}
                required
                className="min-w-0 flex-1 rounded-md border border-onyx-border bg-onyx-surface px-2 py-1 text-xs text-onyx-text focus:border-onyx-accent focus:outline-none"
              />
              <button
                type="button"
                onClick={() => void testConnection()}
                disabled={connectionStatus === "testing"}
                className="shrink-0 rounded-md bg-onyx-surface-hover px-2 py-1 text-xs font-medium text-onyx-text disabled:opacity-50"
              >
                {connectionStatus === "testing" ? "Testing…" : "Test"}
              </button>
            </div>
            {connectionMessage && (
              <p
                className={`mt-2 text-[11px] ${
                  connectionStatus === "unreachable" ? "text-onyx-status-blocked" : "text-onyx-text-dim"
                }`}
              >
                {connectionMessage}
              </p>
            )}
          </div>
        )}
      </form>
    </div>
  );
}

function normalizeServerAddress(value: string): string {
  return value.trim().replace(/\/+$/, "");
}

function isPlausibleServerAddress(value: string): boolean {
  try {
    const url = new URL(value);
    return (url.protocol === "http:" || url.protocol === "https:") && url.host.length > 0;
  } catch {
    return false;
  }
}

function isCredentialRejection(error: ShellError): boolean {
  return error.message === "Invalid username or password";
}
