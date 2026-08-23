import { useEffect, useRef, useState } from "react";
import { apiClient } from "@/api/client";
import { describeError } from "@/utils/errorHandler";

/**
 * Staff profile administration: list, single edit (upsert), and batch
 * CSV/JSON import/export. Wraps `routes::profiles`/`routes::profiles::batch`
 * — see `DECISIONS.md`'s "Staff Profiles" section for the confirmed
 * visibility/mandatory-field rules this UI reflects but does not itself
 * enforce (the server is the real authority; this UI just mirrors the
 * same mandatory-field list so a bad submission fails fast).
 */

interface ProfileRow {
  profile_id: string;
  owner_id: string;
  full_name: string;
  photo_blob_key: string | null;
  job_title: string | null;
  contact_email: string | null;
  contact_phone: string | null;
  department: string | null;
  class_label: string | null;
  parent_display_name: string | null;
  team_memberships: string[];
}

export default function Profiles() {
  const [profiles, setProfiles] = useState<ProfileRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<ProfileRow | "new" | null>(null);

  async function refresh() {
    try {
      const response = await apiClient.get<ProfileRow[]>("/api/profiles");
      setProfiles(response.data);
      setError(null);
    } catch (e) {
      setError(describeError(e));
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  return (
    <div className="max-w-5xl">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold text-onyx-text">Staff Profiles</h1>
        <button
          type="button"
          onClick={() => setEditing("new")}
          className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white"
        >
          New profile
        </button>
      </div>

      <ImportExportPanel onImported={() => void refresh()} />

      {error && <p className="mt-4 text-sm text-onyx-status-blocked">{error}</p>}
      {profiles === null && !error && (
        <p className="mt-4 text-sm text-onyx-text-dim">Loading…</p>
      )}

      {profiles && (
        <div className="mt-6 overflow-x-auto rounded-lg border border-onyx-border">
          <table className="w-full text-left text-sm">
            <thead className="border-b border-onyx-border bg-onyx-surface text-onyx-text-dim">
              <tr>
                <th className="px-3 py-2 font-medium">Name</th>
                <th className="px-3 py-2 font-medium">Job title</th>
                <th className="px-3 py-2 font-medium">Department</th>
                <th className="px-3 py-2 font-medium">Class</th>
                <th className="px-3 py-2 font-medium">Contact</th>
                <th className="px-3 py-2 font-medium"></th>
              </tr>
            </thead>
            <tbody>
              {profiles.map((p) => (
                <tr key={p.profile_id} className="border-b border-onyx-border last:border-0">
                  <td className="px-3 py-2 text-onyx-text">{p.full_name}</td>
                  <td className="px-3 py-2 text-onyx-text-dim">{p.job_title ?? "—"}</td>
                  <td className="px-3 py-2 text-onyx-text-dim">{p.department ?? "—"}</td>
                  <td className="px-3 py-2 text-onyx-text-dim">{p.class_label ?? "—"}</td>
                  <td className="px-3 py-2 text-onyx-text-dim">
                    {p.contact_email ?? "—"} · {p.contact_phone ?? "—"}
                  </td>
                  <td className="px-3 py-2">
                    <button
                      type="button"
                      onClick={() => setEditing(p)}
                      className="rounded-md bg-onyx-surface px-2 py-1 text-xs text-onyx-text hover:bg-onyx-surface-hover"
                    >
                      Edit
                    </button>
                  </td>
                </tr>
              ))}
              {profiles.length === 0 && (
                <tr>
                  <td colSpan={6} className="px-3 py-6 text-center text-onyx-text-dim">
                    No profiles yet.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      {editing !== null && (
        <EditProfileDialog
          profile={editing === "new" ? null : editing}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            void refresh();
          }}
        />
      )}
    </div>
  );
}

/**
 * Every field the server requires (see `ImportExportRow::validate` in
 * `routes::profiles::batch`) — mirrored here for fast client-side
 * feedback, not as a substitute for the server's own check.
 */
const MANDATORY_FIELDS = [
  "owner_id",
  "full_name",
  "contact_email",
  "contact_phone",
  "job_title",
  "department",
] as const;

function EditProfileDialog({
  profile,
  onClose,
  onSaved,
}: {
  profile: ProfileRow | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [form, setForm] = useState({
    owner_id: profile?.owner_id ?? "",
    full_name: profile?.full_name ?? "",
    job_title: profile?.job_title ?? "",
    contact_email: profile?.contact_email ?? "",
    contact_phone: profile?.contact_phone ?? "",
    department: profile?.department ?? "",
    photo_blob_key: profile?.photo_blob_key ?? "",
    class_label: profile?.class_label ?? "",
    parent_display_name: profile?.parent_display_name ?? "",
    team_memberships: (profile?.team_memberships ?? []).join(";"),
  });
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  function update<K extends keyof typeof form>(key: K, value: string) {
    setForm((f) => ({ ...f, [key]: value }));
  }

  async function save() {
    const missing = MANDATORY_FIELDS.filter((f) => form[f].trim() === "");
    if (missing.length > 0) {
      setError(`Required: ${missing.join(", ")}`);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      await apiClient.put("/api/admin/profiles", {
        owner_id: form.owner_id,
        full_name: form.full_name,
        job_title: form.job_title || null,
        contact_email: form.contact_email || null,
        contact_phone: form.contact_phone || null,
        department: form.department || null,
        photo_blob_key: form.photo_blob_key || null,
        class_label: form.class_label || null,
        parent_display_name: form.parent_display_name || null,
        team_memberships: form.team_memberships
          .split(";")
          .map((s) => s.trim())
          .filter((s) => s.length > 0),
      });
      onSaved();
    } catch (e) {
      setError(describeError(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center bg-black/40 px-4">
      <div className="w-full max-w-lg rounded-lg border border-onyx-border bg-onyx-surface p-5">
        <h2 className="text-base font-medium text-onyx-text">
          {profile ? "Edit profile" : "New profile"}
        </h2>
        {!profile && (
          <p className="mt-1 text-xs text-onyx-text-dim">
            The user id must already have an account — profile creation does not create
            accounts.
          </p>
        )}

        <div className="mt-3 grid grid-cols-2 gap-2">
          <Field label="User id (UUID)" value={form.owner_id} onChange={(v) => update("owner_id", v)} disabled={!!profile} />
          <Field label="Full name" value={form.full_name} onChange={(v) => update("full_name", v)} />
          <Field label="Job title" value={form.job_title} onChange={(v) => update("job_title", v)} />
          <Field label="Department" value={form.department} onChange={(v) => update("department", v)} />
          <Field label="Contact email" value={form.contact_email} onChange={(v) => update("contact_email", v)} />
          <Field label="Contact phone" value={form.contact_phone} onChange={(v) => update("contact_phone", v)} />
          <Field label="Class label" value={form.class_label} onChange={(v) => update("class_label", v)} />
          <Field label="Parent display name" value={form.parent_display_name} onChange={(v) => update("parent_display_name", v)} />
          <Field label="Photo blob key" value={form.photo_blob_key} onChange={(v) => update("photo_blob_key", v)} />
          <Field
            label="Team memberships (;-separated)"
            value={form.team_memberships}
            onChange={(v) => update("team_memberships", v)}
          />
        </div>

        {error && <p className="mt-3 text-xs text-onyx-status-blocked">{error}</p>}

        <div className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => void save()}
            disabled={loading}
            className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
          >
            {loading ? "Saving…" : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}

function Field({
  label,
  value,
  onChange,
  disabled,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  disabled?: boolean;
}) {
  return (
    <div>
      <label className="block text-xs font-medium text-onyx-text-dim">{label}</label>
      <input
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        className="mt-1 w-full rounded-md border border-onyx-border bg-onyx-bg px-3 py-1.5 text-sm text-onyx-text focus:border-onyx-accent focus:outline-none disabled:opacity-50"
      />
    </div>
  );
}

interface RowResult {
  outcome: "created" | "updated" | "failed";
  user_id: string | null;
  reason?: string;
}

interface ImportSummary {
  total_rows: number;
  created: number;
  updated: number;
  failed: number;
  results: RowResult[];
}

function ImportExportPanel({ onImported }: { onImported: () => void }) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [summary, setSummary] = useState<ImportSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleFileChosen(file: File) {
    setLoading(true);
    setError(null);
    setSummary(null);
    try {
      const form = new FormData();
      form.append("file", file, file.name);
      // `apiClient`'s instance-level default sets Content-Type:
      // application/json (see api/client.ts), which would otherwise
      // silently override FormData's own multipart boundary — axios
      // does not auto-detect FormData and skip a caller-set default the
      // way some libraries do. Explicitly clearing it to `undefined`
      // lets axios/the browser generate the correct
      // `multipart/form-data; boundary=...` header instead. Confirmed
      // by checking client.ts's actual defaults before writing this,
      // not assumed to "just work."
      const response = await apiClient.post<ImportSummary>(
        "/api/admin/profiles/import",
        form,
        { headers: { "Content-Type": undefined } },
      );
      setSummary(response.data);
      onImported();
    } catch (e) {
      setError(describeError(e));
    } finally {
      setLoading(false);
      if (fileInputRef.current) fileInputRef.current.value = "";
    }
  }

  function exportUrl(format: "csv" | "json") {
    return `${apiClient.defaults.baseURL}/api/admin/profiles/export?format=${format}`;
  }

  return (
    <div className="mt-4 rounded-lg border border-onyx-border bg-onyx-surface p-4">
      <h2 className="text-sm font-medium text-onyx-text">Batch import / export</h2>
      <p className="mt-1 text-xs text-onyx-text-dim">
        Mandatory columns: user_id, full_name, contact_email, contact_phone, job_title,
        department. user_id must already have an account. Rows matching an existing profile
        are updated; unmatched rows create a new one. Invalid rows are skipped and reported
        below — the rest of the file still imports.
      </p>

      <div className="mt-3 flex flex-wrap items-center gap-2">
        <input
          ref={fileInputRef}
          type="file"
          accept=".csv,.json,text/csv,application/json"
          onChange={(e) => {
            const file = e.target.files?.[0];
            if (file) void handleFileChosen(file);
          }}
          className="text-xs text-onyx-text-dim"
        />
        {loading && <span className="text-xs text-onyx-text-dim">Importing…</span>}
        <a
          href={exportUrl("csv")}
          target="_blank"
          rel="noreferrer"
          className="rounded-md bg-onyx-surface px-2 py-1 text-xs text-onyx-text hover:bg-onyx-surface-hover"
        >
          Export CSV
        </a>
        <a
          href={exportUrl("json")}
          target="_blank"
          rel="noreferrer"
          className="rounded-md bg-onyx-surface px-2 py-1 text-xs text-onyx-text hover:bg-onyx-surface-hover"
        >
          Export JSON
        </a>
      </div>

      {error && <p className="mt-2 text-xs text-onyx-status-blocked">{error}</p>}

      {summary && (
        <div className="mt-3 rounded-md border border-onyx-border bg-onyx-bg p-3">
          <p className="text-xs text-onyx-text">
            {summary.total_rows} rows — {summary.created} created, {summary.updated} updated,{" "}
            {summary.failed} failed.
          </p>
          {summary.failed > 0 && (
            <ul className="mt-2 space-y-1">
              {summary.results
                .filter((r) => r.outcome === "failed")
                .map((r, i) => (
                  <li key={i} className="text-xs text-onyx-status-blocked">
                    {r.user_id ?? "(no user_id)"}: {r.reason}
                  </li>
                ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}
