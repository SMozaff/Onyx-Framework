import { useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import type { Id16, LoadedAggregate, ShellError } from "@/types/onyx";
import { isShellError } from "@/types/onyx";
import { useQuery } from "@/hooks/useQuery";
import { useCommand } from "@/hooks/useCommand";
import { useSession } from "@/hooks/useSession";
import { useI18n } from "@/i18n/I18nContext";

export default function Files() {
  const { fileAssetId } = useParams<{ fileAssetId?: string }>();
  const navigate = useNavigate();
  const session = useSession();
  const { t } = useI18n();

  const targetId: Id16 | null = fileAssetId ? JSON.parse(fileAssetId) : null;
  const { data, loading, error, refetch } = useQuery<LoadedAggregate>("GetFileAsset", targetId);

  return (
    <div className="max-w-3xl">
      <h1 className="text-xl font-semibold text-onyx-text">{t("files.title")}</h1>

      <IdLookup onLookup={(id) => navigate(`/files/${JSON.stringify(id)}`)} />

      <UploadPanel session={session} onUploaded={(id) => navigate(`/files/${JSON.stringify(id)}`)} />

      {targetId && (
        <div className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-4">
          {loading && <p className="text-sm text-onyx-text-dim">{t("common.loading")}</p>}
          {error && <p className="text-sm text-onyx-status-blocked">{error.message}</p>}
          {!loading && !error && data === null && (
            <p className="text-sm text-onyx-text-dim">{t("files.noFileFoundForId")}</p>
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
  const { t } = useI18n();
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
        {t("files.lookupByFileId")}
      </label>
      <div className="mt-1 flex gap-2">
        <input
          value={raw}
          onChange={(e) => setRaw(e.target.value)}
          placeholder={t("common.id16Placeholder")}
          className="flex-1 rounded-md border border-onyx-border bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          onClick={submit}
          className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover"
        >
          {t("common.view")}
        </button>
      </div>
      {parseError && <p className="mt-1 text-xs text-onyx-status-blocked">{parseError}</p>}
    </div>
  );
}

const MAX_FILE_SIZE_BYTES = 100 * 1024 * 1024;

function UploadPanel({
  session,
  onUploaded,
}: {
  session: ReturnType<typeof useSession>;
  onUploaded: (id: Id16) => void;
}) {
  const { t } = useI18n();
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
      <h2 className="text-sm font-medium text-onyx-text">{t("files.uploadFile")}</h2>
      <p className="mt-1 text-xs text-onyx-text-dim">
        {t("files.uploadPathNote")} {MAX_FILE_SIZE_BYTES / (1024 * 1024)} MB.
      </p>
      <div className="mt-2 flex gap-2">
        <input
          value={path}
          onChange={(e) => setPath(e.target.value)}
          placeholder={t("files.uploadPathPlaceholder")}
          className="flex-1 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          onClick={() => void upload()}
          disabled={busy || path.trim().length === 0}
          className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
        >
          {busy ? t("common.uploading") : t("common.upload")}
        </button>
      </div>
      {error && <p className="mt-1 text-xs text-onyx-status-blocked">{error}</p>}
      {lastHash && (
        <p className="mt-2 break-all text-xs text-onyx-text-dim">
          {t("files.uploadedContentHash")} <span className="text-onyx-text">{lastHash}</span>
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
  const { t } = useI18n();
  const fileName = String(asset.aggregate.file_name ?? t("files.unnamed"));
  const mimeType = String(asset.aggregate.mime_type ?? t("files.unknownType"));
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

function VersionList({ versions }: { versions: { content_hash?: string; size_bytes?: number }[] }) {
  const { t } = useI18n();
  const [destination, setDestination] = useState("");
  const [busyHash, setBusyHash] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);

  async function download(contentHash: string) {
    if (destination.trim().length === 0) {
      setError(t("files.destinationRequired"));
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
      setSaved(t("files.bytesWrittenTo", { count: bytesWritten, path: destination }));
    } catch (e) {
      setError(isShellError(e) ? e.message : String(e));
    } finally {
      setBusyHash(null);
    }
  }

  if (versions.length === 0) {
    return (
      <p className="mt-4 text-sm text-onyx-text-dim">
        {t("files.noVersions")}
      </p>
    );
  }

  return (
    <div className="mt-4">
      <h3 className="text-sm font-medium text-onyx-text">{t("files.versions")}</h3>
      <div className="mt-2">
        <label className="block text-xs font-medium text-onyx-text-dim">{t("files.downloadDestination")}</label>
        <input
          value={destination}
          onChange={(e) => setDestination(e.target.value)}
          placeholder={t("files.downloadPathPlaceholder")}
          className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
      </div>
      <ul className="mt-3 space-y-2">
        {versions.map((v, i) => {
          const hash =
            typeof v.content_hash === "string"
              ? v.content_hash
              : String((v.content_hash as unknown as { 0?: string } | undefined)?.[0] ?? "");
          return (
            <li key={`${hash}-${i}`} className="rounded-md border border-onyx-border bg-onyx-bg p-2 text-xs">
              <p className="break-all text-onyx-text-dim">
                {t("files.versionSummary", { version: i + 1, size: v.size_bytes ?? 0 })}
              </p>
              <p className="mt-1 break-all text-onyx-text-dim">{hash}</p>
              <button
                type="button"
                onClick={() => void download(hash)}
                disabled={busyHash !== null || hash.length === 0}
                className="mt-2 rounded-md bg-onyx-surface-hover px-3 py-1 text-xs text-onyx-text disabled:opacity-50"
              >
                {busyHash === hash ? t("common.downloading") : t("common.download")}
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
  const { t } = useI18n();
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

  async function runSimple(hook: ReturnType<typeof useCommand>, commandType: string, payload: unknown) {
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
      <h3 className="text-sm font-medium text-onyx-text">{t("files.access")}</h3>
      <div className="mt-2 flex gap-2">
        <input
          value={userIdRaw}
          onChange={(e) => setUserIdRaw(e.target.value)}
          placeholder={t("files.userIdPlaceholder")}
          className="flex-1 rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none"
        />
        <button
          type="button"
          disabled={grantCmd.loading}
          onClick={() => void runWithUserId(grantCmd, "GrantFileAccess")}
          className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
        >
          {t("files.grant")}
        </button>
        <button
          type="button"
          disabled={revokeCmd.loading}
          onClick={() => void runWithUserId(revokeCmd, "RevokeFileAccess")}
          className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
        >
          {t("files.revoke")}
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
          onClick={() => void runSimple(quarantineCmd, "QuarantineFile", { QuarantineFile: { reason: "Flagged for review" } })}
          className="rounded-md bg-onyx-status-review/15 px-3 py-1.5 text-xs text-onyx-status-review hover:bg-onyx-status-review/25 disabled:opacity-50"
        >
          {t("files.quarantine")}
        </button>
        <button
          type="button"
          disabled={archiveCmd.loading}
          onClick={() => void runSimple(archiveCmd, "ArchiveFile", { ArchiveFile: { reason: null } })}
          className="rounded-md bg-onyx-status-closed/15 px-3 py-1.5 text-xs text-onyx-status-closed hover:bg-onyx-status-closed/25 disabled:opacity-50"
        >
          {t("files.archive")}
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
