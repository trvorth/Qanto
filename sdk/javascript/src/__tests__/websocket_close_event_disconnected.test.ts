import { QantoWebSocket } from '../websocket';

// Mock the 'ws' module to emit open then close with code/reason
jest.mock('ws', () => {
  const listeners: Record<string, Function[]> = {};
  class FakeWS {
    public readyState = 0; // CONNECTING
    constructor(_url: string) {
      setImmediate(() => {
        (listeners['open'] || []).forEach(fn => fn());
        this.readyState = 1; // OPEN
        setImmediate(() => {
          (listeners['close'] || []).forEach(fn => fn(1006, 'server reset'));
          this.readyState = 3; // CLOSED
        });
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

describe('QantoWebSocket server close event', () => {
  test('emits disconnected with code and reason', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);

    const logSpy = jest.spyOn(console, 'log').mockImplementation(() => {});

    const disconnectedPromise = new Promise<void>((resolve) => {
      ws.on('disconnected', (code: number, reason: string) => {
        expect(code).toBe(1006);
        expect(reason).toBe('server reset');
        resolve();
      });
    });

    await ws.connect();

    await disconnectedPromise;

    logSpy.mockRestore();
    // Teardown
    await ws.disconnect();
  });
});