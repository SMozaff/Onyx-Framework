import { useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { observerApi } from '../../api/onyx';
import { isContentHash } from '../../utils/validation';
import { normalizeError } from '../../utils/errorHandler';
import type { FileDownload } from '../../types/api';

export function FileDetailPage() {
  const { contentHash = '' } = useParams<{ contentHash: string }>();
  const [result, setResult] = useState<FileDownload | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const invalid = !isContentHash(contentHash);

  const download = async () => {
    setBusy(true);
    setError(null);
    try {
      const file = await observerApi.downloadFile(contentHash);
      setResult(file);
      const url = URL.createObjectURL(file.blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = file.fileName;
      anchor.click();
      URL.revokeObjectURL(url);
    } catch (caught) {
      setError(normalizeError(caught).message);
      setResult(null);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-6">
      <Link className="text-sm text-slate-500 underline underline-offset-4" to="/reports">
        ← Reports
      </Link>

      <header>
        <p className="text-xs uppercase tracking-wide text-slate-500">Content-addressed file</p>
        <h2 className="break-all text-lg font-semibold text-slate-900">{contentHash}</h2>
        <p className="text-sm text-slate-600">
          {invalid
            ? 'A content hash must be a lowercase hex string. This page is reachable only with a valid hash.'
            : 'Download is gated by the can_download_files capability server-side.'}
        </p>
      </header>

      {invalid ? (
        <p className="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700" role="alert">
          Invalid content hash.
        </p>
      ) : (
        <div className="space-y-4">
          <button
            type="button"
            className="rounded bg-[#0a1e3d] px-4 py-2 text-white hover:bg-[#122a4d] disabled:cursor-not-allowed disabled:opacity-60"
            onClick={() => void download()}
            disabled={busy}
          >
            {busy ? 'Downloading…' : 'Download file'}
          </button>

          {error ? (
            <p className="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700" role="alert">
              {error}
            </p>
          ) : null}

          {result ? (
            <dl className="rounded-lg border border-slate-200 bg-white p-4 text-sm">
              <div className="flex justify-between py-1">
                <dt className="text-slate-500">Size</dt>
                <dd className="font-medium text-slate-900">{result.size} bytes</dd>
              </div>
              <div className="flex justify-between py-1">
                <dt className="text-slate-500">Type</dt>
                <dd className="font-medium text-slate-900">{result.blob.type || 'binary'}</dd>
              </div>
            </dl>
          ) : null}
        </div>
      )}
    </div>
  );
}