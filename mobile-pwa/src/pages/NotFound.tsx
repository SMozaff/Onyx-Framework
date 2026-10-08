import { Link } from 'react-router-dom';
import { useI18n } from '../i18n/I18nContext';

export function NotFoundPage() {
  const { t } = useI18n();
  return (
    <div className="py-12 text-center">
      <h2 className="text-lg font-semibold text-slate-900">{t("common.pageNotFound")}</h2>
      <p className="mt-1 text-sm text-slate-500">
        {t("common.observerProjectionNotFound")}
      </p>
      <Link className="mt-4 inline-block text-sm underline underline-offset-4" to="/dashboard">
        {t("settings.returnToDashboard")}
      </Link>
    </div>
  );
}