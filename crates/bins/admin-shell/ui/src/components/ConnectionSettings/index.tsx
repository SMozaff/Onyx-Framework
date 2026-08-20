import { useState } from "react";
import { getServerAddress, isPlausibleServerAddress, setServerAddress } from "@/utils/serverAddress";

const LOCAL_ADDRESS = "http://127.0.0.1:3000";
type ConnectionMode = "local" | "network";
type ConnectionStatus = "idle" | "testing" | "ok" | "error";

function defaultMode(address: string): ConnectionMode {
  return address.includes("127.0.0.1") || address.includes("localhost") ? "local" : "network";
}

export default function ConnectionSettings() {
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
      setMessage("Enter a full address including http:// or https://.");
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
      setMessage("Connection verified and saved. You can now sign in.");
    } catch {
      setStatus("error");
      setMessage("Could not reach a server at this address. It was not saved.");
    }
  }

  return (
    <section className="mt-4 rounded-md border border-onyx-border bg-onyx-bg p-3" aria-labelledby="connection-settings-title">
      <h2 id="connection-settings-title" className="text-xs font-semibold text-onyx-text">Connection setup</h2>
      <p className="mt-1 text-[11px] text-onyx-text-dim">Choose where ONYX Admin should connect. The address is saved only after a successful health check.</p>
      <div className="mt-3 grid grid-cols-2 gap-2" role="group" aria-label="Server location">
        <button type="button" className={`rounded-md border px-2 py-2 text-left text-xs ${mode === "local" ? "border-onyx-accent bg-onyx-surface text-onyx-text" : "border-onyx-border text-onyx-text-dim hover:bg-onyx-surface-hover"}`} onClick={() => selectMode("local")}>This computer</button>
        <button type="button" className={`rounded-md border px-2 py-2 text-left text-xs ${mode === "network" ? "border-onyx-accent bg-onyx-surface text-onyx-text" : "border-onyx-border text-onyx-text-dim hover:bg-onyx-surface-hover"}`} onClick={() => selectMode("network")}>Another computer</button>
      </div>
      <label htmlFor="admin-server-address" className="mt-3 block text-xs font-medium text-onyx-text-dim">Server address</label>
      {mode === "network" && <p className="mt-1 text-[11px] text-onyx-text-dim">For example: http://192.168.0.250:3000</p>}
      <input id="admin-server-address" value={value} onChange={(event) => { setValue(event.target.value); setStatus("idle"); setMessage(null); }} placeholder={mode === "local" ? LOCAL_ADDRESS : "http://192.168.0.250:3000"} className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-surface px-2 py-1.5 text-xs text-onyx-text focus:border-onyx-accent focus:outline-none" />
      <button type="button" onClick={() => void handleTestAndSave()} disabled={status === "testing"} className="mt-2 w-full rounded-md bg-onyx-surface-hover px-2 py-1.5 text-xs font-medium text-onyx-text hover:bg-onyx-border disabled:opacity-50">{status === "testing" ? "Testing connection…" : "Test connection and save"}</button>
      {message && <p className={`mt-2 text-[11px] ${status === "error" ? "text-onyx-status-blocked" : "text-onyx-text-dim"}`} role="status">{message}</p>}
    </section>
  );
}
