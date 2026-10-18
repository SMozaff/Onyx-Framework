import type { AxiosInstance } from "axios";
import { apiClient } from "../api/client";

export type ProfileTransferFormat = "csv" | "json";

type TransferClient = Pick<AxiosInstance, "get" | "post">;

export interface ProfileTransferDependencies {
  client?: TransferClient;
  saveBlob?: (blob: Blob, filename: string) => void;
}

const CONTENT_TYPE_BY_FORMAT: Record<ProfileTransferFormat, string> = {
  csv: "text/csv",
  json: "application/json",
};

function formatForImport(file: Pick<File, "name" | "type">): ProfileTransferFormat {
  const name = file.name.toLowerCase();
  if (name.endsWith(".csv")) return "csv";
  if (name.endsWith(".json")) return "json";

  const contentType = file.type.split(";", 1)[0].trim().toLowerCase();
  if (contentType === CONTENT_TYPE_BY_FORMAT.csv) return "csv";
  if (contentType === CONTENT_TYPE_BY_FORMAT.json) return "json";

  throw new Error("Select a CSV or JSON profile file.");
}

/**
 * Sends the file contents directly because the profile batch endpoint parses the
 * raw request body as CSV or JSON according to Content-Type; it is not a
 * multipart upload. The import only creates or updates profiles for existing
 * accounts, never accounts themselves.
 */
export async function importProfileFile<TResult>(
  file: File,
  { client = apiClient }: ProfileTransferDependencies = {},
): Promise<TResult> {
  const format = formatForImport(file);
  const response = await client.post<TResult>(
    "/api/admin/profiles/import",
    await file.text(),
    { headers: { "Content-Type": CONTENT_TYPE_BY_FORMAT[format] } },
  );
  return response.data;
}

/** Download a Blob through a short-lived object URL, always releasing it. */
export function saveProfileExportBlob(blob: Blob, filename: string): void {
  const objectUrl = URL.createObjectURL(blob);
  const link = document.createElement("a");

  try {
    link.href = objectUrl;
    link.download = filename;
    link.style.display = "none";
    document.body.appendChild(link);
    link.click();
  } finally {
    link.remove();
    URL.revokeObjectURL(objectUrl);
  }
}

/**
 * Retrieves an export through apiClient so its bearer-token interceptor and
 * session-refresh handling apply; a bare navigation cannot carry that header.
 */
export async function exportProfileFile(
  format: ProfileTransferFormat,
  { client = apiClient, saveBlob = saveProfileExportBlob }: ProfileTransferDependencies = {},
): Promise<void> {
  const response = await client.get<Blob>(
    `/api/admin/profiles/export?format=${format}`,
    { responseType: "blob" },
  );
  const blob = response.data instanceof Blob
    ? response.data
    : new Blob([response.data], { type: CONTENT_TYPE_BY_FORMAT[format] });
  saveBlob(blob, `staff-profiles.${format}`);
}
