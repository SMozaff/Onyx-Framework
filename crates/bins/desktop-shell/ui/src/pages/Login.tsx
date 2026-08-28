import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNavigate } from "react-router-dom";
import { isShellError, type ShellError } from "@/types/onyx";
import { userFacingMessage } from "@/utils/userFacingError";
import type { SessionWire } from "@/hooks/useSession";
import onyxLogo from "@/assets/onyx-logo.png";

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
        credentials: {
          serverAddress: normalizedAddress,
          username,
          password,
        },
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
        setError(userFacingMessage(caught));
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
    <div className="onyx-auth-shell flex flex-col lg:flex-row">
      <section className="onyx-auth-aside" aria-labelledby="staff-signin-context">
        <div className="relative z-10 flex items-center gap-3">
          <img src={onyxLogo} alt="ONYX" className="h-11 w-11 shrink-0 object-contain" />
          <div>
            <p className="text-[0.72rem] font-extrabold tracking-[0.24em] text-white">ONYX</p>
            <p className="mt-0.5 text-[0.62rem] text-sky-100/70">Staff operations</p>
          </div>
        </div>
        <div className="onyx-auth-copy">
          <p className="text-[0.72rem] font-extrabold tracking-[0.19em] text-sky-100/90">SECURE DESKTOP ACCESS</p>
          <h2 id="staff-signin-context" className="mt-4 max-w-md text-4xl font-light leading-[1.03] tracking-[-0.045em] text-white sm:text-5xl">
            Operational clarity, from every authorized desktop.
          </h2>
          <p className="mt-5 max-w-lg text-sm leading-6 text-sky-50/85">
            Review mission work, resolve approvals, and act with an explicit local sync state.
          </p>
        </div>
        <p className="relative z-10 text-[0.68rem] text-sky-100/75">Native desktop replica · Server-authoritative commands · Protected session</p>
      </section>

      <section className="flex flex-1 items-center justify-center px-5 py-10 sm:px-10 lg:px-16">
        <form onSubmit={submit} className="onyx-auth-card p-6 sm:p-7">
          <p className="text-[0.66rem] font-extrabold tracking-[0.16em] text-onyx-accent">STAFF OPERATOR</p>
          <h1 className="mt-3 text-3xl font-medium tracking-[-0.04em] text-onyx-text">Sign in to ONYX</h1>
          <p className="mt-2 text-sm leading-5 text-onyx-text-dim">
            Use your organization credentials. Your session remains in secure native storage on this device.
          </p>

          <div className="mt-6">
            <label htmlFor="username" className="block text-xs font-bold text-onyx-text">
              Username
            </label>
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
            <label htmlFor="password" className="block text-xs font-bold text-onyx-text">
              Password
            </label>
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

          <button
            type="submit"
            disabled={loading}
            className="mt-5 w-full rounded-lg bg-onyx-accent px-3 py-2.5 text-sm font-bold text-white shadow-sm transition-colors hover:bg-[#174d7b] disabled:cursor-not-allowed disabled:opacity-50"
          >
            {loading ? "Signing in…" : "Sign in"}
          </button>

          <div className="mt-5 rounded-lg border border-sky-100 bg-sky-50/75 p-3">
            <p className="text-xs font-bold text-onyx-text">Session security</p>
            <p className="mt-1 text-[0.68rem] leading-4 text-onyx-text-dim">Tokens remain in protected native storage. Reauthentication is required when the server changes.</p>
          </div>

          <button
            type="button"
            onClick={() => setShowServerSettings((open) => !open)}
            className="mt-4 w-full text-center text-xs font-semibold text-onyx-accent underline decoration-dotted underline-offset-4 hover:text-[#174d7b]"
            aria-expanded={showServerSettings}
          >
            {showServerSettings ? "Hide server address" : "Server address / connection settings"}
          </button>

          {showServerSettings && (
            <div className="mt-3 rounded-lg border border-onyx-border bg-slate-50 p-3">
              <label htmlFor="serverAddress" className="block text-xs font-bold text-onyx-text">
                Server address
              </label>
              <p className="mt-1 text-[0.68rem] leading-4 text-onyx-text-dim">
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
                  className="min-w-0 flex-1 rounded-md border border-onyx-border bg-white px-2.5 py-2 text-xs text-onyx-text focus:border-onyx-accent focus:outline-none"
                />
                <button
                  type="button"
                  onClick={() => void testConnection()}
                  disabled={connectionStatus === "testing"}
                  className="shrink-0 rounded-md border border-onyx-border bg-white px-3 py-2 text-xs font-bold text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
                >
                  {connectionStatus === "testing" ? "Testing…" : "Test"}
                </button>
              </div>
              {connectionMessage && (
                <p
                  className={`mt-2 text-[0.68rem] leading-4 ${
                    connectionStatus === "unreachable" ? "text-onyx-status-blocked" : "text-onyx-status-approved"
                  }`}
                >
                  {connectionMessage}
                </p>
              )}
            </div>
          )}
        </form>
      </section>
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
