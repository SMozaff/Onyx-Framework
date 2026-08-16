import { afterEach, describe, expect, it, vi } from 'vitest';
import { EventStream } from '../../src/api/events';

class FakeWebSocket extends EventTarget {
  static instances: FakeWebSocket[] = [];
  sent: string[] = [];
  constructor(public readonly url: string) { super(); FakeWebSocket.instances.push(this); }
  send(value: string) { this.sent.push(value); }
  close() { this.dispatchEvent(new CloseEvent('close')); }
  open() { this.dispatchEvent(new Event('open')); }
  message(value: unknown) { this.dispatchEvent(new MessageEvent('message', { data: JSON.stringify(value) })); }
}

describe('WebSocket event stream', () => {
  afterEach(() => { vi.useRealTimers(); FakeWebSocket.instances = []; });
  it('authenticates in the URL and sends a subscription', () => {
    vi.stubGlobal('WebSocket', FakeWebSocket);
    const stream = new EventStream({ token: 'access token', filter: { organization_id: 'org' }, onEvent: vi.fn() });
    stream.connect(); const socket = FakeWebSocket.instances[0]; socket.open();
    expect(socket.url).toContain('token=access%20token');
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
