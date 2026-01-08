import { QantoWebSocket } from '../websocket';
import { WalletBalance, BalanceUpdate } from '../types';

describe('QantoWebSocket balance_update event', () => {
  test('emits update with WalletBalance schema', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const balance: WalletBalance = {
      spendable_confirmed: 100,
      immature_coinbase_confirmed: 20,
      unconfirmed_delta: -5,
      total_confirmed: 120,
    };

    const msg: any = {
      type: 'balance_update',
      address: 'QANTO_ADDR_TEST',
      balance,
      timestamp: Date.now(),
      finalized: true,
    };

    await new Promise<void>((resolve) => {
      ws.on('balance_update', (update: BalanceUpdate) => {
        expect(update.address).toBe('QANTO_ADDR_TEST');
        expect(update.finalized).toBe(true);
        expect(typeof update.timestamp).toBe('number');
        expect(update.balance.total_confirmed).toBe(120);
        expect(update.balance.spendable_confirmed).toBe(100);
        expect(update.balance.immature_coinbase_confirmed).toBe(20);
        expect(update.balance.unconfirmed_delta).toBe(-5);
        resolve();
      });

      // Call private handler via any-cast to avoid real network
      (ws as any).handleMessage(msg);
    });
  });
});