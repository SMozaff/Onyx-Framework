import { useState } from "react";
import type { FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "@/i18n/I18nContext";
import { userFacingMessage } from "@/utils/userFacingError";

/**
 * Shown before any operational route when the account must change its
 * password (first login or reset). On success the server has revoked every
 * session, so the caller signs out and returns to the login screen.
 */
export default function ChangePassword({ onChanged }: { onChanged: () => Promise<void> }) {
  const { t } = useI18n();
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    setLoading(true);
    try {
      await invoke("change_password", { currentPassword, newPassword });
      await onChanged();
    } catch (caught: unknown) {
      setError(userFacingMessage(caught));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="onyx-auth-shell flex flex-1 items-center justify-center px-5 py-10">
      <form onSubmit={submit} className="onyx-auth-card w-full max-w-md p-6 sm:p-7">
        <h1 className="text-2xl font-medium tracking-[-0.04em] text-onyx-text">{t("auth.changePasswordTitle")}</h1>
        <p className="mt-2 text-sm leading-5 text-onyx-text-dim">{t("auth.changePasswordBody")}</p>

        <div className="mt-6">
          <label htmlFor="current-password" className="block text-xs font-bold text-onyx-text">{t("auth.currentPassword")}</label>
          <input id="current-password" type="password" value={currentPassword} onChange={(event) => setCurrentPassword(event.target.value)} autoComplete="current-password" required className="mt-1.5 w-full rounded-lg border border-onyx-border bg-white px-3 py-2.5 text-sm text-onyx-text shadow-sm focus:border-onyx-accent focus:outline-none" />
        </div>

        <div className="mt-4">
          <label htmlFor="new-password" className="block text-xs font-bold text-onyx-text">{t("auth.newPassword")}</label>
          <input id="new-password" type="password" value={newPassword} onChange={(event) => setNewPassword(event.target.value)} autoComplete="new-password" required minLength={12} className="mt-1.5 w-full rounded-lg border border-onyx-border bg-white px-3 py-2.5 text-sm text-onyx-text shadow-sm focus:border-onyx-accent focus:outline-none" />
        </div>

        {error && <p className="mt-4 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-xs leading-5 text-onyx-status-blocked" role="alert">{error}</p>}

        <button type="submit" disabled={loading} className="mt-5 w-full rounded-lg bg-onyx-accent px-3 py-2.5 text-sm font-bold text-white shadow-sm transition-colors hover:bg-[#174d7b] disabled:cursor-not-allowed disabled:opacity-50">
          {loading ? t("auth.changingPassword") : t("auth.changePasswordSubmit")}
        </button>
      </form>
    </div>
  );
}
