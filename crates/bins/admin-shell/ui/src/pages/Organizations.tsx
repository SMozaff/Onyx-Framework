import { useEffect, useState, type FormEvent } from "react";
import { useI18n } from "@/i18n/I18nContext";
import { describeError } from "@/utils/errorHandler";
import {
  organizationsApi,
  type OrganizationAdminRow,
  type OrganizationRow,
} from "./organizationsApi";

const SYSTEM_ORGANIZATION_ID = "__allfather__";

export default function Organizations() {
  const { t } = useI18n();
  const [organizations, setOrganizations] = useState<OrganizationRow[] | null>(null);
  const [selectedOrganizationId, setSelectedOrganizationId] = useState<string>("");
  const [admins, setAdmins] = useState<OrganizationAdminRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [adminsError, setAdminsError] = useState<string | null>(null);

  async function refreshOrganizations() {
    try {
      const rows = await organizationsApi.listOrganizations();
      setOrganizations(rows);
      setSelectedOrganizationId((selected) => selected || rows.find((row) => row.id !== SYSTEM_ORGANIZATION_ID)?.id || "");
      setError(null);
    } catch (cause) {
      setError(describeError(cause));
    }
  }

  async function refreshAdmins(organizationId = selectedOrganizationId) {
    if (!organizationId) {
      setAdmins(null);
      return;
    }
    try {
      setAdmins(await organizationsApi.listAdmins(organizationId));
      setAdminsError(null);
    } catch (cause) {
      setAdminsError(describeError(cause));
    }
  }

  useEffect(() => { void refreshOrganizations(); }, []);
  useEffect(() => { void refreshAdmins(selectedOrganizationId); }, [selectedOrganizationId]);

  const selectedOrganization = organizations?.find((row) => row.id === selectedOrganizationId) ?? null;
  return (
    <div className="max-w-6xl">
      <p className="text-[0.66rem] font-extrabold tracking-[0.15em] text-onyx-accent">{t("common.allFatherAdministration")}</p>
      <h1 className="mt-2 text-2xl font-semibold tracking-[-0.03em] text-onyx-text">{t("nav.organizations")}</h1>
      <p className="mt-1 text-sm text-onyx-text-dim">{t("common.organizationManagementDescription")}</p>

      <CreateOrganizationForm onCreated={async (organization) => {
        await refreshOrganizations();
        setSelectedOrganizationId(organization.id);
      }} />

      {error && <p className="mt-4 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-onyx-status-blocked" role="alert">{error}</p>}
      {organizations === null && !error && <p className="mt-4 text-sm text-onyx-text-dim">{t("common.loading")}</p>}
      {organizations?.length === 0 && <p className="mt-4 rounded-lg border border-dashed border-onyx-border p-4 text-sm text-onyx-text-dim">{t("common.noOrganizationsYet")}</p>}

      {organizations && organizations.length > 0 && (
        <section className="mt-6 rounded-xl border border-onyx-border bg-white p-5 shadow-sm">
          <div className="flex flex-wrap items-baseline justify-between gap-2">
            <h2 className="text-base font-semibold text-onyx-text">{t("common.organizations")}</h2>
            <button type="button" onClick={() => void refreshOrganizations()} className="rounded-md border border-onyx-border px-3 py-1.5 text-xs font-bold text-onyx-text hover:bg-onyx-surface-hover">
              {t("common.refresh")}
            </button>
          </div>
          <div className="mt-4 space-y-3">
            {organizations.map((organization) => (
              <OrganizationCard
                key={organization.id}
                organization={organization}
                selected={selectedOrganizationId === organization.id}
                onSelect={() => setSelectedOrganizationId(organization.id)}
                onChanged={async () => { await refreshOrganizations(); await refreshAdmins(organization.id); }}
              />
            ))}
          </div>
        </section>
      )}

      {selectedOrganization && selectedOrganization.id !== SYSTEM_ORGANIZATION_ID && (
        <AdminsPanel
          organization={selectedOrganization}
          admins={admins}
          error={adminsError}
          onChanged={() => void refreshAdmins()}
        />
      )}
    </div>
  );
}

