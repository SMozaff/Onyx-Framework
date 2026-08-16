import { useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import type { Id16, LoadedAggregate, ShellError } from "@/types/onyx";
import { isShellError } from "@/types/onyx";
import { useQuery } from "@/hooks/useQuery";
import { useCommand } from "@/hooks/useCommand";
import { useSession } from "@/hooks/useSession";

/**
 * File sharing: upload a local file, view a `FileAsset`'s metadata and
 * versions, manage per-user access, and download previously-uploaded
 * content back to disk.
 *
 * Upload/download go through the dedicated `upload_file`/`download_file`
 * Tauri commands rather than `execute_command`, because a real upload is
 * a multi-step domain sequence that also needs the actual file bytes —
 * see `client_composition::file_upload`'s module doc comment. Everything
 * else on this page (access grants, quarantine, archive) is a plain
 * `FileAssetCommand` through the normal command registry.
 *
 * Same "no list/projection query exists yet" caveat as the other pages:
 * this is id-driven, not a browsable file listing.
 */
export default function Files() {
  const { fileAssetId } = useParams<{ fileAssetId?: string }>();
  const navigate = useNavigate();
  const session = useSession();

  const targetId: Id16 | null = fileAssetId ? JSON.parse(fileAssetId) : null;
  const { data, loading, error, refetch } = useQuery<LoadedAggregate>("GetFileAsset", targetId);

  return (
    <div className="max-w-3xl">
      <h1 className="text-xl font-semibold text-onyx-text">Files</h1>

      <IdLookup onLookup={(id) => navigate(`/files/${JSON.stringify(id)}`)} />

      <UploadPanel session={session} onUploaded={(id) => navigate(`/files/${JSON.stringify(id)}`)} />

      {targetId && (
        <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
          {loading && <p className="text-sm text-onyx-text-dim">Loading…</p>}
          {error && <p className="text-sm text-onyx-status-blocked">{error.message}</p>}
          {!loading && !error && data === null && (
            <p className="text-sm text-onyx-text-dim">No file found for this id.</p>
          )}
          {data && (
            <FileAssetPanel
              targetId={targetId}
              asset={data}
              session={session}
              onChanged={() => void refetch()}
            />
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
        Look up by file id (JSON array of 16 bytes)
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

/** 100 MB — mirrors `file_domain::value::MAX_FILE_SIZE_BYTES`, which the
 * backend enforces independently. Shown here only so the user gets an
 * immediate, local message rather than a round trip for an obviously
 * oversized file; the backend remains the authority. */
const MAX_FILE_SIZE_BYTES = 100 * 1024 * 1024;

function UploadPanel({
  session,
  onUploaded,
}: {
  session: ReturnType<typeof useSession>;
  onUploaded: (id: Id16) => void;
}) {
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastHash, setLastHash] = useState<string | null>(null);

  async function upload() {
    if (path.trim().length === 0) return;
    setBusy(true);
    setError(null);
    try {
      const outcome = (await invoke("upload_file", {
        path,
        organizationId: session.organizationId,
        userId: session.userId,
        deviceId: session.deviceId,
      })) as {
        file_asset_id: Id16;
        content_hash: string;
        size_bytes: number;
      };
      setPath("");
      setLastHash(outcome.content_hash);
      onUploaded(outcome.file_asset_id);
    } catch (e) {
      const shellError: ShellError | null = isShellError(e) ? e : null;
      setError(shellError ? shellError.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <h2 className="text-sm font-medium text-onyx-text">Upload a file</h2>
      <p className="mt-1 text-xs text-onyx-text-dim">
        Enter the full path of a file on this machine. Maximum{" "}
        {MAX_FILE_SIZE_BYTES / (1024 * 1024)} MB.
      </p>
      <div className="mt-2 flex gap-2">
        <input
          value={path}
          onChange={(e) => setPath(e.target.value)}
          placeholder="/home/user/documents/report.pdf"
          className="flex-1 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          onClick={() => void upload()}
          disabled={busy || path.trim().length === 0}
          className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
        >
          {busy ? "Uploading…" : "Upload"}
        </button>
      </div>
      {error && <p className="mt-1 text-xs text-onyx-status-blocked">{error}</p>}
      {lastHash && (
        <p className="mt-2 break-all text-xs text-onyx-text-dim">
          Uploaded. Content hash: <span className="text-onyx-text">{lastHash}</span>
        </p>
      )}
    </div>
  );
}

function FileAssetPanel({
  targetId,
  asset,
  session,
  onChanged,
}: {
  targetId: Id16;
  asset: LoadedAggregate;
  session: ReturnType<typeof useSession>;
  onChanged: () => void;
}) {
  const fileName = String(asset.aggregate.file_name ?? "(unnamed)");
  const mimeType = String(asset.aggregate.mime_type ?? "unknown");
  const status = String(asset.aggregate.status ?? "Unknown");
  const versions = Array.isArray(asset.aggregate.versions)
    ? (asset.aggregate.versions as { content_hash?: string; size_bytes?: number }[])
    : [];

  return (
    <div>
      <div className="flex items-center justify-between">
        <h2 className="text-base font-medium text-onyx-text">{fileName}</h2>
        <span className="text-xs text-onyx-text-dim">{status}</span>
      </div>
      <p className="mt-1 text-xs text-onyx-text-dim">{mimeType}</p>

      <VersionList versions={versions} />

      <AccessControls targetId={targetId} asset={asset} session={session} onChanged={onChanged} />
    </div>
  );
}

function VersionList({
  versions,
}: {
  versions: { content_hash?: string; size_bytes?: number }[];
}) {
  const [destination, setDestination] = useState("");
  const [busyHash, setBusyHash] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);

  async function download(contentHash: string) {
    if (destination.trim().length === 0) {
      setError("Enter a destination path first.");
      return;
    }
    setBusyHash(contentHash);
    setError(null);
    setSaved(null);
    try {
      const bytesWritten = (await invoke("download_file", {
        contentHash,
        destinationPath: destination,
      })) as number;
      setSaved(`${bytesWritten} bytes written to ${destination}`);
    } catch (e) {
      setError(isShellError(e) ? e.message : String(e));
    } finally {
      setBusyHash(null);
    }
  }

  if (versions.length === 0) {
    return (
      <p className="mt-4 text-sm text-onyx-text-dim">
        No versions recorded yet. A file uploaded through this app records its first version
        automatically.
      </p>
    );
  }

  return (
    <div className="mt-4">
      <h3 className="text-sm font-medium text-onyx-text">Versions</h3>
      <div className="mt-2">
        <label className="block text-xs font-medium text-onyx-text-dim">
          Download destination path
        </label>
        <input
          value={destination}
          onChange={(e) => setDestination(e.target.value)}
          placeholder="/home/user/downloads/report.pdf"
          className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
      </div>
      <ul className="mt-3 space-y-2">
        {versions.map((v, i) => {
          const hash =
            typeof v.content_hash === "string"
              ? v.content_hash
              : // `ContentHash` is a newtype over String; serde emits it
                // transparently as a bare string, but tolerate an object
                // shape rather than crash if that ever changes.
                String((v.content_hash as unknown as { 0?: string } | undefined)?.[0] ?? "");
          return (
            <li
              key={`${hash}-${i}`}
              className="rounded-md border border-onyx-border bg-onyx-bg p-2 text-xs"
            >
              <p className="break-all text-onyx-text-dim">
                v{i + 1} · {v.size_bytes ?? 0} bytes
              </p>
              <p className="mt-1 break-all text-onyx-text-dim">{hash}</p>
              <button
                type="button"
                onClick={() => void download(hash)}
                disabled={busyHash !== null || hash.length === 0}
                className="mt-2 rounded-md bg-onyx-surface-hover px-3 py-1 text-xs text-onyx-text disabled:opacity-50"
              >
                {busyHash === hash ? "Downloading…" : "Download"}
              </button>
            </li>
          );
        })}
      </ul>
      {error && <p className="mt-2 text-xs text-onyx-status-blocked">{error}</p>}
      {saved && <p className="mt-2 text-xs text-onyx-status-approved">{saved}</p>}
    </div>
  );
}

function AccessControls({
  targetId,
  asset,
  session,
  onChanged,
}: {
  targetId: Id16;
  asset: LoadedAggregate;
  session: ReturnType<typeof useSession>;
  onChanged: () => void;
}) {
  const [userIdRaw, setUserIdRaw] = useState("");
  const [formError, setFormError] = useState<string | null>(null);
  const grantCmd = useCommand();
  const revokeCmd = useCommand();
  const quarantineCmd = useCommand();
  const archiveCmd = useCommand();

  async function runWithUserId(hook: ReturnType<typeof useCommand>, commandType: string) {
    setFormError(null);
    let userId: Id16;
    try {
      const parsed = JSON.parse(userIdRaw);
      if (!Array.isArray(parsed) || parsed.length !== 16) {
        throw new Error("expected a JSON array of 16 numbers");
      }
      userId = parsed as Id16;
    } catch (e) {
      setFormError(e instanceof Error ? e.message : String(e));
      return;
    }
    await hook.execute({
      commandType,
      targetId,
      targetType: "file_asset",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: asset.version,
      expectedLifecycleEpoch: asset.lifecycle_epoch,
      payload: { [commandType]: { user_id: userId } },
    });
    setUserIdRaw("");
    onChanged();
  }

  async function runSimple(
    hook: ReturnType<typeof useCommand>,
    commandType: string,
    payload: unknown,
  ) {
    await hook.execute({
      commandType,
      targetId,
      targetType: "file_asset",
      organizationId: session.organizationId,
      userId: session.userId,
      deviceId: session.deviceId,
      expectedVersion: asset.version,
      expectedLifecycleEpoch: asset.lifecycle_epoch,
      payload,
    });
    onChanged();
  }

  return (
    <div className="mt-6 border-t border-onyx-border pt-4">
      <h3 className="text-sm font-medium text-onyx-text">Access</h3>
      <div className="mt-2 flex gap-2">
        <input
          value={userIdRaw}
          onChange={(e) => setUserIdRaw(e.target.value)}
          placeholder="User id [12,34,...]"
          className="flex-1 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          disabled={grantCmd.loading}
          onClick={() => void runWithUserId(grantCmd, "GrantFileAccess")}
          className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
        >
          Grant
        </button>
        <button
          type="button"
          disabled={revokeCmd.loading}
          onClick={() => void runWithUserId(revokeCmd, "RevokeFileAccess")}
          className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
        >
          Revoke
        </button>
      </div>
      {(formError ?? grantCmd.error ?? revokeCmd.error) && (
        <p className="mt-1 text-xs text-onyx-status-blocked">
          {formError ?? (grantCmd.error ?? revokeCmd.error)?.message}
        </p>
      )}

      <div className="mt-4 flex flex-wrap gap-2">
        <button
          type="button"
          disabled={quarantineCmd.loading}
          onClick={() =>
            void runSimple(quarantineCmd, "QuarantineFile", {
              QuarantineFile: { reason: "Flagged for review" },
            })
          }
          className="rounded-md bg-onyx-status-review/15 px-3 py-1.5 text-xs text-onyx-status-review hover:bg-onyx-status-review/25 disabled:opacity-50"
        >
          Quarantine
        </button>
        <button
          type="button"
          disabled={archiveCmd.loading}
          onClick={() =>
            void runSimple(archiveCmd, "ArchiveFile", { ArchiveFile: { reason: null } })
          }
          className="rounded-md bg-onyx-status-closed/15 px-3 py-1.5 text-xs text-onyx-status-closed hover:bg-onyx-status-closed/25 disabled:opacity-50"
        >
          Archive
        </button>
      </div>
      {(quarantineCmd.error ?? archiveCmd.error) && (
        <p className="mt-1 text-xs text-onyx-status-blocked">
          {(quarantineCmd.error ?? archiveCmd.error)?.message}
        </p>
      )}
    </div>
  );
}
