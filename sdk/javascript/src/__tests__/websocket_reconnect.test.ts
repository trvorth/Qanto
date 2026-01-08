import { QantoWebSocket } from '../websocket';
import WS from 'ws';

describe('QantoWebSocket reconnection and heartbeat', () => {
  let timeoutSpy: jest.SpyInstance;
  let clearIntervalSpy: jest.SpyInstance;

  beforeEach(() => {
    jest.useFakeTimers();
    timeoutSpy = jest.spyOn(global, 'setTimeout');
    clearIntervalSpy = jest.spyOn(global, 'clearInterval');
  });

  afterEach(() => {
    jest.useRealTimers();
    jest.restoreAllMocks();
  });

  test('scheduleReconnect uses exponential backoff and re-subscribes', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const anyWs = ws as any;

    // Seed existing subscriptions
    anyWs.subscriptions.add('blocks');
    anyWs.subscriptions.add('transactions');
    // Ensure the instance tracks a balance subscription via the public API
    // Provide a fake OPEN socket so send() succeeds inside subscribeToBalances
    anyWs.ws = { readyState: WS.OPEN, send: jest.fn() } as any;
    await ws.subscribeToBalances('ADDR1');

    // Stub connect + subscribe methods to avoid real network
    const connectSpy = jest.spyOn(ws, 'connect').mockResolvedValue(undefined);
    const subscribeSpy = jest.spyOn(ws as any, 'subscribe').mockResolvedValue(undefined);

    // Trigger reconnect scheduling (attempt 1)
    anyWs.scheduleReconnect();

    // Expect backoff delay of 1000ms (1st attempt)
    expect(timeoutSpy).toHaveBeenCalled();
    const delayArg = timeoutSpy.mock.calls[0][1];
    expect(delayArg).toBe(1000);

    // Fast-forward time to execute reconnect callback
    jest.advanceTimersByTime(1000);
    // Ensure pending timers fire and async tasks flush
    jest.runOnlyPendingTimers();
    await Promise.resolve();
    await Promise.resolve();

    // Verify connect and re-subscriptions were invoked
    expect(connectSpy).toHaveBeenCalledTimes(1);
    // Ensure both subscriptions were re-established
    const callArgs = subscribeSpy.mock.calls.map(c => c[0]);
    expect(callArgs).toContain('blocks');
    expect(callArgs).toContain('transactions');
    // Ensure balance alias subscription is re-established via send()
    const sendCalls = (anyWs.ws.send as jest.Mock).mock.calls.map(args => String(args[0]));
    expect(sendCalls.some(s => s.includes('wallet_balance:ADDR1'))).toBe(true);
  });

  test('scheduleReconnect logs error when connect fails', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const anyWs = ws as any;

    // Seed one subscription to exercise resubscribe loop path
    anyWs.subscriptions.add('blocks');

    // Stub connect to reject to hit error catch path
    jest.spyOn(ws, 'connect').mockRejectedValue(new Error('offline'));
    const errorSpy = jest.spyOn(console, 'error').mockImplementation(() => {});

    anyWs.scheduleReconnect();
    jest.advanceTimersByTime(1000);
    await Promise.resolve();

    expect(errorSpy).toHaveBeenCalled();
    errorSpy.mockRestore();
  });

  test('heartbeat pings when connected and stops cleanly', () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const anyWs = ws as any;

    // Inject minimal ws with OPEN state and ping spy
    const fakeSocket: any = { readyState: WS.OPEN, ping: jest.fn() };
    anyWs.ws = fakeSocket;

    // Start heartbeat and advance time
    anyWs.startHeartbeat();
    // Interval is 30000ms; advance twice
    jest.advanceTimersByTime(60000);
    expect(fakeSocket.ping).toHaveBeenCalledTimes(2);

    // Stop heartbeat and ensure interval cleared
    anyWs.stopHeartbeat();
    // Further advances should not call ping more
    jest.advanceTimersByTime(60000);
    expect(fakeSocket.ping).toHaveBeenCalledTimes(2);
    expect(clearIntervalSpy).toHaveBeenCalled();
  });
});