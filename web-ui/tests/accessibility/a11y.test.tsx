import { render } from '@testing-library/react';
import { axe } from 'vitest-axe';
import { describe, expect, it } from 'vitest';
import StatusBadge from '../../src/components/StatusBadge';
import ApprovalDialog from '../../src/components/ApprovalDialog';

describe('accessibility', () => {
  it('renders status with text and no axe violations', async () => {
    const { container } = render(<StatusBadge status="active" />);
    const results = await axe(container);
    expect(results.violations).toHaveLength(0);
  });

  it('renders an accessible approval dialog', async () => {
    const approval = {
      id: 'a',
      title: 'Approve evidence',
      description: 'Review',
      status: 'pending' as const,
      requested_by: 'User',
      target_id: 't',
      target_type: 'task',
      created_at: '2026-08-05T00:00:00Z',
      decided_at: null,
      decision_reason: null,
      web_action_permitted: true,
      version: 1,
      lifecycle_epoch: 0,
      authority_epoch: 0,
    };
    const { container } = render(
      <ApprovalDialog
        approval={approval}
        decision="approve"
        busy={false}
        onCancel={() => {}}
        onConfirm={() => {}}
      />,
    );
    const results = await axe(container);
    expect(results.violations).toHaveLength(0);
  });
});
