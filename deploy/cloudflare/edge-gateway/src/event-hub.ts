interface HubEnv {
  DB: D1Database;
}

interface SocketAttachment {
  organization_id: string;
  user_id: string;
  subscribed: boolean;
  event_types: string[];
  aggregate_ids: string[];
}

interface EventRow {
  seq: number;
  event_id: string;
  aggregate_id: string;
  aggregate_version: number;
  event_type: string;
  payload: string;
  occurred_at: number;
  vector_clock: string;
  operation_id: string;
  correlation_id: string;
  causation_id: string | null;
  actor: string;
  organization_id: string;
  aggregate_type: string | null;
  lifecycle_epoch: number | null;
  authority_epoch: number | null;
}

function decodeJson(value: string | null | undefined, fallback: unknown): unknown {
  if (!value) return fallback;
  try { return JSON.parse(value); } catch { return fallback; }
}

function isoTime(value: number): string {
  const milliseconds = value < 1_000_000_000_000 ? value * 1000 : value;
  return new Date(milliseconds).toISOString();
}

/**
 * One Durable Object per organization owns the WebSocket fan-out for that
 * tenant. The Worker authenticates the access token and forwards the upgrade
 * with trusted internal tenant/user headers; browsers cannot address this
 * Durable Object namespace directly.
 */
export class EventHub {
  constructor(private readonly state: DurableObjectState, private readonly env: HubEnv) {}

  async fetch(request: Request): Promise<Response> {
    if (request.headers.get("upgrade")?.toLowerCase() !== "websocket") {
      return new Response("Not found", { status: 404 });
    }
    const organizationId = request.headers.get("x-onyx-authenticated-org");
    const userId = request.headers.get("x-onyx-authenticated-user");
    if (!organizationId || !userId) return new Response("Unauthorized", { status: 401 });

    const pair = new WebSocketPair();
    const client = pair[0];
    const server = pair[1];
    this.state.acceptWebSocket(server);
    await this.state.storage.put("organization_id", organizationId);
    server.serializeAttachment({
      organization_id: organizationId,
      user_id: userId,
      subscribed: false,
      event_types: [],
      aggregate_ids: [],
    } satisfies SocketAttachment);

    // Establish a cursor at the current tail on first use, so a new connection
    // receives live invalidation hints, not an unbounded historical replay.
    let cursor = await this.state.storage.get<number>("cursor");
    if (cursor === undefined) {
      const row = await this.env.DB.prepare("SELECT COALESCE(MAX(rowid), 0) AS seq FROM domain_events WHERE organization_id=?").bind(organizationId).first<{ seq: number }>();
      cursor = Number(row?.seq ?? 0);
      await this.state.storage.put("cursor", cursor);
    }
    return new Response(null, {
      status: 101,
      webSocket: client,
      headers: { "Sec-WebSocket-Protocol": "onyx-bearer" },
    });
  }

  async webSocketMessage(socket: WebSocket, message: string | ArrayBuffer): Promise<void> {
    if (typeof message !== "string" || message.length > 16_384) return;
    let value: Record<string, unknown>;
    try {
      const parsed: unknown = JSON.parse(message);
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return;
      value = parsed as Record<string, unknown>;
    } catch { return; }
    if (value.type !== "subscribe") return;

    const attachment = socket.deserializeAttachment() as SocketAttachment | null;
    if (!attachment?.organization_id || !attachment.user_id) {
      try { socket.close(1008, "unauthorized"); } catch { /* already closed */ }
      return;
    }
    const filter = value.filter && typeof value.filter === "object" && !Array.isArray(value.filter)
      ? value.filter as Record<string, unknown>
      : {};
    if (typeof filter.organization_id === "string" && filter.organization_id !== attachment.organization_id) {
      socket.send(JSON.stringify({ type: "error", code: "TENANT_MISMATCH" }));
      socket.close(1008, "tenant mismatch");
      return;
    }
    attachment.event_types = this.stringArray(filter.event_types);
    attachment.aggregate_ids = this.stringArray(filter.aggregate_ids);
    attachment.subscribed = true;
    socket.serializeAttachment(attachment);
    socket.send(JSON.stringify({ type: "subscribed" }));
    await this.state.storage.setAlarm(Date.now() + 1000);
  }

  async webSocketClose(socket: WebSocket): Promise<void> {
    try { socket.close(); } catch { /* already closed */ }
    if (this.state.getWebSockets().length > 0) {
      await this.state.storage.setAlarm(Date.now() + 1000);
    }
  }

  async webSocketError(): Promise<void> {
    if (this.state.getWebSockets().length > 0) {
      await this.state.storage.setAlarm(Date.now() + 1000);
    }
  }

  async alarm(): Promise<void> {
    if (this.state.getWebSockets().length === 0) return;
    try {
      const cursor = Number((await this.state.storage.get<number>("cursor")) ?? 0);
      const organizationId = await this.state.storage.get<string>("organization_id");
      if (!organizationId) return;
      const result = await this.env.DB.prepare(
        "SELECT e.rowid AS seq,e.event_id,e.aggregate_id,e.aggregate_version,e.event_type,e.payload,e.occurred_at,e.vector_clock,e.operation_id,e.correlation_id,e.causation_id,e.actor,e.organization_id,a.aggregate_type,a.lifecycle_epoch,a.authority_epoch FROM domain_events e LEFT JOIN aggregates a ON a.id=e.aggregate_id AND a.organization_id=e.organization_id WHERE e.organization_id=? AND e.rowid>? ORDER BY e.rowid ASC LIMIT 100"
      ).bind(organizationId, cursor).all<EventRow>();
      const rows = result.results ?? [];
      for (const row of rows) {
        const event = {
          event_id: row.event_id,
          event_type: row.event_type,
          schema_version: "1.0",
          aggregate_ref: {
            id: row.aggregate_id,
            type: row.aggregate_type ?? "unknown",
            organization_id: row.organization_id,
          },
          aggregate_version: Number(row.aggregate_version),
          lifecycle_epoch: Number(row.lifecycle_epoch ?? 0),
          authority_epoch: Number(row.authority_epoch ?? 0),
          operation_id: row.operation_id,
          actor: decodeJson(row.actor, {}),
          occurred_at: isoTime(Number(row.occurred_at)),
          recorded_at: isoTime(Number(row.occurred_at)),
          vector_clock: decodeJson(row.vector_clock, {}),
          correlation_id: row.correlation_id,
          causation_id: row.causation_id ?? null,
          payload: decodeJson(row.payload, {}),
        };
        for (const socket of this.state.getWebSockets()) {
          const attachment = socket.deserializeAttachment() as SocketAttachment | null;
          if (!attachment?.subscribed || attachment.organization_id !== row.organization_id) continue;
          if (attachment.event_types.length && !attachment.event_types.includes(row.event_type)) continue;
          if (attachment.aggregate_ids.length && !attachment.aggregate_ids.includes(row.aggregate_id)) continue;
          try { socket.send(JSON.stringify(event)); } catch { /* close callbacks clean up failed sockets */ }
        }
      }
      if (rows.length > 0) await this.state.storage.put("cursor", Number(rows[rows.length - 1].seq));
    } catch {
      // D1 interruptions must not leak provider details or kill active sockets.
    }
    if (this.state.getWebSockets().length > 0) {
      await this.state.storage.setAlarm(Date.now() + 1000);
    }
  }

  private stringArray(value: unknown): string[] {
    if (!Array.isArray(value)) return [];
    return value
      .filter((item): item is string => typeof item === "string" && item.length > 0 && item.length <= 256)
      .slice(0, 100);
  }
}
