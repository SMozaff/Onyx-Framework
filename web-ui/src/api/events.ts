import type { DomainEventEnvelope, EventSubscriptionFilter } from '../types/events';
import { API_BASE } from './client';

/**
 * Realtime origin for the event stream.
 *
 * The WebSocket endpoint is served by the same Worker as the REST API, so derive
 * its origin from the resolved API base when `VITE_WS_BASE` is not set.
 * `VITE_API_BASE` already defaults to the Worker in production, but
 * `VITE_WS_BASE` had no production default, so a Pages build without it opened
 * `ws://127.0.0.1:3000` from an https:// origin and the browser blocked the
 * mixed-content connection, leaving the status badge stuck at "disconnected".
 */
export function realtimeBase(apiBase: string, configuredWsBase?: string): string {
  const configured = configuredWsBase?.trim();
  const base = configured ? configured : apiBase.replace(/^http/i, 'ws');
  return base.replace(/\/+$/, '');
}

const WS_BASE = realtimeBase(API_BASE, import.meta.env.VITE_WS_BASE);

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
    this.socket = new WebSocket(`${WS_BASE}/api/events`, ['onyx-bearer', this.options.token]);
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
