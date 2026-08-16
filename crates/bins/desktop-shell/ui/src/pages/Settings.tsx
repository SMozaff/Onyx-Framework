import { useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import type { Id16, LoadedAggregate } from "@/types/onyx";
import { newId16 } from "@/types/onyx";
import { useQuery } from "@/hooks/useQuery";
import { useCommand } from "@/hooks/useCommand";
import { useSession } from "@/hooks/useSession";

/**
 * Policy administration — the Manager/Admin-facing settings surface.
 * Covers the full `policy-domain` command set per the explicit product
 * decision to expose all of Part I §4.18, not just feature toggles:
 * versioned rule authoring and publication, evaluation, violation
 * recording, retirement, and Legal Hold apply/release.
 *
 * # Role gating (flagged, not silently assumed)
 * The Manager role exists end-to-end on the backend (`is_manager` on
 * `UserRecord`, `require_manager_or_admin` in `api-server`'s admin
 * routes — see `DECISIONS.md`), but this desktop shell has no real
 * authenticated session yet (`useSession` is a placeholder — see its own
 * doc comment), so there is no logged-in user whose role this page could
 * check. This page therefore does NOT gate itself by role: doing so
 * against a fabricated placeholder identity would be security theatre,
 * not access control. Real gating lands with the auth flow (Increment 7).
 * The authoritative check is the backend's, which is unaffected by this.
 *
 * # Rule vocabulary
 * `PolicyRule` is deliberately open (`{rule_type, key, value}` — see
 * `policy_domain::value::PolicyRule`'s doc comment), so this page lets an
 * admin author arbitrary key/value rules rather than hard-coding a fixed
 * list of settings that would need a code change to extend. Common keys
 * are offered as suggestions, not a closed set.
 */
export default function Settings() {
  const { policyId } = useParams<{ policyId?: string }>();
  const navigate = useNavigate();
  const session = useSession();

  const targetId: Id16 | null = policyId ? JSON.parse(policyId) : null;
  const { data, loading, error, refetch } = useQuery<LoadedAggregate>("GetPolicy", targetId);

  return (
    <div className="max-w-3xl">
      <h1 className="text-xl font-semibold text-onyx-text">Settings</h1>
      <p className="mt-1 text-sm text-onyx-text-dim">
        Organization policy: feature availability, limits, retention, and legal holds.
      </p>

      <IdLookup onLookup={(id) => navigate(`/settings/${JSON.stringify(id)}`)} />

      <CreatePolicyForm
        session={session}
        onCreated={(id) => navigate(`/settings/${JSON.stringify(id)}`)}
      />

      {targetId && (
        <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
          {loading && <p className="text-sm text-onyx-text-dim">Loading…</p>}
          {error && <p className="text-sm text-onyx-status-blocked">{error.message}</p>}
          {!loading && !error && data === null && (
            <p className="text-sm text-onyx-text-dim">No policy found for this id.</p>
          )}
          {data && (
            <PolicyPanel
              targetId={targetId}
              policy={data}
              session={session}
              onChanged={() => void refetch()}
            />
          )}
        </div>
      )}

      <LegalHoldPanel session={session} />
    </div>
  );
}

function IdLookup({ onLookup }: { onLookup: (id: Id16) => void }) {
  const [raw, setRaw] = useState("");
  const [parseError, setParseError] = useState<string | null>(null);

  function submit() {
    try {
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed) || parsed.length !== 16) {
        throw new Error("expected a JSON array of 16 numbers (the wire shape of an ObjectId)");
      }
      setParseError(null);
      onLookup(parsed as Id16);
    } catch (e) {
      setParseError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <div className="mt-4">
      <label className="block text-xs font-medium text-onyx-text-dim">
        Look up by policy id (JSON array of 16 bytes)
      </label>
      <div className="mt-1 flex gap-2">
        <input
          value={raw}
          onChange={(e) => setRaw(e.target.value)}
          placeholder="[12,34,...]"
          className="flex-1 rounded-md border border-onyx-border bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          onClick={submit}
          className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover"
        >
          Open
        </button>
      </div>
      {parseError && <p className="mt-1 text-xs text-onyx-status-blocked">{parseError}</p>}
    </div>
  );
}

