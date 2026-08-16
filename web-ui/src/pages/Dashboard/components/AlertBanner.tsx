import type { DashboardProjection } from '../../../types/query';
export default function AlertBanner({ data }: { data: DashboardProjection }) {
  if (!data.blocked_tasks && !data.unread_notifications) return <div className="alert-banner alert-success"><strong>Operations stable</strong><span>No blocked tasks or unread operational alerts.</span></div>;
  return <div className="alert-banner"><strong>Attention required</strong><span>{data.blocked_tasks} blocked task(s) and {data.unread_notifications} unread notification(s) need review.</span></div>;
}
