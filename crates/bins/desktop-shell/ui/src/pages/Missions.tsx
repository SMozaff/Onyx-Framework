import { useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import type { Id16, LoadedAggregate } from "@/types/onyx";
import { newId16 } from "@/types/onyx";
import { useQuery } from "@/hooks/useQuery";
import { useCommand } from "@/hooks/useCommand";
import { useSession } from "@/hooks/useSession";
import StatusBadge from "@/components/StatusBadge";

/**
 * Mission list + detail. "List" is honest about a real backend
 * limitation (see `Dashboard.tsx`'s doc comment): there is no
 * list/projection query, only `GetMission` by a known id. This page is
 * therefore id-driven — paste/select a known Mission id to view it, or
 * create a new one (whose id then becomes known). A real "list all
 * missions in my org" view needs a projection query this increment's
 * backend doesn't have; not invented here.
 */
export default function Missions() {
  const { missionId } = useParams<{ missionId?: string }>();
  const navigate = useNavigate();
  const session = useSession();

  const targetId: Id16 | null = missionId ? JSON.parse(missionId) : null;
  const { data, loading, error, refetch } = useQuery<LoadedAggregate>("GetMission", targetId);

  return (
    <div className="max-w-3xl">
      <h1 className="text-xl font-semibold text-onyx-text">Missions</h1>

      <IdLookup
        onLookup={(id) => navigate(`/missions/${JSON.stringify(id)}`)}
      />

      <CreateMissionForm
        session={session}
        onCreated={(id) => navigate(`/missions/${JSON.stringify(id)}`)}
      />

      {targetId && (
        <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
          {loading && <p className="text-sm text-onyx-text-dim">Loading…</p>}
          {error && <p className="text-sm text-onyx-status-blocked">{error.message}</p>}
          {!loading && !error && data === null && (
            <p className="text-sm text-onyx-text-dim">No mission found for this id.</p>
          )}
          {data && (
            <div>
              <div className="flex items-center justify-between">
                <h2 className="text-base font-medium text-onyx-text">
                  {String(data.aggregate.name ?? "(unnamed)")}
                </h2>
                <StatusBadge status={String(data.aggregate.status ?? "Unknown")} />
              </div>
              {typeof data.aggregate.description === "string" && data.aggregate.description && (
                <p className="mt-2 text-sm text-onyx-text-dim">{data.aggregate.description}</p>
              )}
              <dl className="mt-4 grid grid-cols-3 gap-2 text-xs text-onyx-text-dim">
                <div>
                  <dt>Version</dt>
                  <dd className="text-onyx-text">{data.version}</dd>
                </div>
                <div>
                  <dt>Lifecycle epoch</dt>
                  <dd className="text-onyx-text">{data.lifecycle_epoch}</dd>
                </div>
                <div>
                  <dt>Authority epoch</dt>
                  <dd className="text-onyx-text">{data.authority_epoch}</dd>
                </div>
              </dl>
              <button
                type="button"
                onClick={() => void refetch()}
                className="mt-4 rounded-md bg-onyx-surface-hover px-3 py-1.5 text-xs text-onyx-text"
              >
                Refresh
              </button>
            </div>
          )}
        </div>
      )}
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
        Look up by mission id (JSON array of 16 bytes)
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
          View
        </button>
      </div>
      {parseError && <p className="mt-1 text-xs text-onyx-status-blocked">{parseError}</p>}
    </div>
  );
}

function CreateMissionForm({
  session,
  onCreated,
}: {
  session: ReturnType<typeof useSession>;
  onCreated: (id: Id16) => void;
}) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const { execute, loading, error } = useCommand();

  async function create() {
    if (name.trim().length === 0) return;
    const result = (await execute({
      commandType: "CreateMission",
      targetId: newId16(), // ignored by CreationHandler (client-composition's MissionCreationHandler) but must still be a valid 16-byte ObjectId — Rust's [u8; 16] rejects an empty array.
      targetType: "mission",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: 0,
      payload: {
        CreateMission: {
          name,
          description: description.trim().length > 0 ? description : null,
          owner_id: session.userId,
        },
      },
    })) as { mission_id?: Id16 };

    if (result.mission_id) {
      setName("");
      setDescription("");
      onCreated(result.mission_id);
    }
  }

  return (
    <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <h2 className="text-sm font-medium text-onyx-text">Create mission</h2>
      <div className="mt-2 space-y-2">
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Mission name"
          className="w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <textarea
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="Description (optional)"
          rows={2}
          className="w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        {error && <p className="text-xs text-onyx-status-blocked">{error.message}</p>}
        <button
          type="button"
          onClick={() => void create()}
          disabled={loading || name.trim().length === 0}
          className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
        >
          {loading ? "Creating…" : "Create"}
        </button>
      </div>
    </div>
  );
}
