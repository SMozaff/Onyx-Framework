import { useState } from "react";
import { useI18n } from "@/i18n/I18nContext";
import {
  getServerAddress,
  isPlausibleServerAddress,
  isSecureEnoughForProduction,
  setServerAddress,
} from "@/utils/serverAddress";

const LOCAL_ADDRESS = "http://127.0.0.1:3000";
const NETWORK_ADDRESS_PLACEHOLDER = "http://192.168.0.250:3000";
type ConnectionMode = "local" | "network";
type ConnectionStatus = "idle" | "testing" | "ok" | "error";

function defaultMode(address: string): ConnectionMode {
  return address.includes("127.0.0.1") || address.includes("localhost") ? "local" : "network";
}

export default function ConnectionSettings() {
  const { t } = useI18n();
  const initialAddress = getServerAddress();
  const [mode, setMode] = useState<ConnectionMode>(() => defaultMode(initialAddress));
  const [value, setValue] = useState(initialAddress);
  const [status, setStatus] = useState<ConnectionStatus>("idle");
  const [message, setMessage] = useState<string | null>(null);

  function selectMode(nextMode: ConnectionMode) {
    setMode(nextMode);
    if (nextMode === "local") setValue(LOCAL_ADDRESS);
    setStatus("idle");
    setMessage(null);
  }

  async function handleTestAndSave() {
    const normalized = value.trim().replace(/\/+$/, "");
    if (!isPlausibleServerAddress(normalized)) {
      setStatus("error");
      setMessage(t("settings.fullAddressRequired"));
      return;
    }
    if (!isSecureEnoughForProduction(normalized)) {
      setStatus("error");
      setMessage(t("settings.connectionSecurityRequirement"));
      return;
    }
    setStatus("testing");
    setMessage(null);
    try {
      const response = await fetch(`${normalized}/health`, { signal: AbortSignal.timeout(5_000) });
      if (!response.ok) throw new Error("unreachable");
      setServerAddress(normalized);
      setValue(normalized);
      setStatus("ok");
      setMessage(t("settings.connectionVerifiedAndSaved"));
    } catch {
      setStatus("error");
      setMessage(t("settings.connectionFailed"));
    }
  }

  return (
    <section className="mt-4 rounded-md border border-onyx-border bg-onyx-bg p-3" aria-labelledby="connection-settings-title">
      <h2 id="connection-settings-title" className="text-xs font-semibold text-onyx-text">{t("common.connectionSetup")}</h2>
      <p className="mt-1 text-[11px] text-onyx-text-dim">{t("common.adminConnectionHelp")}</p>
      <div className="mt-3 grid grid-cols-2 gap-2" role="group" aria-label={t("common.serverLocation")}>
        <button type="button" className={`rounded-md border px-2 py-2 text-left text-xs ${mode === "local" ? "border-onyx-accent bg-onyx-surface text-onyx-text" : "border-onyx-border text-onyx-text-dim hover:bg-onyx-surface-hover"}`} onClick={() => selectMode("local")}>{t("common.thisComputer")}</button>
        <button type="button" className={`rounded-md border px-2 py-2 text-left text-xs ${mode === "network" ? "border-onyx-accent bg-onyx-surface text-onyx-text" : "border-onyx-border text-onyx-text-dim hover:bg-onyx-surface-hover"}`} onClick={() => selectMode("network")}>{t("common.anotherComputer")}</button>
      </div>
      <label htmlFor="admin-server-address" className="mt-3 block text-xs font-medium text-onyx-text-dim">{t("auth.serverAddress")}</label>
      {mode === "network" && <p className="mt-1 text-[11px] text-onyx-text-dim">{t("common.exampleServerAddress")}</p>}
      <input id="admin-server-address" value={value} onChange={(event) => { setValue(event.target.value); setStatus("idle"); setMessage(null); }} placeholder={mode === "local" ? LOCAL_ADDRESS : NETWORK_ADDRESS_PLACEHOLDER} className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-surface px-2 py-1.5 text-xs text-onyx-text focus:border-onyx-accent focus:outline-none" />
      <button type="button" onClick={() => void handleTestAndSave()} disabled={status === "testing"} className="mt-2 w-full rounded-md bg-onyx-surface-hover px-2 py-1.5 text-xs font-medium text-onyx-text hover:bg-onyx-border disabled:opacity-50">{status === "testing" ? t("auth.testing") : t("settings.testConnectionAndSave")}</button>
      {message && <p className={`mt-2 text-[11px] ${status === "error" ? "text-onyx-status-blocked" : "text-onyx-text-dim"}`} role="status">{message}</p>}
    </section>
  );
}
