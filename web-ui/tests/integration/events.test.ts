import { afterEach, describe, expect, it, vi } from 'vitest';
import { EventStream, realtimeBase } from '../../src/api/events';

class FakeWebSocket extends EventTarget {
  static instances: FakeWebSocket[] = [];
  sent: string[] = [];
  constructor(public readonly url: string, public readonly protocols: string | string[] = []) { super(); FakeWebSocket.instances.push(this); }
  send(value: string) { this.sent.push(value); }
  close() { this.dispatchEvent(new CloseEvent('close')); }
  open() { this.dispatchEvent(new Event('open')); }
  message(value: unknown) { this.dispatchEvent(new MessageEvent('message', { data: JSON.stringify(value) })); }
}

describe('WebSocket event stream', () => {
  afterEach(() => { vi.useRealTimers(); FakeWebSocket.instances = []; });
  it('authenticates with WebSocket subprotocols and sends a subscription', () => {
    vi.stubGlobal('WebSocket', FakeWebSocket);
    const stream = new EventStream({ token: 'access.token', filter: { organization_id: 'org' }, onEvent: vi.fn() });
    stream.connect(); const socket = FakeWebSocket.instances[0]; socket.open();
    expect(socket.url.endsWith('/api/events')).toBe(true);
    expect(socket.url).not.toContain('token=');
    expect(socket.protocols).toEqual(['onyx-bearer', 'access.token']);
    expect(JSON.parse(socket.sent[0])).toEqual({ type: 'subscribe', filter: { organization_id: 'org' } });
    stream.close();
  });
  it('delivers domain events and reconnects with exponential delay', () => {
    vi.useFakeTimers(); vi.stubGlobal('WebSocket', FakeWebSocket); const onEvent = vi.fn();
    const stream = new EventStream({ token: 'token', filter: { organization_id: 'org' }, onEvent });
    stream.connect(); const first = FakeWebSocket.instances[0]; first.open(); first.message({ event_id: '1', aggregate_ref: { type: 'mission' } });
    expect(onEvent).toHaveBeenCalledOnce(); first.close(); vi.advanceTimersByTime(999); expect(FakeWebSocket.instances).toHaveLength(1); vi.advanceTimersByTime(1); expect(FakeWebSocket.instances).toHaveLength(2); stream.close();
  });
});

describe('realtime base resolution', () => {
  it('derives wss from the production API base when VITE_WS_BASE is unset', () => {
    expect(realtimeBase('https://onyx-framework.soheil-mozaffari.workers.dev'))
      .toBe('wss://onyx-framework.soheil-mozaffari.workers.dev');
  });
  it('derives ws from the local API base for development', () => {
    expect(realtimeBase('http://127.0.0.1:3000')).toBe('ws://127.0.0.1:3000');
  });
  it('prefers an explicit VITE_WS_BASE, trimmed, without a trailing slash', () => {
    expect(realtimeBase('https://worker.example', '  ws://custom:1234/  ')).toBe('ws://custom:1234');
  });
  it('ignores a blank VITE_WS_BASE and strips a trailing slash from the derived base', () => {
    expect(realtimeBase('https://worker.example/', '   ')).toBe('wss://worker.example');
  });
});
