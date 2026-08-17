import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect } from "react";
import { useCommand } from "@/hooks/useCommand";
import { useQuery } from "@/hooks/useQuery";
import { useSession } from "@/hooks/useSession";
import type { Id16, LoadedAggregate } from "@/types/onyx";

interface DesktopNotification {
  id: Id16;
  title: string;
  message: string;
  priority: string;
  status: string;
  source_type: string;
  created_at: string;
  acknowledged_at: string | null;
  version: number;
  lifecycle_epoch: number;
  authority_epoch: number;
}

type NotificationInbox = LoadedAggregate & {
  aggregate: { notifications?: DesktopNotification[] };
};

function isNotificationEvent(payload: unknown): boolean {
  if (typeof payload !== "object" || payload === null) return false;
  const aggregateRef = (payload as Record<string, unknown>).aggregate_ref;
  return (
    typeof aggregateRef === "object" &&
    aggregateRef !== null &&
    (aggregateRef as Record<string, unknown>).type === "notification"
  );
}

function priorityClass(priority: string): string {
  switch (priority.toLowerCase()) {
    case "urgent":
    case "high":
      return "bg-onyx-status-blocked/15 text-onyx-status-blocked";
    case "low":
      return "bg-onyx-surface-hover text-onyx-text-dim";
    default:
      return "bg-onyx-accent/15 text-onyx-accent";
  }
}

/**
 * Native notification inbox backed only by desktop-shell's Tauri bridge:
 * `ListNotifications`/`Acknowledge` flow through client-composition's local
 * SQLite replica, while the existing `onyx:event` stream prompts a refetch
 * whenever a committed notification event arrives. No HTTP API or polling
 * layer is introduced here.
 */
export default function Notifications() {
  const session = useSession();
  const { data, loading, error, refetch } = useQuery<NotificationInbox>(
    "ListNotifications",
    session.userId,
  );
  const { execute, loading: acknowledging, error: commandError } = useCommand<"Acknowledge">();

  const refreshForNotificationEvent = useCallback(
    (event: { payload: unknown }) => {
      if (isNotificationEvent(event.payload)) {
        void refetch();
      }
    },
    [refetch],
  );

  useEffect(() => {
    let disposed = false;
    let unlisten: UnlistenFn | undefined;

    async function subscribe() {
      try {
        // The Rust bridge owns the long-lived subscription and forwards
        // matching local-replica events as `onyx:event` to the webview.
        await invoke("subscribe_events", { organizationId: session.organizationId });
        const remove = await listen<unknown>("onyx:event", refreshForNotificationEvent);
        if (disposed) {
          remove();
        } else {
          unlisten = remove;
        }
      } catch {
        // The inbox is still usable with its initial query and explicit
        // Refresh control if IPC subscription setup is temporarily unavailable.
      }
    }

    void subscribe();
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refreshForNotificationEvent, session.organizationId]);

  const notifications = data?.aggregate.notifications ?? [];

  async function acknowledge(notification: DesktopNotification) {
    try {
      await execute({
        commandType: "Acknowledge",
        targetId: notification.id,
        targetType: "notification",
        organizationId: session.organizationId,
        userId: session.userId,
        deviceId: session.deviceId,
        expectedVersion: notification.version,
        expectedLifecycleEpoch: notification.lifecycle_epoch,
        payload: "Acknowledge",
      });
      await refetch();
    } catch {
      // useCommand exposes the serializable native command error below.
    }
  }

  return (
    <div className="max-w-4xl">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold text-onyx-text">Notifications</h1>
          <p className="mt-1 text-sm text-onyx-text-dim">
            Notifications addressed to you in this device&apos;s local ONYX replica.
          </p>
        </div>
        <button
          type="button"
          onClick={() => void refetch()}
          className="rounded-md bg-onyx-surface px-3 py-1.5 text-sm text-onyx-text hover:bg-onyx-surface-hover"
        >
          Refresh
        </button>
      </div>

      <div className="mt-6 space-y-3">
        {loading && <p className="text-sm text-onyx-text-dim">Loading notifications…</p>}
        {error && <p className="text-sm text-onyx-status-blocked">{error.message}</p>}
        {commandError && <p className="text-sm text-onyx-status-blocked">{commandError.message}</p>}
        {!loading && !error && notifications.length === 0 && (
          <p className="rounded-lg border border-dashed border-onyx-border p-5 text-sm text-onyx-text-dim">
            You have no notifications in this local replica.
          </p>
        )}

        {notifications.map((notification) => {
          const acknowledged = notification.status === "acknowledged";
          return (
            <article
              key={JSON.stringify(notification.id)}
              className="rounded-lg border border-onyx-border bg-onyx-surface p-4"
            >
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div>
                  <h2 className="font-medium text-onyx-text">{notification.title}</h2>
                  <p className="mt-1 whitespace-pre-wrap text-sm text-onyx-text-dim">
                    {notification.message}
                  </p>
                </div>
                <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${priorityClass(notification.priority)}`}>
                  {notification.priority}
                </span>
              </div>

              <div className="mt-4 flex flex-wrap items-center justify-between gap-3 text-xs text-onyx-text-dim">
                <span>
                  {notification.source_type} · {new Date(notification.created_at).toLocaleString()}
                </span>
                {acknowledged ? (
                  <span className="text-onyx-status-approved">Acknowledged</span>
                ) : (
                  <button
                    type="button"
                    disabled={acknowledging}
                    onClick={() => void acknowledge(notification)}
                    className="rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    {acknowledging ? "Acknowledging…" : "Acknowledge"}
                  </button>
                )}
              </div>
            </article>
          );
        })}
      </div>
    </div>
  );
}
