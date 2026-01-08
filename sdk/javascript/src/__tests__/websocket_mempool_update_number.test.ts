import { QantoWebSocket } from '../websocket';

describe('QantoWebSocket mempool_update with numeric payload', () => {
  test('emits mempool_update with numeric data (fallback path)', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);

    await new Promise<void>((resolve) => {
      ws.on('mempool_update', (size: number) => {
        expect(size).toBe(99);
        resolve();
      });

      // Call private handler via any-cast to hit the numeric fallback path
      (ws as any).handleMessage({ type: 'mempool_update', data: 99, timestamp: Date.now() });
    });
  });
});