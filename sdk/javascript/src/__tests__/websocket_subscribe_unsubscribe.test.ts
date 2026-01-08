import { QantoWebSocket } from '../websocket';
import WS from 'ws';

describe('QantoWebSocket subscribe/unsubscribe semantics', () => {
  test('subscribe connects when disconnected, sends payload, and tracks set', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const anyWs = ws as any;

    // Start disconnected
    anyWs.ws = null;

    // Stub connect to attach a fake OPEN socket
    jest.spyOn(ws, 'connect').mockImplementation(async () => {
      anyWs.ws = { readyState: WS.OPEN, send: jest.fn() } as any;
    });

    await ws.subscribe('blocks');

    // Verify send was called with serialized subscription message
    const fakeSocket = anyWs.ws as any;
    expect(fakeSocket.send).toHaveBeenCalledTimes(1);
    const arg = (fakeSocket.send as jest.Mock).mock.calls[0][0];
    const payload = JSON.parse(arg);
    expect(payload.action).toBe('subscribe');
    expect(payload.subscription_type).toBe('blocks');

    // Track subscription in set
    expect(ws.getSubscriptions()).toContain('blocks');
  });

  test('unsubscribe returns early when not connected', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const anyWs = ws as any;

    // Seed subscription and set CLOSED state
    anyWs.subscriptions.add('blocks');
    anyWs.ws = { readyState: WS.CLOSED, send: jest.fn() } as any;

    await ws.unsubscribe('blocks');

    // Should not send and should not remove from set
    expect((anyWs.ws as any).send).not.toHaveBeenCalled();
    expect(ws.getSubscriptions()).toContain('blocks');
  });

  test('subscribeToBalances sends alias topic and tracks address', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const anyWs = ws as any;
    anyWs.ws = { readyState: WS.OPEN, send: jest.fn() } as any;

    await ws.subscribeToBalances('ADDR1');
    const arg = (anyWs.ws.send as jest.Mock).mock.calls[0][0];
    const payload = JSON.parse(arg);
    expect(payload.type).toBe('subscribe');
    expect(payload.topics).toContain('wallet_balance:ADDR1');
    expect((ws as any).balanceSubscriptions.has('ADDR1')).toBe(true);
  });

  test('subscribeToBalances finalizedOnly uses filters', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const anyWs = ws as any;
    anyWs.ws = { readyState: WS.OPEN, send: jest.fn() } as any;

    await ws.subscribeToBalances('ADDR2', { finalizedOnly: true });
    const arg = (anyWs.ws.send as jest.Mock).mock.calls[0][0];
    const payload = JSON.parse(arg);
    expect(payload.type).toBe('subscribe');
    expect(payload.subscription_type).toBe('balances');
    expect(payload.filters).toEqual({ address: 'ADDR2', finalized_only: 'true' });
    expect((ws as any).balanceSubscriptions.has('ADDR2')).toBe(true);
  });

  test('subscribeToBlocks and subscribeToTransactions update subscriptions set', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const anyWs = ws as any;
    anyWs.ws = { readyState: WS.OPEN, send: jest.fn() } as any;

    await ws.subscribeToBlocks();
    await ws.subscribeToTransactions();

    const subs = ws.getSubscriptions();
    expect(subs).toContain('blocks');
    expect(subs).toContain('transactions');
  });
});