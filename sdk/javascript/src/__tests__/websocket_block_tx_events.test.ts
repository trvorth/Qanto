import { QantoWebSocket } from '../websocket';
import { QantoBlock, Transaction } from '../types';

describe('QantoWebSocket block and transaction events', () => {
  test('emits block event for block_notification', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const block: QantoBlock = {
      id: 'B123',
      parent_ids: ['B122', 'B121'],
      height: 100,
      timestamp: Date.now(),
      weight: 42,
    } as any;

    await new Promise<void>((resolve) => {
      ws.on('block', (b: QantoBlock) => {
        expect(b.id).toBe('B123');
        expect(Array.isArray(b.parent_ids)).toBe(true);
        resolve();
      });

      (ws as any).handleMessage({ type: 'block_notification', data: block, timestamp: Date.now() });
    });
  });

  test('emits transaction event for transaction_confirmation', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const tx: Transaction = {
      id: 'T789',
      inputs: [],
      outputs: [],
      fee: 10,
    } as any;

    await new Promise<void>((resolve) => {
      ws.on('transaction', (t: Transaction) => {
        expect(t.id).toBe('T789');
        resolve();
      });

      (ws as any).handleMessage({ type: 'transaction_confirmation', data: tx, timestamp: Date.now() });
    });
  });
});