/**
 * Renders a mission/task status string as a colored pill. Status values
 * come directly from the Rust domain enums (`MissionStatus`/`TaskStatus`
 * in `mission-domain`/`work-domain`), which serde serializes as their
 * bare Rust variant names — e.g. `"Active"`, `"AwaitingApproval"`,
 * `"Reopened"` — so the keys below are an exhaustive-as-known mapping of
 * those exact strings, not free-form UI copy.
 *
 * Deliberately tolerant of an unrecognized status: falls back to a
 * neutral badge with the raw string rather than crashing, since this
 * component has no way to guarantee it covers every current and future
 * variant of two separate Rust enums it doesn't import types from
 * directly (the Tauri IPC boundary only carries JSON, not Rust types).
 */

const STATUS_STYLES: Record<string, string> = {
  Draft: "bg-onyx-status-draft/15 text-onyx-status-draft border-onyx-status-draft/30",
  Ready: "bg-onyx-status-active/15 text-onyx-status-active border-onyx-status-active/30",
  Active: "bg-onyx-status-active/15 text-onyx-status-active border-onyx-status-active/30",
  Planning: "bg-onyx-status-active/15 text-onyx-status-active border-onyx-status-active/30",
  AwaitingApproval: "bg-onyx-status-review/15 text-onyx-status-review border-onyx-status-review/30",
  Review: "bg-onyx-status-review/15 text-onyx-status-review border-onyx-status-review/30",
  Submitted: "bg-onyx-status-review/15 text-onyx-status-review border-onyx-status-review/30",
  Approved: "bg-onyx-status-approved/15 text-onyx-status-approved border-onyx-status-approved/30",
  Paused: "bg-onyx-status-blocked/15 text-onyx-status-blocked border-onyx-status-blocked/30",
  Halted: "bg-onyx-status-blocked/15 text-onyx-status-blocked border-onyx-status-blocked/30",
  Blocked: "bg-onyx-status-blocked/15 text-onyx-status-blocked border-onyx-status-blocked/30",
  Closed: "bg-onyx-status-closed/15 text-onyx-status-closed border-onyx-status-closed/30",
  Cancelled: "bg-onyx-status-closed/15 text-onyx-status-closed border-onyx-status-closed/30",
  Archived: "bg-onyx-status-closed/15 text-onyx-status-closed border-onyx-status-closed/30",
  Reopened: "bg-onyx-status-review/15 text-onyx-status-review border-onyx-status-review/30",
};

const FALLBACK_STYLE = "bg-onyx-text-dim/15 text-onyx-text-dim border-onyx-text-dim/30";

export interface StatusBadgeProps {
  status: string;
}

export default function StatusBadge({ status }: StatusBadgeProps) {
  const style = STATUS_STYLES[status] ?? FALLBACK_STYLE;
  return (
    <span
      className={`inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-medium ${style}`}
    >
      {status}
    </span>
  );
}
