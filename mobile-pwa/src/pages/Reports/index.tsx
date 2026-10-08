import { StatusBadge } from '../../components/StatusBadge';
import { useObserverQuery } from '../../hooks/useQuery';
import type { ReportProjection } from '../../types/query';
import { useI18n } from '../../i18n/I18nContext';

export function ReportsPage() {
  const { t } = useI18n();
  const query = useObserverQuery<ReportProjection>('report.detail');
  const report = query.data?.data?.[0];

  return (
    <div className="space-y-6">
      <header>
        <p className="text-xs uppercase tracking-wide text-slate-500">{t("reports.evidenceViewer")}</p>
        <h2 className="text-lg font-semibold text-slate-900">{t("reports.title")}</h2>
        <p className="text-sm text-slate-600">
          {t("reports.readOnlyAccess")}
        </p>
      </header>

      {query.isPending && <p className="text-sm text-slate-500">{t("common.loading")}</p>}
      {!query.isPending && !report && (
        <p className="text-sm text-slate-500">{t("reports.noPublishedReportProjection")}</p>
      )}

      {report ? (
        <article className="space-y-4 rounded-lg border border-slate-200 bg-white p-4">
          <div className="flex items-start justify-between gap-3">
            <div>
              <p className="text-xs uppercase tracking-wide text-slate-500">{t("reports.operationalReport")}</p>
              <h3 className="text-lg font-semibold text-slate-900">{report.title}</h3>
              <p className="text-xs text-slate-500">
                Prepared by {report.author} · {new Date(report.submitted_at).toLocaleString()}
              </p>
            </div>
            <StatusBadge status={report.status} />
          </div>

          <section>
            <h4 className="mb-2 text-sm font-semibold text-slate-900">{t("reports.summary")}</h4>
            <p className="text-sm text-slate-700">{report.summary}</p>
          </section>

          <section>
            <h4 className="mb-2 text-sm font-semibold text-slate-900">{t("reports.evidenceReferences")}</h4>
            {report.evidence.length === 0 ? (
              <p className="text-sm text-slate-500">{t("reports.noAttachments")}</p>
            ) : (
              <ul className="divide-y divide-slate-200 rounded border border-slate-200">
                {report.evidence.map((item) => (
                  <li key={item.file_name} className="flex items-center justify-between px-3 py-2">
                    <div>
                      <p className="text-sm font-medium text-slate-900">{item.label}</p>
                      <p className="text-xs text-slate-500">{item.file_name}</p>
                    </div>
                    <span className="rounded-full bg-slate-100 px-2 py-0.5 text-xs text-slate-600">
                      {t("common.viewOnly")}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <footer className="text-xs text-slate-400">
            Projection version {report.version} · Updated {new Date(report.updated_at).toLocaleString()}
          </footer>
        </article>
      ) : null}
    </div>
  );
}