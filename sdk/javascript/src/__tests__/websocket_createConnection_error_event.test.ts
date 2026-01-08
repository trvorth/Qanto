import { QantoWebSocket } from '../websocket';
import { WebSocketError } from '../types';

// Mock the 'ws' module to inject a fake WebSocket that emits an 'error' on connect
jest.mock('ws', () => {
  const listeners: Record<string, Function[]> = {};
  class FakeWS {
    public readyState = 0; // CONNECTING
    constructor(_url: string) {
      // Emit an 'error' before any 'open' to simulate connection failure
      setImmediate(() => {
        (listeners['error'] || []).forEach(fn => fn(new Error('socket failure')));
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

describe('QantoWebSocket createConnection WebSocket error path', () => {
  test('emits error and connect rejects when underlying ws errors', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);

    const errorEventPromise = new Promise<void>((resolve) => {
      ws.on('error', (err: Error) => {
        expect(err).toBeInstanceOf(WebSocketError);
        expect(err.message).toContain('WebSocket error: socket failure');
        resolve();
      });
    });

    await expect(ws.connect()).rejects.toBeInstanceOf(WebSocketError);

    // Ensure teardown even if connect failed
    await ws.disconnect();

    await errorEventPromise;
  });
});