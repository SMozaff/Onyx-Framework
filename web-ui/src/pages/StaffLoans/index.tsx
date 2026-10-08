import { useI18n } from '../../i18n/I18nContext';
import { useMemo, useState } from 'react';
import { useOnyxQuery } from '../../hooks/useQuery';
import { useAuthStore } from '../../stores/authStore';
import type { StaffLoanProjection } from '../../types/query';
import CreateLoanForm from './CreateLoanForm';
import LoanCard from './LoanCard';

/**
 * Staff loans — list + per-row actions, following the same
 * master-detail-adjacent shape as `Approvals` (a flat card list with
 * inline actions, since a loan's whole state fits on one card, unlike
 * Todo/Target's richer detail panel). Lives in `web-ui`, not
 * `admin-shell`, for the same reason `TodoTargets` does: design doc
 * §2.1 confirms any user may request a loan, the real owner approves
 * it, and the staff member approves extensions — three different
 * non-Admin roles interact with this feature directly.
 *
 * Scope of this slice: request, approve, decline, extend, end. Does
 * NOT surface `ExpireStaffLoan` — per
 * `todo_domain::command::StaffLoanCommand`'s own doc comment, that
 * command is invoked only by the scheduled background job (design doc
 * §2.1's advance-warning/expiry job), never a user action.
 */
export default function StaffLoansPage() {
  const { t } = useI18n();
  const query = useOnyxQuery<StaffLoanProjection>('staff_loan.list');
  const user = useAuthStore((state) => state.user);
  const [filter, setFilter] = useState<'all' | 'mine' | 'escalated'>('all');

  const loans = useMemo(() => {
    const all = query.data?.data ?? [];
    if (filter === 'all' || !user) return all;
    if (filter === 'escalated') return all.filter((loan) => loan.escalated_to === user.id);
    return all.filter(
      (loan) =>
        loan.staff_user_id === user.id || loan.real_owner_id === user.id || loan.borrowing_manager_id === user.id,
    );
  }, [query.data, filter, user]);

  return (
    <div className="page-stack">
      <header className="page-header">
        <div>
          <p className="eyebrow">{t('tasks.staffWorkflow')}</p>
          <h1>{t('nav.staffLoans')}</h1>
          <p>{t('common.staffLoanDescription')}</p>
        </div>
        <label className="filter-label">
          Show
          <select value={filter} onChange={(e) => setFilter(e.target.value as 'all' | 'mine' | 'escalated')}>
            <option value="all">{t('common.allLoans')}</option>
            <option value="mine">{t('common.involvingMe')}</option>
            <option value="escalated">{t('approvals.escalatedToYou')}</option>
          </select>
        </label>
      </header>

      {user ? <CreateLoanForm /> : null}

      <section className="panel">
        <div className="panel-heading">
          <h2>{t('common.loans')}</h2>
          <span>{loans.length} shown</span>
        </div>
        {query.isLoading ? (
          <div className="skeleton-list" />
        ) : loans.length === 0 ? (
          <p style={{ color: 'var(--muted)', padding: '12px 4px' }}>
            {filter === 'mine'
              ? t('common.noLoansInvolvingYou')
              : filter === 'escalated'
                ? t('common.noLoanApprovalsEscalated')
                : t('common.noLoansYetRequestOne')}
          </p>
        ) : (
          <div className="card-list">
            {loans.map((loan) => (
              <LoanCard key={loan.id} loan={loan} currentUserId={user?.id ?? null} />
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
