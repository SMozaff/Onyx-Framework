import { useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import type { Id16, LoadedAggregate } from "@/types/onyx";
import { newId16 } from "@/types/onyx";
import { useQuery } from "@/hooks/useQuery";
import { useCommand } from "@/hooks/useCommand";
import { useSession } from "@/hooks/useSession";
import { useI18n } from "@/i18n/I18nContext";
import StatusBadge from "@/components/StatusBadge";

export default function Missions() {
  const { missionId } = useParams<{ missionId?: string }>();
  const navigate = useNavigate();
  const session = useSession();
  const { t } = useI18n();

  const targetId: Id16 | null = missionId ? JSON.parse(missionId) : null;
  const { data, loading, error, refetch } = useQuery<LoadedAggregate>("GetMission", targetId);

  return (
    <div className="max-w-3xl">
      <h1 className="text-xl font-semibold text-onyx-text">{t("missions.title")}</h1>
      <IdLookup onLookup={(id) => navigate(`/missions/${JSON.stringify(id)}`)} />
      <CreateMissionForm session={session} onCreated={(id) => navigate(`/missions/${JSON.stringify(id)}`)} />

      {targetId && (
        <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
          {loading && <p className="text-sm text-onyx-text-dim">{t("common.loading")}</p>}
          {error && <p className="text-sm text-onyx-status-blocked">{error.message}</p>}
          {!loading && !error && data === null && <p className="text-sm text-onyx-text-dim">{t("missions.noMissionFoundForId")}</p>}
          {data && (
            <div>
              <div className="flex items-center justify-between">
                <h2 className="text-base font-medium text-onyx-text">{String(data.aggregate.name ?? t("missions.unnamed"))}</h2>
                <StatusBadge status={String(data.aggregate.status ?? "Unknown")} />
              </div>
              {typeof data.aggregate.description === "string" && data.aggregate.description && <p className="mt-2 text-sm text-onyx-text-dim">{data.aggregate.description}</p>}
              <dl className="mt-4 grid grid-cols-3 gap-2 text-xs text-onyx-text-dim">
                <div><dt>{t("common.version")}</dt><dd className="text-onyx-text">{data.version}</dd></div>
                <div><dt>{t("missions.lifecycleEpoch")}</dt><dd className="text-onyx-text">{data.lifecycle_epoch}</dd></div>
                <div><dt>{t("missions.authorityEpoch")}</dt><dd className="text-onyx-text">{data.authority_epoch}</dd></div>
              </dl>
              <button type="button" onClick={() => void refetch()} className="mt-4 rounded-md bg-onyx-surface-hover px-3 py-1.5 text-xs text-onyx-text">{t("common.refresh")}</button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function IdLookup({ onLookup }: { onLookup: (id: Id16) => void }) {
  const { t } = useI18n();
  const [raw, setRaw] = useState("");
  const [parseError, setParseError] = useState<string | null>(null);
  function submit() {
    try {
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed) || parsed.length !== 16) throw new Error("expected a JSON array of 16 numbers (the wire shape of an ObjectId)");
      setParseError(null);
      onLookup(parsed as Id16);
    } catch (e) { setParseError(e instanceof Error ? e.message : String(e)); }
  }
  return (
    <div className="mt-4">
      <label className="block text-xs font-medium text-onyx-text-dim">{t("missions.lookupByMissionId")}</label>
      <div className="mt-1 flex gap-2">
        <input value={raw} onChange={(e) => setRaw(e.target.value)} placeholder={t("common.id16Placeholder")} className="flex-1 rounded-md border border-onyx-border bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none" />
        <button type="button" onClick={submit} className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover">{t("common.view")}</button>
      </div>
      {parseError && <p className="mt-1 text-xs text-onyx-status-blocked">{parseError}</p>}
    </div>
  );
}

function CreateMissionForm({ session, onCreated }: { session: ReturnType<typeof useSession>; onCreated: (id: Id16) => void }) {
  const { t } = useI18n();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const { execute, loading, error } = useCommand();

  async function create() {
    if (name.trim().length === 0) return;
    const result = (await execute({
      commandType: "CreateMission",
      targetId: newId16(),
      targetType: "mission",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: 0,
      payload: { CreateMission: { name, description: description.trim().length > 0 ? description : null, owner_id: session.userId } },
    })) as { mission_id?: Id16 };

    if (result.mission_id) {
      setName("");
      setDescription("");
      onCreated(result.mission_id);
    }
  }

  return (
    <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <h2 className="text-sm font-medium text-onyx-text">{t("missions.createMission")}</h2>
      <div className="mt-2 space-y-2">
        <input value={name} onChange={(e) => setName(e.target.value)} placeholder={t("missions.namePlaceholder")} className="w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none" />
        <textarea value={description} onChange={(e) => setDescription(e.target.value)} placeholder={t("missions.descriptionPlaceholder")} rows={2} className="w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none" />
        {error && <p className="text-xs text-onyx-status-blocked">{error.message}</p>}
        <button type="button" onClick={() => void create()} disabled={loading || name.trim().length === 0} className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50">
          {loading ? t("common.creating") : t("common.create")}
        </button>
      </div>
    </div>
  );
}
