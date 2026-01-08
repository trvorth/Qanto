import { QantoWebSocket } from '../websocket';
import WS from 'ws';

describe('QantoWebSocket heartbeat error handling', () => {
  beforeEach(() => {
    jest.useFakeTimers();
    jest.spyOn(global, 'setInterval');
  });

  afterEach(() => {
    jest.useRealTimers();
    jest.restoreAllMocks();
  });

  test('startHeartbeat logs error when ws.ping throws', () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const anyWs = ws as any;

    const consoleError = jest.spyOn(console, 'error').mockImplementation(() => {});

    anyWs.ws = {
      readyState: WS.OPEN,
      ping: jest.fn(() => {
        throw new Error('ping failed');
      }),
    } as any;

    anyWs.startHeartbeat();

    // Advance the heartbeat interval (30s)
    jest.advanceTimersByTime(30000);

    expect(consoleError).toHaveBeenCalled();

    // Cleanup
    anyWs.stopHeartbeat();
  });
});