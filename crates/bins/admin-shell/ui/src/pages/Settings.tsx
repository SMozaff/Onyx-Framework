import { useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { apiClient } from "@/api/client";
import { useCommand } from "@/hooks/useCommand";
import { useQuery } from "@/hooks/useQuery";
import { describeError } from "@/utils/errorHandler";
import { useAuthStore } from "@/stores/authStore";
import { getServerAddress, setServerAddress, isPlausibleServerAddress } from "@/utils/serverAddress";

/**
 * Policy administration — ported from `desktop-shell`'s `Settings.tsx`
 * to this app's thin-HTTP-client architecture. Covers the full
 * `policy-domain` command set: versioned rule authoring and
 * publication, evaluation, violation recording, retirement, and Legal
 * Hold apply/release.
 *
 * Ids are plain UUID strings, not desktop-shell's `Id16` byte-array
 * form — this app has no Tauri IPC layer, and `/api/query`'s
 * id-normalization fix (added alongside this page) makes every
 * aggregate id a real UUID string over the wire.
 *
 * `CreatePolicy`/`ApplyLegalHold` go through dedicated REST routes
 * (`POST /api/admin/policies`, `POST /api/admin/legal-holds`), not a
 * command envelope — both are `create()`-routed commands that
 * `/api/command` cannot express. Every other Policy/LegalHold action
 * goes through `/api/command` via `useCommand`.
 */
export default function Settings() {
  const { policyId } = useParams<{ policyId?: string }>();
  const navigate = useNavigate();

  const targetId = policyId ?? null;
  const { data, loading, error, refetch } = useQuery<Record<string, unknown>>(
    "policy.detail",
    targetId ? { id: targetId } : null,
  );
  const policy = data?.data as Record<string, unknown>[] | undefined;
  const policyRow = policy?.[0];

  return (
    <div className="max-w-3xl">
      <h1 className="text-xl font-semibold text-onyx-text">Policy &amp; Settings</h1>
      <p className="mt-1 text-sm text-onyx-text-dim">
        Organization policy: feature availability, limits, retention, and legal holds.
      </p>

      <ServerConnectionSettings />

      <IdLookup onLookup={(id) => navigate(`/settings/${id}`)} />
      <CreatePolicyForm onCreated={(id) => navigate(`/settings/${id}`)} />

      {targetId && (
        <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
          {loading && <p className="text-sm text-onyx-text-dim">Loading…</p>}
          {error && <p className="text-sm text-onyx-status-blocked">{error.message}</p>}
          {!loading && !error && !policyRow && (
            <p className="text-sm text-onyx-text-dim">No policy found for this id.</p>
          )}
          {policyRow && (
            <PolicyPanel targetId={targetId} policy={policyRow} onChanged={() => void refetch()} />
          )}
        </div>
      )}

      <LegalHoldPanel />
    </div>
  );
}

/**
 * Lets an admin point this app at a different backend server without
 * rebuilding — added because `admin-shell` previously had the server
 * address hardcoded at build time (`VITE_API_BASE` env var / defaulted
 * to `127.0.0.1:3000`), which meant every install on every PC could
 * only ever talk to a server running on that same machine. See
 * `utils/serverAddress.ts` for the storage layer and
 * `api/client.ts`'s request interceptor for how this takes effect
 * immediately, with no restart required.
 */
function ServerConnectionSettings() {
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
      setMessage("Enter a full address including http:// or https://, e.g. http://192.168.0.250:3000");
      return;
    }
    setStatus("testing");
    setMessage(null);
    const reachable = await testConnection(value);
    if (!reachable) {
      setStatus("error");
      setMessage(
        "Could not reach a server at this address. The address was NOT saved — " +
          "double-check the IP/port, that the server is running, and that this PC " +
          "can reach it on the network before saving.",
      );
      return;
    }
    setServerAddress(value);
    setStatus("ok");
    setMessage("Saved. This app will now use this address for all requests.");
  }

  return (
    <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <h2 className="text-sm font-semibold text-onyx-text">Server connection</h2>
      <p className="mt-1 text-xs text-onyx-text-dim">
        The address of the ONYX backend this app talks to. Change this if you're running
        the Admin app on a different computer than the server — e.g. a LAN address like{" "}
        <code className="rounded bg-onyx-bg px-1 py-0.5">http://192.168.0.250:3000</code>{" "}
        instead of <code className="rounded bg-onyx-bg px-1 py-0.5">http://127.0.0.1:3000</code>.
      </p>

      <div className="mt-3 flex gap-2">
        <input
          value={value}
          onChange={(e) => {
            setValue(e.target.value);
            setStatus("idle");
          }}
          placeholder="http://192.168.0.250:3000"
          className="flex-1 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          onClick={() => void handleSave()}
          disabled={status === "testing"}
          className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
        >
          {status === "testing" ? "Testing…" : "Test & Save"}
        </button>
      </div>

      {message && (
        <p
          className={`mt-2 text-xs ${
            status === "error" ? "text-onyx-status-blocked" : "text-onyx-text-dim"
          }`}
        >
          {message}
        </p>
      )}
    </div>
  );
}

