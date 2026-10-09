import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useSession } from "@/hooks/useSession";
import { useI18n } from "@/i18n/I18nContext";

const ConnectionStatus = {
  Idle: "idle",
  Testing: "testing",
  Reachable: "reachable",
  Unreachable: "unreachable",
} as const;

type ConnectionStatus = (typeof ConnectionStatus)[keyof typeof ConnectionStatus];

type BackendEnvironment = "local" | "cloud";
const ENVIRONMENT_KEY = "onyx_staff_backend_environment";
const LOCAL_ADDRESS_KEY = "onyx_staff_local_backend_address";
const CLOUD_ADDRESS_KEY = "onyx_staff_cloud_backend_address";
const LOCAL_DEFAULT = "http://127.0.0.1:3000";
const CLOUD_DEFAULT = "https://onyx-api-docker.onrender.com";

function readEnvironment(): BackendEnvironment {
  return localStorage.getItem(ENVIRONMENT_KEY) === "local" ? "local" : "cloud";
}
function readAddress(environment: BackendEnvironment, currentAddress: string): string {
  const key = environment === "local" ? LOCAL_ADDRESS_KEY : CLOUD_ADDRESS_KEY;
  const savedEnvironment = readEnvironment();
  const canReuseCurrentAddress = environment === savedEnvironment && currentAddress.trim().length > 0;
  return localStorage.getItem(key) || (canReuseCurrentAddress ? currentAddress : environment === "local" ? LOCAL_DEFAULT : CLOUD_DEFAULT);
}

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
  const { t } = useI18n();
  const navigate = useNavigate();
  const [environment, setEnvironment] = useState<BackendEnvironment>(() => readEnvironment());
  const [serverAddress, setServerAddress] = useState(() => readAddress(readEnvironment(), session.serverAddress));
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>(ConnectionStatus.Idle);
  const [message, setMessage] = useState<string | null>(null);
  const [endingSession, setEndingSession] = useState(false);

  const normalizedAddress = normalizeServerAddress(serverAddress);

  function chooseEnvironment(next: BackendEnvironment) {
    setEnvironment(next);
    setServerAddress(readAddress(next, session.serverAddress));
    setConnectionStatus(ConnectionStatus.Idle);
    setMessage(null);
  }
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
      <h1 className="text-xl font-semibold text-onyx-text">{t("settings.title")}</h1>
      <p className="mt-1 text-sm text-onyx-text-dim">
        {t("settings.desktopConnectionNote")}
      </p>

      <section className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-5">
        <h2 className="text-base font-semibold text-onyx-text">{t("settings.serverConnection")}</h2>
        <p className="mt-1 text-sm text-onyx-text-dim">
          You are signed in as <strong className="font-medium text-onyx-text">{session.username}</strong>.
          The server address and your session tokens are saved together, so changing the address
          requires a new sign-in rather than reusing credentials from another server.
        </p>

        <div className="mt-5 flex flex-wrap gap-2" role="group" aria-label={t("settings.backendEnvironment")}>
          <button type="button" aria-pressed={environment === "local"} onClick={() => chooseEnvironment("local")} className={`rounded-md border px-3 py-1.5 text-sm ${environment === "local" ? "border-onyx-accent bg-onyx-accent/10 text-onyx-text" : "border-onyx-border text-onyx-text-dim"}`}>{t("settings.localBackend")}</button>
          <button type="button" aria-pressed={environment === "cloud"} onClick={() => chooseEnvironment("cloud")} className={`rounded-md border px-3 py-1.5 text-sm ${environment === "cloud" ? "border-onyx-accent bg-onyx-accent/10 text-onyx-text" : "border-onyx-border text-onyx-text-dim"}`}>{t("settings.cloudBackend")}</button>
        </div>
        <label htmlFor="serverAddress" className="mt-3 block text-xs font-medium text-onyx-text-dim">
          {environment === "cloud" ? t("settings.cloudApiUrl") : t("settings.localApiUrl")}
        </label>
        <input
          id="serverAddress"
          value={serverAddress}
          onChange={(event) => {
            setServerAddress(event.target.value);
            setConnectionStatus(ConnectionStatus.Idle);
            setMessage(null);
          }}
          placeholder={environment === "cloud" ? CLOUD_DEFAULT : LOCAL_DEFAULT}
          className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <p className="mt-2 text-xs text-onyx-text-dim">
          {t("auth.serverAddressExample")}
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
            {connectionStatus === ConnectionStatus.Testing ? t("auth.testing") : t("settings.testConnection")}
          </button>
          {changed && (
            <button
              type="button"
              onClick={() => void connectToNewServer()}
              disabled={endingSession || connectionStatus === ConnectionStatus.Testing}
              className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
            >
              {endingSession ? t("auth.signingOut") : t("settings.useServerAndSignInAgain")}
            </button>
          )}
        </div>
      </section>

      <section className="mt-4 rounded-lg border border-onyx-border bg-onyx-surface p-5">
        <h2 className="text-base font-semibold text-onyx-text">{t("common.account")}</h2>
        <p className="mt-1 text-sm text-onyx-text-dim">
          Signing out clears the saved server-specific session tokens. It does not reset this
          device’s local SQLite data or replica identity.
        </p>
        <button
          type="button"
          onClick={() => navigate("/", { replace: true })}
          className="mt-4 rounded-md bg-onyx-surface-hover px-3 py-1.5 text-sm font-medium text-onyx-text"
        >
          {t("settings.returnToDashboard")}
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
