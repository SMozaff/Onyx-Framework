import { useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import type { Id16, LoadedAggregate } from "@/types/onyx";
import { newId16 } from "@/types/onyx";
import { useQuery } from "@/hooks/useQuery";
import { useCommand } from "@/hooks/useCommand";
import { useSession } from "@/hooks/useSession";
import StatusBadge from "@/components/StatusBadge";

/**
 * Task list + detail. Same "no list/projection query exists yet" caveat
 * as `Missions.tsx` applies — see that file's doc comment.
 */
export default function Tasks() {
  const { taskId } = useParams<{ taskId?: string }>();
  const navigate = useNavigate();
  const session = useSession();

  const targetId: Id16 | null = taskId ? JSON.parse(taskId) : null;
  const { data, loading, error, refetch } = useQuery<LoadedAggregate>("GetTask", targetId);
  const { execute: executeMarkReady, loading: markingReady } = useCommand();

  async function markReady() {
    if (!targetId || !data) return;
    await executeMarkReady({
      commandType: "MarkReady",
      targetId,
      targetType: "task",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: data.version,
      payload: { MarkReady: { reason: "Marked ready from desktop shell" } },
    });
    await refetch();
  }

  return (
    <div className="max-w-3xl">
      <h1 className="text-xl font-semibold text-onyx-text">Tasks</h1>

      <IdLookup onLookup={(id) => navigate(`/tasks/${JSON.stringify(id)}`)} />
      <CreateTaskForm session={session} onCreated={(id) => navigate(`/tasks/${JSON.stringify(id)}`)} />

      {targetId && (
        <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
          {loading && <p className="text-sm text-onyx-text-dim">Loading…</p>}
          {error && <p className="text-sm text-onyx-status-blocked">{error.message}</p>}
          {!loading && !error && data === null && (
            <p className="text-sm text-onyx-text-dim">No task found for this id.</p>
          )}
          {data && (
            <div>
              <div className="flex items-center justify-between">
                <h2 className="text-base font-medium text-onyx-text">
                  {String(data.aggregate.title ?? "(untitled)")}
                </h2>
                <StatusBadge status={String(data.aggregate.status ?? "Unknown")} />
              </div>
              {typeof data.aggregate.description === "string" && data.aggregate.description && (
                <p className="mt-2 text-sm text-onyx-text-dim">{data.aggregate.description}</p>
              )}

              <div className="mt-4 flex items-center gap-3">
                {data.aggregate.status === "Draft" && (
                  <button
                    type="button"
                    onClick={() => void markReady()}
                    disabled={markingReady}
                    className="rounded-md bg-onyx-accent px-3 py-1.5 text-xs font-medium text-white disabled:opacity-50"
                  >
                    {markingReady ? "Marking ready…" : "Mark ready"}
                  </button>
                )}
                <button
                  type="button"
                  onClick={() => void refetch()}
                  className="rounded-md bg-onyx-surface-hover px-3 py-1.5 text-xs text-onyx-text"
                >
                  Refresh
                </button>
              </div>

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
        Look up by task id (JSON array of 16 bytes)
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

function CreateTaskForm({
  session,
  onCreated,
}: {
  session: ReturnType<typeof useSession>;
  onCreated: (id: Id16) => void;
}) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [missionIdRaw, setMissionIdRaw] = useState("");
  const { execute, loading, error } = useCommand();

  async function create() {
    if (title.trim().length === 0) return;
    let missionId: Id16;
    try {
      missionId = missionIdRaw.trim().length > 0 ? JSON.parse(missionIdRaw) : newId16();
    } catch {
      return;
    }

    const result = (await execute({
      commandType: "CreateTask",
      targetId: newId16(), // ignored by CreationHandler; must be a valid 16-byte ObjectId (see Missions.tsx's identical note).
      targetType: "task",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: 0,
      payload: {
        CreateTask: {
          mission_id: missionId,
          title,
          description: description.trim().length > 0 ? description : null,
          owner_id: session.userId,
        },
      },
    })) as { task_id?: Id16 };

    if (result.task_id) {
      setTitle("");
      setDescription("");
      onCreated(result.task_id);
    }
  }

  return (
    <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <h2 className="text-sm font-medium text-onyx-text">Create task</h2>
      <div className="mt-2 space-y-2">
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Task title"
          className="w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <textarea
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="Description (optional)"
          rows={2}
          className="w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <input
          value={missionIdRaw}
          onChange={(e) => setMissionIdRaw(e.target.value)}
          placeholder="Mission id (JSON array of 16 bytes; leave blank for a random placeholder)"
          className="w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        {error && <p className="text-xs text-onyx-status-blocked">{error.message}</p>}
        <button
          type="button"
          onClick={() => void create()}
          disabled={loading || title.trim().length === 0}
          className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
        >
          {loading ? "Creating…" : "Create"}
        </button>
      </div>
    </div>
  );
}