function IdLookup({ onLookup }: { onLookup: (id: string) => void }) {
  const [raw, setRaw] = useState("");
  return (
    <div className="mt-4">
      <label className="block text-xs font-medium text-onyx-text-dim">
        Look up by policy id (UUID)
      </label>
      <div className="mt-1 flex gap-2">
        <input
          value={raw}
          onChange={(e) => setRaw(e.target.value)}
          placeholder="00000000-0000-0000-0000-000000000000"
          className="flex-1 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          onClick={() => raw && onLookup(raw)}
          className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover"
        >
          Open
        </button>
      </div>
    </div>
  );
}

function CreatePolicyForm({ onCreated }: { onCreated: (id: string) => void }) {
  const [name, setName] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function create() {
    if (name.trim().length === 0) return;
    setLoading(true);
    setError(null);
    try {
      const response = await apiClient.post<{ policy_id: string }>("/api/admin/policies", {
        name,
      });
      setName("");
      onCreated(response.data.policy_id);
    } catch (e) {
      setError(describeError(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <h2 className="text-sm font-medium text-onyx-text">Create policy</h2>
      <div className="mt-2 flex gap-2">
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Policy name, e.g. Default Org Governance"
          className="flex-1 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          onClick={() => void create()}
          disabled={loading || name.trim().length === 0}
          className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
        >
          {loading ? "Creating…" : "Create"}
        </button>
      </div>
      {error && <p className="mt-1 text-xs text-onyx-status-blocked">{error}</p>}
    </div>
  );
}

const RULE_TYPES = ["FeatureToggle", "Threshold", "Authorization", "Retention"] as const;
type RuleType = (typeof RULE_TYPES)[number];

interface DraftRule {
  rule_type: RuleType;
  key: string;
  value: string;
}

const SUGGESTED_KEYS = [
  "messaging.enabled",
  "file_sharing.enabled",
  "file.max_size_bytes",
  "message.retention_days",
];

function baseCommandParams(targetId: string, organizationId: string, expectedVersion: number) {
  return { targetId, targetType: "policy", organizationId, expectedVersion };
}

function PolicyPanel({
  targetId,
  policy,
  onChanged,
}: {
  targetId: string;
  policy: Record<string, unknown>;
  onChanged: () => void;
}) {
  const status = String(policy.status ?? "Unknown");
  const name = String(policy.name ?? "(unnamed)");
  const version = typeof policy.version === "number" ? policy.version : 0;
  const versions = Array.isArray(policy.versions)
    ? (policy.versions as { version_number?: number; status?: string; rules?: DraftRule[] }[])
    : [];

  return (
    <div>
      <div className="flex items-center justify-between">
        <h2 className="text-base font-medium text-onyx-text">{name}</h2>
        <span className="text-xs text-onyx-text-dim">{status}</span>
      </div>
      <VersionHistory versions={versions} />
      <DraftVersionForm targetId={targetId} version={version} onChanged={onChanged} />
      <PolicyActions targetId={targetId} version={version} onChanged={onChanged} />
    </div>
  );
}

function VersionHistory({
  versions,
}: {
  versions: { version_number?: number; status?: string; rules?: DraftRule[] }[];
}) {
  if (versions.length === 0) {
    return <p className="mt-3 text-xs text-onyx-text-dim">No versions yet.</p>;
  }
  return (
    <div className="mt-3">
      <h3 className="text-xs font-medium text-onyx-text-dim">Versions</h3>
      <ul className="mt-1 space-y-2">
        {versions.map((v) => (
          <li key={v.version_number} className="rounded-md border border-onyx-border p-2 text-xs">
            <div className="flex items-center justify-between">
              <span className="text-onyx-text">v{v.version_number}</span>
              <span className="text-onyx-text-dim">{v.status}</span>
            </div>
            {Array.isArray(v.rules) && v.rules.length > 0 && (
              <ul className="mt-1 space-y-0.5 text-onyx-text-dim">
                {v.rules.map((r, i) => (
                  <li key={i}>
                    {r.key} = {String(r.value)} ({r.rule_type})
                  </li>
                ))}
              </ul>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

function DraftVersionForm({
  targetId,
  version,
  onChanged,
}: {
  targetId: string;
  version: number;
  onChanged: () => void;
}) {
  const [rules, setRules] = useState<DraftRule[]>([
    { rule_type: "FeatureToggle", key: "", value: "" },
  ]);
  const { execute, loading, error } = useCommand();
  const organizationId = useAuthStore((s) => s.user?.organization_id) ?? "";

  function updateRule(index: number, patch: Partial<DraftRule>) {
    setRules((rs) => rs.map((r, i) => (i === index ? { ...r, ...patch } : r)));
  }

  function addRule() {
    setRules((rs) => [...rs, { rule_type: "FeatureToggle", key: "", value: "" }]);
  }

  async function submit() {
    const parsed = rules
      .filter((r) => r.key.trim().length > 0)
      .map((r) => {
        let value: unknown = r.value;
        if (r.value === "true") value = true;
        else if (r.value === "false") value = false;
        else if (r.value.trim() !== "" && !Number.isNaN(Number(r.value))) value = Number(r.value);
        return { rule_type: r.rule_type, key: r.key, value };
      });
    if (parsed.length === 0) return;

    await execute({
      commandType: "CreatePolicyVersion",
      ...baseCommandParams(targetId, organizationId, version),
      payload: { CreatePolicyVersion: { rules: parsed } },
    });
    setRules([{ rule_type: "FeatureToggle", key: "", value: "" }]);
    onChanged();
  }

  return (
    <div className="mt-4 rounded-md border border-onyx-border bg-onyx-bg p-3">
      <h3 className="text-xs font-medium text-onyx-text-dim">Draft a new version</h3>
      {rules.map((rule, i) => (
        <div key={i} className="mt-2 flex flex-wrap items-center gap-2">
          <select
            value={rule.rule_type}
            onChange={(e) => updateRule(i, { rule_type: e.target.value as RuleType })}
            className="rounded-md border border-onyx-border bg-onyx-surface px-2 py-1 text-xs text-onyx-text"
          >
            {RULE_TYPES.map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </select>
          <input
            list="suggested-keys"
            value={rule.key}
            onChange={(e) => updateRule(i, { key: e.target.value })}
            placeholder="key, e.g. messaging.enabled"
            className="flex-1 rounded-md border border-onyx-border bg-onyx-surface px-2 py-1 text-xs text-onyx-text"
          />
          <input
            value={rule.value}
            onChange={(e) => updateRule(i, { value: e.target.value })}
            placeholder="value"
            className="w-28 rounded-md border border-onyx-border bg-onyx-surface px-2 py-1 text-xs text-onyx-text"
          />
        </div>
      ))}
      <datalist id="suggested-keys">
        {SUGGESTED_KEYS.map((k) => (
          <option key={k} value={k} />
        ))}
      </datalist>
      <div className="mt-2 flex gap-2">
        <button
          type="button"
          onClick={addRule}
          className="rounded-md bg-onyx-surface px-2 py-1 text-xs text-onyx-text hover:bg-onyx-surface-hover"
        >
          + Add rule
        </button>
        <button
          type="button"
          onClick={() => void submit()}
          disabled={loading}
          className="rounded-md bg-onyx-accent px-2 py-1 text-xs font-medium text-white disabled:opacity-50"
        >
          {loading ? "Saving…" : "Save draft version"}
        </button>
      </div>
      {error && <p className="mt-1 text-xs text-onyx-status-blocked">{error.message}</p>}
    </div>
  );
}

function PolicyActions({
  targetId,
  version,
  onChanged,
}: {
  targetId: string;
  version: number;
  onChanged: () => void;
}) {
  const publishCmd = useCommand();
  const retireCmd = useCommand();
  const evaluateCmd = useCommand();
  const [evaluateKey, setEvaluateKey] = useState("");
  const [evaluateResult, setEvaluateResult] = useState<string | null>(null);
  const organizationId = useAuthStore((s) => s.user?.organization_id) ?? "";

  return (
    <div className="mt-4 flex flex-wrap items-center gap-2 border-t border-onyx-border pt-3">
      <button
        type="button"
        disabled={publishCmd.loading}
        onClick={() =>
          void publishCmd
            .execute({
              commandType: "PublishPolicyVersion",
              ...baseCommandParams(targetId, organizationId, version),
              payload: { PublishPolicyVersion: null },
            })
            .then(onChanged)
        }
        className="rounded-md bg-onyx-status-approved/15 px-3 py-1.5 text-xs text-onyx-status-approved hover:bg-onyx-status-approved/25 disabled:opacity-50"
      >
        Publish draft
      </button>
      <button
        type="button"
        disabled={retireCmd.loading}
        onClick={() =>
          void retireCmd
            .execute({
              commandType: "RetirePolicy",
              ...baseCommandParams(targetId, organizationId, version),
              payload: { RetirePolicy: null },
            })
            .then(onChanged)
        }
        className="rounded-md bg-onyx-status-blocked/15 px-3 py-1.5 text-xs text-onyx-status-blocked hover:bg-onyx-status-blocked/25 disabled:opacity-50"
      >
        Retire policy
      </button>
      <div className="flex items-center gap-2">
        <input
          value={evaluateKey}
          onChange={(e) => setEvaluateKey(e.target.value)}
          placeholder="rule key to evaluate"
          className="rounded-md border border-onyx-border bg-onyx-bg px-2 py-1 text-xs text-onyx-text"
        />
        <button
          type="button"
          disabled={evaluateCmd.loading || !evaluateKey}
          onClick={() =>
            void evaluateCmd
              .execute({
                commandType: "EvaluatePolicy",
                ...baseCommandParams(targetId, organizationId, version),
                payload: { EvaluatePolicy: { rule_key: evaluateKey } },
              })
              .then((result) => setEvaluateResult(JSON.stringify(result)))
          }
          className="rounded-md bg-onyx-surface px-2 py-1 text-xs text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
        >
          Evaluate
        </button>
      </div>
      {evaluateResult && <span className="text-xs text-onyx-text-dim">{evaluateResult}</span>}
      {(publishCmd.error ?? retireCmd.error ?? evaluateCmd.error) && (
        <p className="w-full text-xs text-onyx-status-blocked">
          {(publishCmd.error ?? retireCmd.error ?? evaluateCmd.error)?.message}
        </p>
      )}
    </div>
  );
}

function LegalHoldPanel() {
  const [holdIdRaw, setHoldIdRaw] = useState("");
  const targetId = holdIdRaw || null;
  const { data, refetch } = useQuery<Record<string, unknown>>(
    "legal_hold.detail",
    targetId ? { id: targetId } : null,
  );
  const holdRows = data?.data as Record<string, unknown>[] | undefined;
  const hold = holdRows?.[0];

  const [targetIdInput, setTargetIdInput] = useState("");
  const [targetTypeInput, setTargetTypeInput] = useState("file_asset");
  const [reasonInput, setReasonInput] = useState("");
  const [applyError, setApplyError] = useState<string | null>(null);
  const [applying, setApplying] = useState(false);
  const releaseCmd = useCommand();
  const organizationId = useAuthStore((s) => s.user?.organization_id) ?? "";

  async function apply() {
    setApplying(true);
    setApplyError(null);
    try {
      const response = await apiClient.post<{ legal_hold_id: string }>(
        "/api/admin/legal-holds",
        { target_id: targetIdInput, target_type: targetTypeInput, reason: reasonInput },
      );
      setHoldIdRaw(response.data.legal_hold_id);
      setTargetIdInput("");
      setReasonInput("");
    } catch (e) {
      setApplyError(describeError(e));
    } finally {
      setApplying(false);
    }
  }

  const status = hold ? String(hold.status ?? "Unknown") : null;

  return (
    <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <h2 className="text-sm font-medium text-onyx-text">Legal holds</h2>
      <div className="mt-2 grid grid-cols-2 gap-2">
        <input
          value={targetIdInput}
          onChange={(e) => setTargetIdInput(e.target.value)}
          placeholder="Target id (UUID)"
          className="rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <input
          value={targetTypeInput}
          onChange={(e) => setTargetTypeInput(e.target.value)}
          placeholder="Target type, e.g. file_asset"
          className="rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <input
          value={reasonInput}
          onChange={(e) => setReasonInput(e.target.value)}
          placeholder="Reason"
          className="col-span-2 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
      </div>
      {applyError && <p className="mt-1 text-xs text-onyx-status-blocked">{applyError}</p>}
      <button
        type="button"
        onClick={() => void apply()}
        disabled={applying || !targetIdInput || !reasonInput}
        className="mt-2 rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
      >
        {applying ? "Applying…" : "Apply legal hold"}
      </button>

      <div className="mt-4">
        <label className="block text-xs font-medium text-onyx-text-dim">View a hold by id</label>
        <input
          value={holdIdRaw}
          onChange={(e) => setHoldIdRaw(e.target.value)}
          placeholder="Legal hold id (UUID)"
          className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
      </div>

      {hold && (
        <div className="mt-3 rounded-md border border-onyx-border bg-onyx-bg p-3">
          <p className="text-sm text-onyx-text">Status: {status}</p>
          {status === "Applied" && (
            <button
              type="button"
              disabled={releaseCmd.loading}
              onClick={() =>
                void releaseCmd
                  .execute({
                    commandType: "ReleaseLegalHold",
                    targetId: holdIdRaw,
                    targetType: "legal_hold",
                    organizationId,
                    expectedVersion: typeof hold.version === "number" ? hold.version : 0,
                    payload: { ReleaseLegalHold: { reason: "Released by administrator" } },
                  })
                  .then(() => void refetch())
              }
              className="mt-2 rounded-md bg-onyx-status-blocked/15 px-3 py-1.5 text-xs text-onyx-status-blocked hover:bg-onyx-status-blocked/25 disabled:opacity-50"
            >
              Release
            </button>
          )}
          {releaseCmd.error && (
            <p className="mt-1 text-xs text-onyx-status-blocked">{releaseCmd.error.message}</p>
          )}
        </div>
      )}
    </div>
  );
}
