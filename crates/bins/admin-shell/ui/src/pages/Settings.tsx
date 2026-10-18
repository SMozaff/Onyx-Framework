import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { apiClient } from "@/api/client";
import { useCommand } from "@/hooks/useCommand";
import { useQuery } from "@/hooks/useQuery";
import { describeError } from "@/utils/errorHandler";
import { useAuthStore } from "@/stores/authStore";
import { useI18n } from "@/i18n/I18nContext";
import {
  detailPanelState,
  detailProjection,
  detailRequestFilters,
  isAmbiguousCreateError,
  legalHoldReleaseCommand,
  newCreateOperation,
  policyCommand,
  postIdempotentCreation,
  projectionConcurrency,
  type IdempotentCreateOperation,
  type ProjectionState,
} from "./settingsHelpers";

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
const POLICY_ID_PLACEHOLDER = "00000000-0000-0000-0000-000000000000";

export default function Settings() {
  const { t } = useI18n();
  const { policyId } = useParams<{ policyId?: string }>();
  const navigate = useNavigate();

  const targetId = policyId ?? null;
  const { data, loading, error, refetch } = useQuery<Record<string, unknown>>(
    "policy.detail",
    targetId ? detailRequestFilters(targetId) : null,
  );
  const policyRow = targetId ? detailProjection(data, targetId) : null;
  const policyDetailState = detailPanelState(targetId, loading, error, policyRow);

  return (
    <div className="max-w-3xl">
      <h1 className="text-xl font-semibold text-onyx-text">{t("common.policySettings")}</h1>
      <p className="mt-1 text-sm text-onyx-text-dim">
        {t("settings.organizationPolicyDescription")}
      </p>

      <IdLookup onLookup={(id) => navigate(`/settings/${id}`)} />
      <CreatePolicyForm onCreated={(id) => navigate(`/settings/${id}`)} />

      {targetId && (
        <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
          {policyDetailState === "loading" && <p className="text-sm text-onyx-text-dim">{t("common.loading")}</p>}
          {policyDetailState === "error" && <p className="text-sm text-onyx-status-blocked">{error?.message}</p>}
          {policyDetailState === "empty" && (
            <p className="text-sm text-onyx-text-dim">{t("common.noPolicyFound")}</p>
          )}
          {policyDetailState === "ready" && policyRow && (
            <PolicyPanel targetId={targetId} policy={policyRow} onChanged={() => void refetch()} />
          )}
        </div>
      )}

      <LegalHoldPanel />
      <MobileAccessPanel />
    </div>
  );
}

const USER_CLASSES: { value: string; label: string }[] = [
  { value: "top_level_manager", label: "Top-level Manager" },
  { value: "senior_manager", label: "Senior Manager" },
  { value: "team_leader", label: "Team Leader" },
  { value: "supervisor", label: "Supervisor" },
  { value: "staff", label: "Staff" },
];

/**
 * Class-based mobile access control. Per an explicit product decision,
 * mobile login is restrictive by default: a class with no grant here
 * cannot log in from the mobile app at all (Admin is unaffected either
 * way — it always bypasses this check server-side, same as every other
 * class-based gate in this codebase). Reads/writes
 * `GET`/`PUT /api/admin/mobile-access` directly, like `ServerConnectionSettings`
 * and `Profiles.tsx`'s own plain-table settings do, rather than going
 * through `useCommand`/`useQuery` — this isn't an event-sourced
 * aggregate.
 */
