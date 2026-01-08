import { QantoWebSocket } from '../websocket';
import { WebSocketError } from '../types';

// Mock the 'ws' module to inject a fake WebSocket that emits a malformed message and then opens
jest.mock('ws', () => {
  const listeners: Record<string, Function[]> = {};
  class FakeWS {
    public readyState = 0; // CONNECTING
    constructor(_url: string) {
      // Emit a malformed 'message' first, then 'open'
      setImmediate(() => {
        (listeners['message'] || []).forEach(fn => fn(Buffer.from('not-json')));
        (listeners['open'] || []).forEach(fn => fn());
        this.readyState = 1; // OPEN
      });
    }
    on(event: string, cb: Function) {
      listeners[event] = listeners[event] || [];
      listeners[event].push(cb);
    }
    send(_data: any) {}
    ping() {}
    close(_code?: number, _reason?: string) { this.readyState = 3; }
    terminate() {}
    static OPEN = 1;
    static CONNECTING = 0;
    static CLOSING = 2;
    static CLOSED = 3;
  }
  return { __esModule: true, default: FakeWS };
});

describe('QantoWebSocket createConnection message parse error path', () => {
  test('emits error when JSON.parse fails on message but still connects', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);

    const errorPromise = new Promise<void>((resolve) => {
      ws.on('error', (err: Error) => {
        expect(err).toBeInstanceOf(WebSocketError);
        expect(err.message).toContain('Failed to parse message');
        resolve();
      });
    });

    const connectedPromise = new Promise<void>((resolve) => {
      ws.on('connected', () => resolve());
    });

    await ws.connect();
    await Promise.all([errorPromise, connectedPromise]);
    // Explicitly close the connection to avoid open handles.
    await ws.disconnect();
  });
});