import type { DomainEventEnvelope, EventSubscriptionFilter } from '../types/events';

const WS_BASE = import.meta.env.VITE_WS_BASE ?? 'ws://127.0.0.1:3000';

export interface EventStreamOptions {
  token: string;
  filter: EventSubscriptionFilter;
  onEvent: (event: DomainEventEnvelope) => void;
  onResync?: () => void;
  onStatus?: (status: 'connecting' | 'connected' | 'disconnected') => void;
}

export class EventStream {
  private socket: WebSocket | null = null;
  private retryDelay = 1_000;
  private closedByUser = false;
  private reconnectTimer: number | null = null;

  constructor(private readonly options: EventStreamOptions) {}

  connect(): void {
    this.closedByUser = false;
    this.options.onStatus?.('connecting');
    this.socket = new WebSocket(`${WS_BASE}/api/events?token=${encodeURIComponent(this.options.token)}`);
    this.socket.addEventListener('open', () => {
      this.retryDelay = 1_000;
      this.options.onStatus?.('connected');
      this.socket?.send(JSON.stringify({ type: 'subscribe', filter: this.options.filter }));
    });
    this.socket.addEventListener('message', (message) => {
      try {
        const parsed = JSON.parse(String(message.data)) as DomainEventEnvelope | { type: string };
        if ('event_id' in parsed) this.options.onEvent(parsed);
        else if (parsed.type === 'resync_required') this.options.onResync?.();
      } catch {
        // Malformed provider messages are ignored; the projection query remains authoritative.
      }
    });
    this.socket.addEventListener('close', () => this.scheduleReconnect());
    this.socket.addEventListener('error', () => this.socket?.close());
  }

  close(): void {
    this.closedByUser = true;
    if (this.reconnectTimer !== null) window.clearTimeout(this.reconnectTimer);
    this.socket?.close();
    this.socket = null;
    this.options.onStatus?.('disconnected');
  }

  private scheduleReconnect(): void {
    this.options.onStatus?.('disconnected');
    if (this.closedByUser) return;
    const delay = this.retryDelay;
    this.retryDelay = Math.min(this.retryDelay * 2, 30_000);
    this.reconnectTimer = window.setTimeout(() => this.connect(), delay);
  }
}