function CreateOrganizationForm({ onCreated }: { onCreated: (organization: OrganizationRow) => Promise<void> }) {
  const { t } = useI18n();
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const organization = await organizationsApi.createOrganization(name);
      setName("");
      await onCreated(organization);
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form onSubmit={(event) => void submit(event)} className="mt-6 rounded-xl border border-onyx-border bg-white p-5 shadow-sm">
      <h2 className="text-base font-semibold text-onyx-text">{t("common.createOrganization")}</h2>
      <div className="mt-4 flex flex-col gap-3 sm:flex-row">
        <label className="min-w-0 flex-1 text-xs font-bold text-onyx-text">
          {t("common.organizationName")}
          <input value={name} onChange={(event) => setName(event.target.value)} required maxLength={160} className="mt-1.5 w-full rounded-lg border border-onyx-border px-3 py-2 text-sm text-onyx-text" />
        </label>
        <button type="submit" disabled={busy || !name.trim()} className="self-end rounded-lg bg-onyx-accent px-4 py-2 text-sm font-bold text-white disabled:cursor-not-allowed disabled:opacity-50">
          {busy ? t("common.creating") : t("common.createOrganization")}
        </button>
      </div>
      {error && <p className="mt-3 text-xs text-onyx-status-blocked" role="alert">{error}</p>}
    </form>
  );
}