function MobileAccessPanel() {
  const { t } = useI18n();
  const [allowed, setAllowed] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [version, setVersion] = useState(0);
  const [enforcementStatus, setEnforcementStatus] = useState("unknown");
  const [savedAt, setSavedAt] = useState<number | null>(null);

  function applyServerState(data: {
    allowed_classes?: unknown;
    version?: unknown;
    enforcement_status?: unknown;
  }) {
    if (!Array.isArray(data.allowed_classes) ||
        !data.allowed_classes.every((value) => typeof value === "string") ||
        typeof data.version !== "number" || !Number.isSafeInteger(data.version) || data.version < 0 ||
        !["disabled_untrusted_client_type", "enabled_attested_client", "enabled_server_policy"].includes(String(data.enforcement_status))) {
      throw new Error("The server returned an invalid mobile-access configuration.");
    }
    setAllowed(new Set(data.allowed_classes as string[]));
    setVersion(data.version);
    setEnforcementStatus(String(data.enforcement_status));
  }

  useEffect(() => {
    void (async () => {
      try {
        const response = await apiClient.get("/api/admin/mobile-access");
        applyServerState(response.data);
      } catch (err) {
        setLoadFailed(true);
        setError(describeError(err));
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  async function save(next: Set<string>) {
    setSaving(true);
    setError(null);
    setSavedAt(null);
    try {
      const allowed_classes = USER_CLASSES.filter((c) => next.has(c.value)).map((c) => c.value);
      const response = await apiClient.put("/api/admin/mobile-access", {
        allowed_classes,
        expected_version: version,
      });
      applyServerState(response.data);
      setSavedAt(Date.now());
    } catch (err) {
      setError(describeError(err));
      // Restore the canonical server value after validation, storage, or
      // optimistic-concurrency failures. Never leave a failed save checked.
      try {
        const response = await apiClient.get("/api/admin/mobile-access");
        applyServerState(response.data);
      } catch {
        setLoadFailed(true);
      }
    } finally {
      setSaving(false);
    }
  }

  function toggle(value: string) {
    const next = new Set(allowed);
    if (next.has(value)) {
      next.delete(value);
    } else {
      next.add(value);
    }
    void save(next);
  }

  return (
    <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <h2 className="text-sm font-semibold text-onyx-text">{t("common.mobileAccess")}</h2>
      <p className="mt-1 text-sm text-onyx-text-dim">
        {t("settings.mobileAccessPolicyDescription")}
      </p>
      <p className="mt-2 rounded-md border border-amber-200 bg-amber-50 p-3 text-sm text-amber-900">
        {t("settings.mobileAccessPolicyScope")}
      </p>
      {enforcementStatus === "disabled_untrusted_client_type" && (
        <p role="alert" className="mt-3 rounded-md border border-onyx-border p-3 text-sm text-onyx-status-blocked">
          {t("settings.mobileAccessEnforcementDisabled")}
        </p>
      )}
      {loading && <p className="mt-3 text-sm text-onyx-text-dim">{t("common.loading")}</p>}
      {error && <p className="mt-3 text-sm text-onyx-status-blocked">{error}</p>}
      {!loading && (
        <div className="mt-3 flex flex-col gap-2">
          {USER_CLASSES.map((c) => (
            <label key={c.value} className="flex items-center gap-2 text-sm text-onyx-text">
              <input
                type="checkbox"
                checked={allowed.has(c.value)}
                disabled={saving || loading || loadFailed}
                onChange={() => toggle(c.value)}
              />
              {c.label}
            </label>
          ))}
        </div>
      )}
      {savedAt && !saving && <p className="mt-2 text-xs text-onyx-text-dim">{t("common.saved")}</p>}
    </div>
  );
}

function IdLookup({ onLookup }: { onLookup: (id: string) => void }) {
  const { t } = useI18n();
  const [raw, setRaw] = useState("");
  return (
    <div className="mt-4">
      <label className="block text-xs font-medium text-onyx-text-dim">
        {t("settings.lookupPolicyById")}
      </label>
      <div className="mt-1 flex gap-2">
        <input
          value={raw}
          onChange={(e) => setRaw(e.target.value)}
          placeholder={POLICY_ID_PLACEHOLDER}
          className="flex-1 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          onClick={() => raw && onLookup(raw)}
          className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover"
        >
          {t("common.open")}
        </button>
      </div>
    </div>
  );
}

function CreatePolicyForm({ onCreated }: { onCreated: (id: string) => void }) {
  const { t } = useI18n();
  const [name, setName] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [operation, setOperation] = useState<IdempotentCreateOperation<{ name: string }> | null>(null);
  const [retrySuggested, setRetrySuggested] = useState(false);

  async function create() {
    const trimmedName = name.trim();
    if (trimmedName.length === 0) return;
    // Editing the form starts a new logical submission. Until then, all
    // retries retain the original key so the Worker can replay its result.
    const pending = operation?.payload.name === trimmedName
      ? operation
      : newCreateOperation({ name: trimmedName });
    setLoading(true);
    setError(null);
    setRetrySuggested(false);
    try {
      const response = await postIdempotentCreation<{ name: string }, { policy_id: string }>(
        apiClient,
        "/api/admin/policies",
        pending,
      );
      if (typeof response.policy_id !== "string" || response.policy_id.length === 0) {
        throw new Error("The policy was created without a usable policy ID.");
      }
      setName("");
      setOperation(null);
      onCreated(response.policy_id);
    } catch (e) {
      setOperation(pending);
      const ambiguous = isAmbiguousCreateError(e);
      setRetrySuggested(ambiguous);
      setError(describeError(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <h2 className="text-sm font-medium text-onyx-text">{t("common.createPolicy")}</h2>
      <div className="mt-2 flex gap-2">
        <input
          value={name}
          onChange={(e) => {
            setName(e.target.value);
            setOperation(null);
            setRetrySuggested(false);
          }}
          placeholder={t("common.policyNameExample")}
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
      {retrySuggested && operation && (
        <button
          type="button"
          onClick={() => void create()}
          disabled={loading}
          className="mt-2 rounded-md bg-onyx-surface px-2 py-1 text-xs text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
        >
          {t("settings.retryCreationSafely")}
        </button>
      )}
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

function PolicyPanel({
  targetId,
  policy,
  onChanged,
}: {
  targetId: string;
  policy: Record<string, unknown>;
  onChanged: () => void;
}) {
  const { t } = useI18n();
  const status = String(policy.status ?? "Unknown");
  const name = String(policy.name ?? "(unnamed)");
  const concurrency = projectionConcurrency(policy);
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
      {concurrency ? (
        <>
          <DraftVersionForm targetId={targetId} policy={policy} onChanged={onChanged} />
          <PolicyActions targetId={targetId} policy={policy} onChanged={onChanged} />
        </>
      ) : (
        <p className="mt-3 text-xs text-onyx-status-blocked">
          {t("settings.policyMissingConcurrencyEpochs")}
        </p>
      )}
    </div>
  );
}

function VersionHistory({
  versions,
}: {
  versions: { version_number?: number; status?: string; rules?: DraftRule[] }[];
}) {
  const { t } = useI18n();
  if (versions.length === 0) {
    return <p className="mt-3 text-xs text-onyx-text-dim">{t("common.noVersionsYet")}</p>;
  }
  return (
    <div className="mt-3">
      <h3 className="text-xs font-medium text-onyx-text-dim">{t("files.versions")}</h3>
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
  policy,
  onChanged,
}: {
  targetId: string;
  policy: ProjectionState;
  onChanged: () => void;
}) {
  const { t } = useI18n();
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

    const command = policyCommand(
      "CreatePolicyVersion",
      targetId,
      organizationId,
      policy,
      { rules: parsed },
    );
    if (!command) return;
    await execute(command);
    setRules([{ rule_type: "FeatureToggle", key: "", value: "" }]);
    onChanged();
  }

  return (
    <div className="mt-4 rounded-md border border-onyx-border bg-onyx-bg p-3">
      <h3 className="text-xs font-medium text-onyx-text-dim">{t("common.draftNewVersion")}</h3>
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
            placeholder={t("common.keyExample")}
            className="flex-1 rounded-md border border-onyx-border bg-onyx-surface px-2 py-1 text-xs text-onyx-text"
          />
          <input
            value={rule.value}
            onChange={(e) => updateRule(i, { value: e.target.value })}
            placeholder={t("common.inputValue")}
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
          + {t("common.addRule")}
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
  policy,
  onChanged,
}: {
  targetId: string;
  policy: ProjectionState;
  onChanged: () => void;
}) {
  const { t } = useI18n();
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
        onClick={() => {
          const command = policyCommand("PublishPolicyVersion", targetId, organizationId, policy, {});
          if (command) void publishCmd.execute(command).then(onChanged);
        }}
        className="rounded-md bg-onyx-status-approved/15 px-3 py-1.5 text-xs text-onyx-status-approved hover:bg-onyx-status-approved/25 disabled:opacity-50"
      >
        {t("settings.publishDraft")}
      </button>
      <button
        type="button"
        disabled={retireCmd.loading}
        onClick={() => {
          const command = policyCommand("RetirePolicy", targetId, organizationId, policy, {});
          if (command) void retireCmd.execute(command).then(onChanged);
        }}
        className="rounded-md bg-onyx-status-blocked/15 px-3 py-1.5 text-xs text-onyx-status-blocked hover:bg-onyx-status-blocked/25 disabled:opacity-50"
      >
        {t("settings.retirePolicy")}
      </button>
      <div className="flex items-center gap-2">
        <input
          value={evaluateKey}
          onChange={(e) => setEvaluateKey(e.target.value)}
          placeholder={t("common.ruleKeyToEvaluate")}
          className="rounded-md border border-onyx-border bg-onyx-bg px-2 py-1 text-xs text-onyx-text"
        />
        <button
          type="button"
          disabled={evaluateCmd.loading || !evaluateKey}
          onClick={() => {
            const command = policyCommand(
              "EvaluatePolicy",
              targetId,
              organizationId,
              policy,
              { rule_key: evaluateKey },
            );
            if (command) {
              void evaluateCmd.execute(command).then((result) => {
                setEvaluateResult(JSON.stringify(result));
                onChanged();
              });
            }
          }}
          className="rounded-md bg-onyx-surface px-2 py-1 text-xs text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
        >
          {t("common.evaluate")}
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
  const { t } = useI18n();
  const [holdIdRaw, setHoldIdRaw] = useState("");
  const targetId = holdIdRaw || null;
  const { data, loading, error, refetch } = useQuery<Record<string, unknown>>(
    "legal_hold.detail",
    targetId ? detailRequestFilters(targetId) : null,
  );
  const hold = targetId ? detailProjection(data, targetId) : null;
  const holdDetailState = detailPanelState(targetId, loading, error, hold);

  const [targetIdInput, setTargetIdInput] = useState("");
  const [targetTypeInput, setTargetTypeInput] = useState("task");
  const [reasonInput, setReasonInput] = useState("");
  const [applyError, setApplyError] = useState<string | null>(null);
  const [applying, setApplying] = useState(false);
  const [operation, setOperation] = useState<IdempotentCreateOperation<{
    target_id: string;
    target_type: string;
    reason: string;
  }> | null>(null);
  const [retrySuggested, setRetrySuggested] = useState(false);
  const releaseCmd = useCommand();
  const organizationId = useAuthStore((s) => s.user?.organization_id) ?? "";

  async function apply() {
    const payload = {
      target_id: targetIdInput.trim(),
      target_type: targetTypeInput.trim(),
      reason: reasonInput.trim(),
    };
    if (!payload.target_id || !payload.target_type || !payload.reason) return;
    const pending = operation
      && operation.payload.target_id === payload.target_id
      && operation.payload.target_type === payload.target_type
      && operation.payload.reason === payload.reason
      ? operation
      : newCreateOperation(payload);
    setApplying(true);
    setApplyError(null);
    setRetrySuggested(false);
    try {
      const response = await postIdempotentCreation<typeof payload, { legal_hold_id: string }>(
        apiClient,
        "/api/admin/legal-holds",
        pending,
      );
      if (typeof response.legal_hold_id !== "string" || response.legal_hold_id.length === 0) {
        throw new Error("The legal hold was created without a usable legal-hold ID.");
      }
      setHoldIdRaw(response.legal_hold_id);
      setTargetIdInput("");
      setReasonInput("");
      setOperation(null);
    } catch (e) {
      setOperation(pending);
      setRetrySuggested(isAmbiguousCreateError(e));
      setApplyError(describeError(e));
    } finally {
      setApplying(false);
    }
  }

  const status = hold ? String(hold.status ?? "Unknown") : null;

  return (
    <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <h2 className="text-sm font-medium text-onyx-text">{t("common.legalHolds")}</h2>
      <div className="mt-2 grid grid-cols-2 gap-2">
        <input
          value={targetIdInput}
          onChange={(e) => {
            setTargetIdInput(e.target.value);
            setOperation(null);
            setRetrySuggested(false);
          }}
          placeholder={t("common.targetIdUuid")}
          className="rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <input
          value={targetTypeInput}
          onChange={(e) => {
            setTargetTypeInput(e.target.value);
            setOperation(null);
            setRetrySuggested(false);
          }}
          placeholder={t("common.targetTypeExample")}
          className="rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <input
          value={reasonInput}
          onChange={(e) => {
            setReasonInput(e.target.value);
            setOperation(null);
            setRetrySuggested(false);
          }}
          placeholder={t("common.reason")}
          className="col-span-2 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
      </div>
      {applyError && <p className="mt-1 text-xs text-onyx-status-blocked">{applyError}</p>}
      <button
        type="button"
        onClick={() => void apply()}
        disabled={applying || !targetIdInput.trim() || !targetTypeInput.trim() || !reasonInput.trim()}
        className="mt-2 rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
      >
        {applying ? "Applying…" : "Apply legal hold"}
      </button>
      {retrySuggested && operation && (
        <button
          type="button"
          onClick={() => void apply()}
          disabled={applying}
          className="ml-2 mt-2 rounded-md bg-onyx-surface px-2 py-1 text-xs text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
        >
          {t("settings.retryApplicationSafely")}
        </button>
      )}

      <div className="mt-4">
        <label className="block text-xs font-medium text-onyx-text-dim">{t("common.viewHoldById")}</label>
        <input
          value={holdIdRaw}
          onChange={(e) => setHoldIdRaw(e.target.value)}
          placeholder={t("common.userIdUuid")}
          className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
      </div>

      {targetId && (
        <div className="mt-3 rounded-md border border-onyx-border bg-onyx-bg p-3">
          {holdDetailState === "loading" && <p className="text-sm text-onyx-text-dim">{t("common.loading")}</p>}
          {holdDetailState === "error" && <p className="text-sm text-onyx-status-blocked">{error?.message}</p>}
          {holdDetailState === "empty" && (
            <p className="text-sm text-onyx-text-dim">{t("settings.noLegalHoldFound")}</p>
          )}
          {holdDetailState === "ready" && hold && (
            <>
              <p className="text-sm text-onyx-text">Status: {status}</p>
              {status === "Applied" && projectionConcurrency(hold) && (
                <button
                  type="button"
                  disabled={releaseCmd.loading}
                  onClick={() => {
                    const command = legalHoldReleaseCommand(
                      targetId,
                      organizationId,
                      hold,
                      "Released by administrator",
                    );
                    if (command) void releaseCmd.execute(command).then(() => void refetch());
                  }}
                  className="mt-2 rounded-md bg-onyx-status-blocked/15 px-3 py-1.5 text-xs text-onyx-status-blocked hover:bg-onyx-status-blocked/25 disabled:opacity-50"
                >
                  {t("settings.release")}
                </button>
              )}
              {status === "Applied" && !projectionConcurrency(hold) && (
                <p className="mt-1 text-xs text-onyx-status-blocked">
                  {t("settings.legalHoldMissingConcurrencyEpochs")}
                </p>
              )}
              {releaseCmd.error && (
                <p className="mt-1 text-xs text-onyx-status-blocked">{releaseCmd.error.message}</p>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}
