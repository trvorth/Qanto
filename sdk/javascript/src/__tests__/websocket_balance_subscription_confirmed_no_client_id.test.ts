import { QantoWebSocket } from '../websocket';

describe('QantoWebSocket balance_subscription_confirmed without client_id', () => {
  test('emits address and undefined client_id', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);

    await new Promise<void>((resolve) => {
      ws.on('balance_subscription_confirmed', (info: { address: string; client_id?: string }) => {
        expect(info.address).toBe('ADDR_NO_CID');
        expect(info.client_id).toBeUndefined();
        resolve();
      });

      (ws as any).handleMessage({ type: 'balance_subscription_confirmed', address: 'ADDR_NO_CID', timestamp: Date.now() });
    });
  });
});