function OrganizationCard({ organization, selected, onSelect, onChanged }: {
  organization: OrganizationRow;
  selected: boolean;
  onSelect: () => void;
  onChanged: () => Promise<void>;
}) {
  const { t } = useI18n();
  const [name, setName] = useState(organization.name);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const system = organization.id === SYSTEM_ORGANIZATION_ID;

  async function rename() {
    setBusy(true);
    setError(null);
    try {
      await organizationsApi.renameOrganization(organization.id, name);
      await onChanged();
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setBusy(false);
    }
  }

  async function toggleActive() {
    if (!organization.is_active && !window.confirm(t("common.confirmActivateOrganization"))) return;
    if (organization.is_active && !window.confirm(t("common.confirmDeactivateOrganization"))) return;
    setBusy(true);
    setError(null);
    try {
      await organizationsApi.setOrganizationActive(organization.id, !organization.is_active);
      await onChanged();
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <article className={`rounded-lg border p-3 ${selected ? "border-onyx-accent bg-sky-50/50" : "border-onyx-border"}`}>
      <div className="flex flex-wrap items-end gap-3">
        <label className="min-w-48 flex-1 text-xs font-bold text-onyx-text">
          {t("common.organizationName")}
          <input value={name} onChange={(event) => setName(event.target.value)} disabled={busy || system} maxLength={160} className="mt-1.5 w-full rounded-md border border-onyx-border bg-white px-2.5 py-2 text-sm text-onyx-text disabled:bg-slate-100" />
        </label>
        <span className={`onyx-state-chip ${organization.is_active ? "bg-emerald-50 text-emerald-700" : "bg-slate-100 text-slate-600"}`}>{organization.is_active ? t("status.active") : t("common.inactive")}</span>
        {system ? (
          <span className="text-xs text-onyx-text-dim">{t("common.systemOrganization")}</span>
        ) : (
          <>
            <button type="button" onClick={() => void rename()} disabled={busy || name.trim() === organization.name} className="rounded-md border border-onyx-border px-3 py-2 text-xs font-bold text-onyx-text disabled:opacity-50">{t("common.rename")}</button>
            <button type="button" onClick={() => void toggleActive()} disabled={busy} className="rounded-md border border-onyx-border px-3 py-2 text-xs font-bold text-onyx-text disabled:opacity-50">{organization.is_active ? t("common.deactivate") : t("common.activate")}</button>
            <button type="button" onClick={onSelect} className="rounded-md bg-onyx-surface-hover px-3 py-2 text-xs font-bold text-onyx-text">{t("common.manageAdmins")}</button>
          </>
        )}
      </div>
      {error && <p className="mt-2 text-xs text-onyx-status-blocked" role="alert">{error}</p>}
    </article>
  );
}

function AdminsPanel({ organization, admins, error, onChanged }: {
  organization: OrganizationRow;
  admins: OrganizationAdminRow[] | null;
  error: string | null;
  onChanged: () => void;
}) {
  const { t } = useI18n();
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  async function create(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setFormError(null);
    try {
      await organizationsApi.createAdmin({ organization_id: organization.id, username, email, password });
      setUsername(""); setEmail(""); setPassword("");
      onChanged();
    } catch (cause) {
      setFormError(describeError(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="mt-6 rounded-xl border border-onyx-border bg-white p-5 shadow-sm">
      <h2 className="text-base font-semibold text-onyx-text">{t("common.organizationAdmins")}</h2>
      <p className="mt-1 text-xs text-onyx-text-dim">{organization.name}</p>
      {!organization.is_active && <p className="mt-3 text-sm text-onyx-status-blocked">{t("common.organizationInactive")}</p>}
      {error && <p className="mt-3 text-sm text-onyx-status-blocked" role="alert">{error}</p>}
      {admins === null && !error && <p className="mt-3 text-sm text-onyx-text-dim">{t("common.loading")}</p>}
      {admins?.length === 0 && <p className="mt-3 text-sm text-onyx-text-dim">{t("common.noOrganizationAdminsYet")}</p>}
      {admins && admins.length > 0 && (
        <div className="mt-4 overflow-x-auto rounded-lg border border-onyx-border">
          <table className="w-full text-left text-sm">
            <thead className="bg-slate-50 text-onyx-text-dim"><tr><th className="px-3 py-2 font-medium">{t("auth.username")}</th><th className="px-3 py-2 font-medium">{t("common.contactEmail")}</th><th className="px-3 py-2 font-medium">{t("status.active")}</th><th className="px-3 py-2 font-medium">{t("common.actions")}</th></tr></thead>
            <tbody>{admins.map((admin) => <AdminRow key={admin.id} admin={admin} onChanged={onChanged} />)}</tbody>
          </table>
        </div>
      )}
      <form onSubmit={(event) => void create(event)} className="mt-5 grid gap-3 sm:grid-cols-2">
        <label className="text-xs font-bold text-onyx-text">{t("auth.username")}<input value={username} onChange={(event) => setUsername(event.target.value)} disabled={busy || !organization.is_active} required className="mt-1.5 w-full rounded-md border border-onyx-border px-2.5 py-2 text-sm disabled:bg-slate-100" /></label>
        <label className="text-xs font-bold text-onyx-text">{t("common.contactEmail")}<input value={email} onChange={(event) => setEmail(event.target.value)} disabled={busy || !organization.is_active} required type="email" className="mt-1.5 w-full rounded-md border border-onyx-border px-2.5 py-2 text-sm disabled:bg-slate-100" /></label>
        <label className="text-xs font-bold text-onyx-text">{t("auth.initialPassword")}<input value={password} onChange={(event) => setPassword(event.target.value)} disabled={busy || !organization.is_active} required minLength={8} type="password" className="mt-1.5 w-full rounded-md border border-onyx-border px-2.5 py-2 text-sm disabled:bg-slate-100" /></label>
        <button type="submit" disabled={busy || !organization.is_active || !username || !email || !password} className="self-end rounded-lg bg-onyx-accent px-4 py-2 text-sm font-bold text-white disabled:cursor-not-allowed disabled:opacity-50">{busy ? t("common.creating") : t("common.createOrganizationAdmin")}</button>
      </form>
      {formError && <p className="mt-3 text-xs text-onyx-status-blocked" role="alert">{formError}</p>}
    </section>
  );
}

function AdminRow({ admin, onChanged }: { admin: OrganizationAdminRow; onChanged: () => void }) {
  const { t } = useI18n();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function toggle() {
    if (admin.is_active && !window.confirm(t("common.confirmDeactivateOrganizationAdmin"))) return;
    setBusy(true);
    setError(null);
    try {
      if (admin.is_active) await organizationsApi.deactivateAdmin(admin.id);
      else await organizationsApi.reactivateAdmin(admin.id);
      onChanged();
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setBusy(false);
    }
  }

  return <tr className="border-t border-onyx-border">
    <td className="px-3 py-2 text-onyx-text">{admin.username}</td>
    <td className="px-3 py-2 text-onyx-text-dim">{admin.email ?? "—"}</td>
    <td className="px-3 py-2 text-onyx-text-dim">{admin.is_active ? t("status.active") : t("common.inactive")}</td>
    <td className="px-3 py-2"><button type="button" onClick={() => void toggle()} disabled={busy} className="rounded-md border border-onyx-border px-2.5 py-1.5 text-xs font-bold text-onyx-text disabled:opacity-50">{admin.is_active ? t("common.deactivate") : t("common.reactivate")}</button>{error && <p className="mt-1 text-xs text-onyx-status-blocked" role="alert">{error}</p>}</td>
  </tr>;
}
