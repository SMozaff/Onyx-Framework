import { useState } from 'react';
import StatusBadge from '../../components/StatusBadge';
import { useAuthStore } from '../../stores/authStore';
import { useDecideList, useSubmitList } from '../../hooks/useCommand';
import type { TargetListProjection, TodoListProjection, VerificationOutcome } from '../../types/query';
import DecisionDialog from './DecisionDialog';

type ListItem = TodoListProjection | TargetListProjection;

/**
 * Detail panel for a single Todo/Target list. Shows items (TodoList)
 * or the time window (TargetList), the Team Leader pre-check if one
 * exists — respecting D.5's redaction (see the `notes` handling below,
 * which must distinguish "no pre-check" from "pre-check exists but
 * substance is hidden from you") — and the actions available at the
 * list's current status: Submit (Draft only, by the owner), and
 * Verify/Reject/Escalate (Submitted or TeamLeaderPreChecked, gated
 * server-side by D.4's verifier-resolution — a caller who isn't
 * authorized gets a real error from the server, surfaced via the
 * existing toast path, not pre-validated here).
 */
export default function ListDetail({ list, kind }: { list: ListItem; kind: 'todo_list' | 'target_list' }) {
  const user = useAuthStore((state) => state.user);
  const submit = useSubmitList(kind);
  const decide = useDecideList(kind);
  const [dialog, setDialog] = useState<'verify' | 'reject' | 'escalate' | null>(null);

  const isOwner = user?.id === list.owner;
  const canSubmit = list.status === 'Draft';
  const canDecide = list.status === 'Submitted' || list.status === 'TeamLeaderPreChecked';

  function confirmDecision(payload: { outcome?: VerificationOutcome; comment?: string; reason?: string }) {
    if (!dialog) return;
    decide.mutate(
      { list, decision: dialog, ...payload },
      { onSuccess: () => setDialog(null) },
    );
  }

  return (
    <section className="detail-panel" aria-labelledby="list-title">
      <div className="detail-header">
        <div>
          <p className="eyebrow">{kind === 'todo_list' ? 'Todo list' : 'Target'}</p>
          <h2 id="list-title">
            {kind === 'todo_list'
              ? `${(list as TodoListProjection).items.length} items`
              : (list as TargetListProjection).description}
          </h2>
        </div>
        <StatusBadge status={list.status} />
      </div>

      {kind === 'todo_list' ? (
        <TodoItems list={list as TodoListProjection} />
      ) : (
        <TargetWindow list={list as TargetListProjection} />
      )}

      <dl className="detail-grid">
        <div>
          <dt>Origin</dt>
          <dd>{list.origin === 'ManagerAssigned' ? 'Manager-assigned' : 'Self-authored'}</dd>
        </div>
        <div>
          <dt>Version</dt>
          <dd>{list.version}</dd>
        </div>
      </dl>

      <PreCheckSection list={list} />

      <div className="detail-section">
        <h3>Actions</h3>
        <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
          {canSubmit && isOwner ? (
            <button
              type="button"
              className="button-primary"
              disabled={submit.isPending}
              onClick={() => submit.mutate(list)}
            >
              {submit.isPending ? 'Submitting…' : 'Submit for verification'}
            </button>
          ) : null}
          {canSubmit && !isOwner ? <p className="muted">Only the owner can submit this list.</p> : null}
          {canDecide ? (
            <>
              <button type="button" className="button-primary" onClick={() => setDialog('verify')}>
                Verify
              </button>
              <button type="button" className="button-secondary" onClick={() => setDialog('escalate')}>
                Escalate
              </button>
              <button type="button" className="button-danger" onClick={() => setDialog('reject')}>
                Reject
              </button>
            </>
          ) : null}
          {!canSubmit && !canDecide ? <p className="muted">No actions available at this status.</p> : null}
        </div>
      </div>

      {dialog ? (
        <DecisionDialog decision={dialog} busy={decide.isPending} onCancel={() => setDialog(null)} onConfirm={confirmDecision} />
      ) : null}
    </section>
  );
}

function TodoItems({ list }: { list: TodoListProjection }) {
  return (
    <div className="detail-section">
      <h3>Items</h3>
      {list.items.length === 0 ? (
        <p className="muted">No items.</p>
      ) : (
        <ul style={{ margin: 0, paddingLeft: 20 }}>
          {list.items.map((item) => (
            <li key={item.item_id}>{item.description}</li>
          ))}
        </ul>
      )}
    </div>
  );
}

function TargetWindow({ list }: { list: TargetListProjection }) {
  return (
    <dl className="detail-grid">
      <div>
        <dt>Window starts</dt>
        <dd>{new Date(list.time_window.start_at / 1_000_000).toLocaleString()}</dd>
      </div>
      <div>
        <dt>Window ends</dt>
        <dd>{new Date(list.time_window.end_at / 1_000_000).toLocaleString()}</dd>
      </div>
    </dl>
  );
}

/**
 * Renders the Team Leader pre-check, if one exists, honoring D.5's
 * redaction: `team_leader_pre_check` being entirely absent means no
 * pre-check has happened yet; the object being present with `notes`
 * omitted (not `null` — genuinely absent as a key) means a pre-check
 * happened but its substance is hidden from this viewer (they are the
 * Staff owner). This distinction matters — showing "no pre-check" when
 * one actually occurred but is merely hidden from you would be
 * misleading about the list's real state.
 */
function PreCheckSection({ list }: { list: ListItem }) {
  const preCheck = list.team_leader_pre_check;
  if (!preCheck) return null;
  const hasNotes = typeof preCheck.notes === 'string';
  return (
    <div className="detail-section">
      <h3>Team Leader pre-check</h3>
      <p className="muted">
        Checked {new Date(preCheck.checked_at / 1_000_000).toLocaleString()}. This is informal and does not gate
        verification.
      </p>
      {hasNotes ? <p>{preCheck.notes}</p> : <p className="muted">Notes are not shown to the list owner.</p>}
    </div>
  );
}
