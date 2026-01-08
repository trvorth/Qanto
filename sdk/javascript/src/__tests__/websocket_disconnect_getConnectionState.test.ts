import { QantoWebSocket } from '../websocket';
import WS from 'ws';

describe('QantoWebSocket disconnect and getConnectionState', () => {
  let clearIntervalSpy: jest.SpyInstance;

  beforeEach(() => {
    jest.useFakeTimers();
    clearIntervalSpy = jest.spyOn(global, 'clearInterval');
  });

  afterEach(() => {
    jest.useRealTimers();
    jest.restoreAllMocks();
  });

  test('disconnect stops heartbeat, closes socket, clears state', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const anyWs = ws as any;

    // Seed heartbeat and subscriptions
    anyWs.heartbeatInterval = setInterval(() => {}, 1000);
    anyWs.subscriptions.add('blocks');
    anyWs.reconnectAttempts = 2;

    // Attach fake OPEN socket with close spy
    const fakeSocket: any = { readyState: WS.OPEN, close: jest.fn() };
    anyWs.ws = fakeSocket;

    await ws.disconnect();

    // Heartbeat cleared
    expect(clearIntervalSpy).toHaveBeenCalled();
    expect(anyWs.heartbeatInterval).toBeNull();

    // Socket closed and nulled
    expect(fakeSocket.close).toHaveBeenCalledWith(1000, 'Manual disconnect');
    expect(anyWs.ws).toBeNull();

    // Subscriptions cleared and attempts reset
    expect(ws.getSubscriptions()).toEqual([]);
    expect(anyWs.reconnectAttempts).toBe(0);
  });

  test('getConnectionState maps ws.readyState and unknown', () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const anyWs = ws as any;

    // Null socket -> CLOSED
    expect(ws.getConnectionState()).toBe('CLOSED');

    // CONNECTING
    anyWs.ws = { readyState: WS.CONNECTING } as any;
    expect(ws.getConnectionState()).toBe('CONNECTING');

    // OPEN
    anyWs.ws = { readyState: WS.OPEN } as any;
    expect(ws.getConnectionState()).toBe('OPEN');

    // CLOSING
    anyWs.ws = { readyState: WS.CLOSING } as any;
    expect(ws.getConnectionState()).toBe('CLOSING');

    // CLOSED
    anyWs.ws = { readyState: WS.CLOSED } as any;
    expect(ws.getConnectionState()).toBe('CLOSED');

    // UNKNOWN
    anyWs.ws = { readyState: 99 } as any;
    expect(ws.getConnectionState()).toBe('UNKNOWN');
  });
});