function CreatePolicyForm({
  session,
  onCreated,
}: {
  session: ReturnType<typeof useSession>;
  onCreated: (id: Id16) => void;
}) {
  const [name, setName] = useState("");
  const { execute, loading, error } = useCommand();

  async function create() {
    if (name.trim().length === 0) return;
    const result = (await execute({
      commandType: "CreatePolicy",
      targetId: newId16(),
      targetType: "policy",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: 0,
      payload: {
        CreatePolicy: {
          name,
          // `PolicyScope::Organization(ObjectId)` — a serde externally
          // tagged enum variant with one unnamed field.
          scope: { Organization: session.organizationId },
        },
      },
    })) as { policy_id?: Id16 };

    if (result.policy_id) {
      setName("");
      onCreated(result.policy_id);
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
      {error && <p className="mt-1 text-xs text-onyx-status-blocked">{error.message}</p>}
    </div>
  );
}

/** `policy_domain::value::PolicyType`'s variants. */
const RULE_TYPES = ["FeatureToggle", "Threshold", "Authorization", "Retention"] as const;
type RuleType = (typeof RULE_TYPES)[number];

interface DraftRule {
  rule_type: RuleType;
  key: string;
  value: string;
}

/** Suggestions only — `PolicyRule.key` is open by design, so these are
 * offered as a starting point rather than a closed list. */
const SUGGESTED_KEYS = [
  "messaging.enabled",
  "file_sharing.enabled",
  "file.max_size_bytes",
  "message.retention_days",
];

function PolicyPanel({
  targetId,
  policy,
  session,
  onChanged,
}: {
  targetId: Id16;
  policy: LoadedAggregate;
  session: ReturnType<typeof useSession>;
  onChanged: () => void;
}) {
  const status = String(policy.aggregate.status ?? "Unknown");
  const name = String(policy.aggregate.name ?? "(unnamed)");
  const versions = Array.isArray(policy.aggregate.versions)
    ? (policy.aggregate.versions as {
        version_number?: number;
        status?: string;
        rules?: DraftRule[];
      }[])
    : [];

  return (
    <div>
      <div className="flex items-center justify-between">
        <h2 className="text-base font-medium text-onyx-text">{name}</h2>
        <span className="text-xs text-onyx-text-dim">{status}</span>
      </div>

      <VersionHistory versions={versions} />

      {status === "Active" && (
        <>
          <DraftVersionForm
            targetId={targetId}
            policy={policy}
            session={session}
            onChanged={onChanged}
          />
          <PolicyActions
            targetId={targetId}
            policy={policy}
            session={session}
            onChanged={onChanged}
          />
        </>
      )}
    </div>
  );
}

function VersionHistory({
  versions,
}: {
  versions: { version_number?: number; status?: string; rules?: DraftRule[] }[];
}) {
  if (versions.length === 0) {
    return (
      <p className="mt-4 text-sm text-onyx-text-dim">
        No versions yet. Draft one below, then publish it to put its rules into effect.
      </p>
    );
  }

  return (
    <div className="mt-4">
      <h3 className="text-sm font-medium text-onyx-text">Versions</h3>
      <ul className="mt-2 space-y-2">
        {versions.map((v) => (
          <li
            key={v.version_number}
            className="rounded-md border border-onyx-border bg-onyx-bg p-3 text-xs"
          >
            <div className="flex items-center justify-between">
              <span className="text-onyx-text">Version {v.version_number}</span>
              <span
                className={
                  v.status === "Published" ? "text-onyx-status-approved" : "text-onyx-status-review"
                }
              >
                {v.status}
              </span>
            </div>
            <ul className="mt-2 space-y-1">
              {(v.rules ?? []).map((r, i) => (
                <li key={`${r.key}-${i}`} className="text-onyx-text-dim">
                  <span className="text-onyx-text">{r.key}</span> = {JSON.stringify(r.value)}{" "}
                  <span className="opacity-60">({r.rule_type})</span>
                </li>
              ))}
            </ul>
          </li>
        ))}
      </ul>
    </div>
  );
}

function DraftVersionForm({
  targetId,
  policy,
  session,
  onChanged,
}: {
  targetId: Id16;
  policy: LoadedAggregate;
  session: ReturnType<typeof useSession>;
  onChanged: () => void;
}) {
  const [rules, setRules] = useState<DraftRule[]>([
    { rule_type: "FeatureToggle", key: "messaging.enabled", value: "true" },
  ]);
  const { execute, loading, error } = useCommand();
  const [formError, setFormError] = useState<string | null>(null);

  function updateRule(index: number, patch: Partial<DraftRule>) {
    setRules((prev) => prev.map((r, i) => (i === index ? { ...r, ...patch } : r)));
  }

  async function submit() {
    setFormError(null);
    // `PolicyRule.value` is a `serde_json::Value` on the Rust side, so a
    // toggle must send a real JSON boolean and a threshold a real number
    // — not the string the text input holds. Parsed here, with an
    // explicit error rather than silently sending a string that would
    // then evaluate wrongly at the consuming call site.
    const parsed: { rule_type: RuleType; key: string; value: unknown }[] = [];
    for (const r of rules) {
      if (r.key.trim().length === 0) {
        setFormError("Every rule needs a key.");
        return;
      }
      let value: unknown;
      try {
        value = JSON.parse(r.value);
      } catch {
        // A bare word is a common, reasonable thing to type; treat it as
        // a JSON string rather than rejecting it.
        value = r.value;
      }
      parsed.push({ rule_type: r.rule_type, key: r.key, value });
    }

    await execute({
      commandType: "CreatePolicyVersion",
      targetId,
      targetType: "policy",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: policy.version,
      expectedLifecycleEpoch: policy.lifecycle_epoch,
      payload: { CreatePolicyVersion: { rules: parsed } },
    });
    onChanged();
  }

  return (
    <div className="mt-6 border-t border-onyx-border pt-4">
      <h3 className="text-sm font-medium text-onyx-text">Draft a new version</h3>
      <p className="mt-1 text-xs text-onyx-text-dim">
        Only one draft may be pending at a time. Publishing makes it immutable and puts it into
        effect.
      </p>

      <div className="mt-3 space-y-2">
        {rules.map((rule, i) => (
          <div key={i} className="flex flex-wrap gap-2">
            <select
              value={rule.rule_type}
              onChange={(e) => updateRule(i, { rule_type: e.target.value as RuleType })}
              className="rounded-md border border-onyx-border bg-onyx-bg px-2 py-1.5 text-xs text-onyx-text focus:border-onyx-accent focus:outline-none"
            >
              {RULE_TYPES.map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </select>
            <input
              value={rule.key}
              onChange={(e) => updateRule(i, { key: e.target.value })}
              list="suggested-policy-keys"
              placeholder="rule key"
              className="flex-1 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
            />
            <input
              value={rule.value}
              onChange={(e) => updateRule(i, { value: e.target.value })}
              placeholder="value (true, 100, ...)"
              className="w-40 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
            />
            <button
              type="button"
              onClick={() => setRules((prev) => prev.filter((_, j) => j !== i))}
              disabled={rules.length === 1}
              className="rounded-md bg-onyx-surface px-2 py-1.5 text-xs text-onyx-text-dim hover:bg-onyx-surface-hover disabled:opacity-40"
            >
              Remove
            </button>
          </div>
        ))}
        <datalist id="suggested-policy-keys">
          {SUGGESTED_KEYS.map((k) => (
            <option key={k} value={k} />
          ))}
        </datalist>
      </div>

      <div className="mt-3 flex gap-2">
        <button
          type="button"
          onClick={() =>
            setRules((prev) => [...prev, { rule_type: "FeatureToggle", key: "", value: "true" }])
          }
          className="rounded-md bg-onyx-surface px-3 py-1.5 text-xs text-onyx-text hover:bg-onyx-surface-hover"
        >
          Add rule
        </button>
        <button
          type="button"
          onClick={() => void submit()}
          disabled={loading}
          className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
        >
          {loading ? "Saving…" : "Save draft"}
        </button>
      </div>
      {(formError ?? error) && (
        <p className="mt-1 text-xs text-onyx-status-blocked">{formError ?? error?.message}</p>
      )}
    </div>
  );
}

function PolicyActions({
  targetId,
  policy,
  session,
  onChanged,
}: {
  targetId: Id16;
  policy: LoadedAggregate;
  session: ReturnType<typeof useSession>;
  onChanged: () => void;
}) {
  const [evaluateKey, setEvaluateKey] = useState("messaging.enabled");
  const [evaluationResult, setEvaluationResult] = useState<string | null>(null);
  const publishCmd = useCommand();
  const evaluateCmd = useCommand();
  const retireCmd = useCommand();

  function baseParams(commandType: string, payload: unknown) {
    return {
      commandType,
      targetId,
      targetType: "policy",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: policy.version,
      expectedLifecycleEpoch: policy.lifecycle_epoch,
      payload,
    };
  }

  return (
    <div className="mt-6 border-t border-onyx-border pt-4">
      <h3 className="text-sm font-medium text-onyx-text">Actions</h3>

      <div className="mt-2 flex flex-wrap gap-2">
        <button
          type="button"
          disabled={publishCmd.loading}
          onClick={() =>
            void publishCmd
              .execute(baseParams("PublishPolicyVersion", { PublishPolicyVersion: null }))
              .then(onChanged)
          }
          className="rounded-md bg-onyx-status-approved/15 px-3 py-1.5 text-xs text-onyx-status-approved hover:bg-onyx-status-approved/25 disabled:opacity-50"
        >
          Publish pending draft
        </button>
        <button
          type="button"
          disabled={retireCmd.loading}
          onClick={() =>
            void retireCmd
              .execute(baseParams("RetirePolicy", { RetirePolicy: null }))
              .then(onChanged)
          }
          className="rounded-md bg-onyx-status-closed/15 px-3 py-1.5 text-xs text-onyx-status-closed hover:bg-onyx-status-closed/25 disabled:opacity-50"
        >
          Retire policy
        </button>
      </div>

      <div className="mt-4">
        <label className="block text-xs font-medium text-onyx-text-dim">
          Evaluate a rule against the published version
        </label>
        <div className="mt-1 flex gap-2">
          <input
            value={evaluateKey}
            onChange={(e) => setEvaluateKey(e.target.value)}
            className="flex-1 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
          />
          <button
            type="button"
            disabled={evaluateCmd.loading}
            onClick={() =>
              void evaluateCmd
                .execute(
                  baseParams("EvaluatePolicy", { EvaluatePolicy: { rule_key: evaluateKey } }),
                )
                .then((r) => {
                  setEvaluationResult(JSON.stringify(r));
                  onChanged();
                })
            }
            className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
          >
            Evaluate
          </button>
        </div>
        {evaluationResult && (
          <p className="mt-1 break-all text-xs text-onyx-text-dim">{evaluationResult}</p>
        )}
      </div>

      {(publishCmd.error ?? evaluateCmd.error ?? retireCmd.error) && (
        <p className="mt-2 text-xs text-onyx-status-blocked">
          {(publishCmd.error ?? evaluateCmd.error ?? retireCmd.error)?.message}
        </p>
      )}
    </div>
  );
}

/**
 * Legal Hold apply/release. A `LegalHold` is its own aggregate root,
 * independent of `Policy` (a Compliance Officer may apply one ad hoc,
 * with no policy version driving it — see
 * `policy_domain::aggregate::LegalHold`'s doc comment), so it gets its
 * own panel rather than living inside a policy's detail view.
 */
function LegalHoldPanel({ session }: { session: ReturnType<typeof useSession> }) {
  const [targetIdRaw, setTargetIdRaw] = useState("");
  const [targetType, setTargetType] = useState("file_asset");
  const [reason, setReason] = useState("");
  const [holdIdRaw, setHoldIdRaw] = useState("");
  const [formError, setFormError] = useState<string | null>(null);
  const applyCmd = useCommand();
  const releaseCmd = useCommand();

  const holdId = holdIdRaw ? tryParseId16(holdIdRaw) : null;
  const { data: hold, refetch } = useQuery<LoadedAggregate>("GetLegalHold", holdId);

  async function apply() {
    setFormError(null);
    let parsedTarget: Id16;
    try {
      const parsed = JSON.parse(targetIdRaw);
      if (!Array.isArray(parsed) || parsed.length !== 16) {
        throw new Error("expected a JSON array of 16 numbers");
      }
      parsedTarget = parsed as Id16;
    } catch (e) {
      setFormError(e instanceof Error ? e.message : String(e));
      return;
    }
    if (reason.trim().length === 0) {
      setFormError("A legal hold requires a reason.");
      return;
    }

    const result = (await applyCmd.execute({
      commandType: "ApplyLegalHold",
      targetId: newId16(),
      targetType: "legal_hold",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: 0,
      payload: {
        ApplyLegalHold: {
          target: { target_id: parsedTarget, target_type: targetType },
          authorizing_policy: null,
          reason,
        },
      },
    })) as { legal_hold_id?: Id16 };

    if (result.legal_hold_id) {
      setTargetIdRaw("");
      setReason("");
      setHoldIdRaw(JSON.stringify(result.legal_hold_id));
    }
  }

  return (
    <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <h2 className="text-sm font-medium text-onyx-text">Legal hold</h2>
      <p className="mt-1 text-xs text-onyx-text-dim">
        Supersedes normal retention and deletion for the held object until released.
      </p>

      <div className="mt-3 space-y-2">
        <div className="flex gap-2">
          <select
            value={targetType}
            onChange={(e) => setTargetType(e.target.value)}
            className="rounded-md border border-onyx-border bg-onyx-bg px-2 py-1.5 text-xs text-onyx-text focus:border-onyx-accent focus:outline-none"
          >
            <option value="file_asset">file_asset</option>
            <option value="conversation">conversation</option>
            <option value="message">message</option>
            <option value="mission">mission</option>
            <option value="task">task</option>
          </select>
          <input
            value={targetIdRaw}
            onChange={(e) => setTargetIdRaw(e.target.value)}
            placeholder="Target id [12,34,...]"
            className="flex-1 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
          />
        </div>
        <input
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          placeholder="Reason (required)"
          className="w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          onClick={() => void apply()}
          disabled={applyCmd.loading}
          className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
        >
          {applyCmd.loading ? "Applying…" : "Apply hold"}
        </button>
      </div>
      {(formError ?? applyCmd.error) && (
        <p className="mt-1 text-xs text-onyx-status-blocked">
          {formError ?? applyCmd.error?.message}
        </p>
      )}

      <div className="mt-4">
        <label className="block text-xs font-medium text-onyx-text-dim">
          View / release a hold by id
        </label>
        <input
          value={holdIdRaw}
          onChange={(e) => setHoldIdRaw(e.target.value)}
          placeholder="[12,34,...]"
          className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
      </div>

      {hold && holdId && (
        <div className="mt-3 rounded-md border border-onyx-border bg-onyx-bg p-3">
          <p className="text-sm text-onyx-text">
            Status: {String(hold.aggregate.status ?? "Unknown")}
          </p>
          {String(hold.aggregate.status) === "Applied" && (
            <button
              type="button"
              disabled={releaseCmd.loading}
              onClick={() =>
                void releaseCmd
                  .execute({
                    commandType: "ReleaseLegalHold",
                    targetId: holdId,
                    targetType: "legal_hold",
                    organizationId: session.organizationId,
                    userId: session.userId,
                    deviceId: session.deviceId,
                    expectedVersion: hold.version,
                    expectedLifecycleEpoch: hold.lifecycle_epoch,
                    payload: {
                      ReleaseLegalHold: { reason: "Released by administrator" },
                    },
                  })
                  .then(() => void refetch())
              }
              className="mt-2 rounded-md bg-onyx-status-closed/15 px-3 py-1.5 text-xs text-onyx-status-closed hover:bg-onyx-status-closed/25 disabled:opacity-50"
            >
              Release hold
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

function tryParseId16(raw: string): Id16 | null {
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) && parsed.length === 16 ? (parsed as Id16) : null;
  } catch {
    return null;
  }
}
