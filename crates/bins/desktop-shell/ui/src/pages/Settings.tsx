import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useSession } from "@/hooks/useSession";

const ConnectionStatus = {
  Idle: "idle",
  Testing: "testing",
  Reachable: "reachable",
  Unreachable: "unreachable",
} as const;

type ConnectionStatus = (typeof ConnectionStatus)[keyof typeof ConnectionStatus];

/**
 * Connection settings for the native Staff client. The server address belongs
 * to the persisted native session alongside server-specific tokens, unlike the
 * Admin client’s independent browser-local preference. Consequently an address
 * change never swaps endpoints under an existing token: after a successful
 * health check, the app clears the current session and returns to login, where
 * a fresh authentication persists the new address and matching tokens.
 */
export default function Settings({
  onRequireReauthentication,
}: {
  onRequireReauthentication: (nextLoginAddress: string) => Promise<void>;
}) {
  const session = useSession();
  const navigate = useNavigate();
  const [serverAddress, setServerAddress] = useState(session.serverAddress);
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>(ConnectionStatus.Idle);
  const [message, setMessage] = useState<string | null>(null);
  const [endingSession, setEndingSession] = useState(false);

  const normalizedAddress = normalizeServerAddress(serverAddress);
  const changed = normalizedAddress !== session.serverAddress;

  async function testConnection(): Promise<boolean> {
    if (!isPlausibleServerAddress(normalizedAddress)) {
      setConnectionStatus(ConnectionStatus.Unreachable);
      setMessage("Enter a full address including http:// or https://.");
      return false;
    }

    setConnectionStatus(ConnectionStatus.Testing);
    setMessage(null);
    try {
      const response = await fetch(`${normalizedAddress}/health`, {
        signal: AbortSignal.timeout(5_000),
      });
      if (!response.ok) {
        throw new Error(`server returned HTTP ${response.status}`);
      }
      setConnectionStatus(ConnectionStatus.Reachable);
      setMessage("Server reachable.");
      return true;
    } catch {
      setConnectionStatus(ConnectionStatus.Unreachable);
      setMessage("Could not reach a server at this address. Your current session is unchanged.");
      return false;
    }
  }

  async function connectToNewServer() {
    const reachable = await testConnection();
    if (!reachable) return;

    setEndingSession(true);
    try {
      await onRequireReauthentication(normalizedAddress);
    } finally {
      setEndingSession(false);
    }
  }

  return (
    <div className="max-w-2xl">
      <h1 className="text-xl font-semibold text-onyx-text">Settings</h1>
      <p className="mt-1 text-sm text-onyx-text-dim">
        Review the connection used by this staff desktop application.
      </p>

      <section className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-5">
        <h2 className="text-base font-semibold text-onyx-text">Server connection</h2>
        <p className="mt-1 text-sm text-onyx-text-dim">
          You are signed in as <strong className="font-medium text-onyx-text">{session.username}</strong>.
          The server address and your session tokens are saved together, so changing the address
          requires a new sign-in rather than reusing credentials from another server.
        </p>

        <label htmlFor="serverAddress" className="mt-5 block text-xs font-medium text-onyx-text-dim">
          Server address
        </label>
        <input
          id="serverAddress"
          value={serverAddress}
          onChange={(event) => {
            setServerAddress(event.target.value);
            setConnectionStatus(ConnectionStatus.Idle);
            setMessage(null);
          }}
          className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <p className="mt-2 text-xs text-onyx-text-dim">
          Example: http://192.168.0.250:3000 for a server on another PC on your network.
        </p>

        {message && (
          <p
            className={`mt-3 text-xs ${
              connectionStatus === ConnectionStatus.Unreachable
                ? "text-onyx-status-blocked"
                : "text-onyx-text-dim"
            }`}
          >
            {message}
          </p>
        )}

        <div className="mt-4 flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => void testConnection()}
            disabled={connectionStatus === ConnectionStatus.Testing || endingSession}
            className="rounded-md bg-onyx-surface-hover px-3 py-1.5 text-sm font-medium text-onyx-text disabled:opacity-50"
          >
            {connectionStatus === ConnectionStatus.Testing ? "Testing…" : "Test connection"}
          </button>
          {changed && (
            <button
              type="button"
              onClick={() => void connectToNewServer()}
              disabled={endingSession || connectionStatus === ConnectionStatus.Testing}
              className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
            >
              {endingSession ? "Signing out…" : "Use this server and sign in again"}
            </button>
          )}
        </div>
      </section>

      <section className="mt-4 rounded-lg border border-onyx-border bg-onyx-surface p-5">
        <h2 className="text-base font-semibold text-onyx-text">Account</h2>
        <p className="mt-1 text-sm text-onyx-text-dim">
          Signing out clears the saved server-specific session tokens. It does not reset this
          device’s local SQLite data or replica identity.
        </p>
        <button
          type="button"
          onClick={() => navigate("/", { replace: true })}
          className="mt-4 rounded-md bg-onyx-surface-hover px-3 py-1.5 text-sm font-medium text-onyx-text"
        >
          Return to dashboard
        </button>
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
