import { useEffect, useState } from "react";
import { apiClient } from "@/api/client";

/**
 * User management: list, create, deactivate/reactivate, class + parent
 * assignment. Wraps the routes built in Phase A
 * (`api_server::routes::admin`) — see `DECISIONS.md`'s "Manager role"
 * and "Phase A" sections for the backend this drives. Nothing here is
 * new backend capability; this is the first UI for routes that were
 * previously curl-only (see `docs/runbooks/user-class-migration.md`,
 * written before this page existed).
 */

const USER_CLASSES = [
  "top_level_manager",
  "senior_manager",
  "team_leader",
  "supervisor",
  "staff",
] as const;

interface UserRow {
  id: string;
  username: string;
  organization_id: string;
  is_admin: boolean;
  is_manager: boolean;
  class: string | null;
  parent_user_id: string | null;
  is_active: boolean;
}

export default function Users() {
  const [users, setUsers] = useState<UserRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    try {
      const response = await apiClient.get<UserRow[]>("/api/admin/users");
      setUsers(response.data);
      setError(null);
    } catch {
      setError("Failed to load users.");
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  return (
    <div className="max-w-6xl">
      <p className="text-[0.66rem] font-extrabold tracking-[0.15em] text-onyx-accent">IDENTITY MANAGEMENT</p>
      <h1 className="mt-2 text-2xl font-semibold tracking-[-0.03em] text-onyx-text">Users</h1>
      <p className="mt-1 text-sm text-onyx-text-dim">Create, classify, and maintain organization access with explicit account state.</p>

      <CreateUserForm onCreated={() => void refresh()} />

      {error && <p className="mt-4 text-sm text-onyx-status-blocked">{error}</p>}
      {users === null && !error && <p className="mt-4 text-sm text-onyx-text-dim">Loading…</p>}

      {users && (
        <div className="mt-6 overflow-x-auto rounded-xl border border-onyx-border bg-white shadow-sm">
          <table className="w-full text-left text-sm">
            <thead className="border-b border-onyx-border bg-slate-50 text-onyx-text-dim">
              <tr>
                <th className="px-3 py-2 font-medium">Username</th>
                <th className="px-3 py-2 font-medium">Admin</th>
                <th className="px-3 py-2 font-medium">Class</th>
                <th className="px-3 py-2 font-medium">Parent</th>
                <th className="px-3 py-2 font-medium">Active</th>
                <th className="px-3 py-2 font-medium">Actions</th>
              </tr>
            </thead>
            <tbody>
              {users.map((u) => (
                <UserRowView key={u.id} user={u} allUsers={users} onChanged={() => void refresh()} />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function CreateUserForm({ onCreated }: { onCreated: () => void }) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [isAdmin, setIsAdmin] = useState(false);
  const [userClass, setUserClass] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function create() {
    setLoading(true);
    setError(null);
    try {
      await apiClient.post("/api/admin/users", {
        username,
        password,
        is_admin: isAdmin,
        class: userClass || null,
      });
      setUsername("");
      setPassword("");
      setIsAdmin(false);
      setUserClass("");
      onCreated();
    } catch (e) {
      const message =
        (e as { response?: { data?: { message?: string } } }).response?.data?.message ??
        "Failed to create user.";
      setError(message);
    } finally {
      setLoading(false);
    }
  }

  return (
    <section className="mt-6 rounded-xl border border-onyx-border bg-white p-5 shadow-sm">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <div>
          <h2 className="text-base font-semibold text-onyx-text">Create user</h2>
          <p className="mt-1 text-xs text-onyx-text-dim">New accounts receive only the classification and administrator access selected here.</p>
        </div>
        <span className="onyx-state-chip bg-sky-50 text-onyx-accent">New account</span>
      </div>
      <div className="mt-5 grid gap-4 sm:grid-cols-2">
        <label className="block text-xs font-bold text-onyx-text">
          Username
          <input
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            placeholder="Username"
            className="mt-1.5 w-full rounded-lg border border-onyx-border bg-white px-3 py-2.5 text-sm text-onyx-text shadow-sm placeholder:text-slate-400 focus:border-onyx-accent focus:outline-none"
          />
        </label>
        <label className="block text-xs font-bold text-onyx-text">
          Initial password
          <input
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            type="password"
            placeholder="Password"
            className="mt-1.5 w-full rounded-lg border border-onyx-border bg-white px-3 py-2.5 text-sm text-onyx-text shadow-sm placeholder:text-slate-400 focus:border-onyx-accent focus:outline-none"
          />
        </label>
        <label className="block text-xs font-bold text-onyx-text">
          Operational class
          <select
            value={userClass}
            onChange={(e) => setUserClass(e.target.value)}
            className="mt-1.5 w-full rounded-lg border border-onyx-border bg-white px-3 py-2.5 text-sm text-onyx-text shadow-sm focus:border-onyx-accent focus:outline-none"
          >
            <option value="">No class</option>
            {USER_CLASSES.map((c) => (
              <option key={c} value={c}>
                {c}
              </option>
            ))}
          </select>
        </label>
        <label className="mt-6 flex items-center gap-2 rounded-lg border border-onyx-border bg-slate-50 px-3 py-2.5 text-sm font-semibold text-onyx-text">
          <input type="checkbox" checked={isAdmin} onChange={(e) => setIsAdmin(e.target.checked)} />
          Grant administrator access
        </label>
      </div>
      {error && <p className="mt-3 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-xs text-onyx-status-blocked" role="alert">{error}</p>}
      <button
        type="button"
        onClick={() => void create()}
        disabled={loading || !username || !password}
        className="mt-5 rounded-lg bg-onyx-accent px-4 py-2.5 text-sm font-bold text-white shadow-sm transition-colors hover:bg-[#174d7b] disabled:cursor-not-allowed disabled:opacity-50"
      >
        {loading ? "Creating…" : "Create user"}
      </button>
    </section>
  );
}

function UserRowView({
  user,
  allUsers,
  onChanged,
}: {
  user: UserRow;
  allUsers: UserRow[];
  onChanged: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function setClass(newClass: string) {
    setBusy(true);
    setError(null);
    try {
      await apiClient.post(`/api/admin/users/${user.id}/class`, { class: newClass || null });
      onChanged();
    } catch {
      setError("Failed to set class.");
    } finally {
      setBusy(false);
    }
  }

  async function setParent(parentId: string) {
    setBusy(true);
    setError(null);
    try {
      await apiClient.post(`/api/admin/users/${user.id}/parent`, {
        parent_user_id: parentId || null,
      });
      onChanged();
    } catch {
      setError("Failed to set parent — check for a cycle or invalid reference.");
    } finally {
      setBusy(false);
    }
  }

  async function toggleActive() {
    setBusy(true);
    setError(null);
    try {
      const path = user.is_active ? "deactivate" : "activate";
      await apiClient.post(`/api/admin/users/${user.id}/${path}`);
      onChanged();
    } catch {
      setError("Failed to update status.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <tr className="border-b border-onyx-border last:border-0">
      <td className="px-3 py-2 text-onyx-text">{user.username}</td>
      <td className="px-3 py-2 text-onyx-text-dim">{user.is_admin ? "Yes" : "—"}</td>
      <td className="px-3 py-2">
        <select
          value={user.class ?? ""}
          onChange={(e) => void setClass(e.target.value)}
          disabled={busy}
          className="rounded-md border border-onyx-border bg-onyx-bg px-2 py-1 text-xs text-onyx-text"
        >
          <option value="">Unclassified</option>
          {USER_CLASSES.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </select>
      </td>
      <td className="px-3 py-2">
        <select
          value={user.parent_user_id ?? ""}
          onChange={(e) => void setParent(e.target.value)}
          disabled={busy}
          className="rounded-md border border-onyx-border bg-onyx-bg px-2 py-1 text-xs text-onyx-text"
        >
          <option value="">No parent</option>
          {allUsers
            .filter((u) => u.id !== user.id)
            .map((u) => (
              <option key={u.id} value={u.id}>
                {u.username}
              </option>
            ))}
        </select>
      </td>
      <td className="px-3 py-2 text-onyx-text-dim">{user.is_active ? "Active" : "Inactive"}</td>
      <td className="px-3 py-2">
        <button
          type="button"
          onClick={() => void toggleActive()}
          disabled={busy}
          className="rounded-md bg-onyx-surface px-2 py-1 text-xs text-onyx-text hover:bg-onyx-surface-hover disabled:opacity-50"
        >
          {user.is_active ? "Deactivate" : "Activate"}
        </button>
        {error && <p className="mt-1 text-xs text-onyx-status-blocked">{error}</p>}
      </td>
    </tr>
  );
}
