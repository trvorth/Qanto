import { QantoWebSocket } from '../websocket';
import { NetworkHealth } from '../types';

describe('QantoWebSocket handleMessage for known types', () => {
  test('emits subscription_confirmed with subscription type', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);

    await new Promise<void>((resolve) => {
      ws.on('subscription_confirmed', (type: string) => {
        expect(type).toBe('blocks');
        resolve();
      });

      (ws as any).handleMessage({
        type: 'subscription_confirmation',
        data: { subscription_type: 'blocks' },
        timestamp: Date.now(),
      });
    });
  });

  test('emits balance_subscription_confirmed with address and client_id', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);

    await new Promise<void>((resolve) => {
      ws.on('balance_subscription_confirmed', (info: { address: string; client_id?: string }) => {
        expect(info.address).toBe('ADDRXYZ');
        expect(info.client_id).toBe('CID123');
        resolve();
      });

      (ws as any).handleMessage({
        type: 'balance_subscription_confirmed',
        address: 'ADDRXYZ',
        client_id: 'CID123',
        timestamp: Date.now(),
      });
    });
  });

  test('emits network_health with payload', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const payload: NetworkHealth = {
      block_count: 10,
      mempool_size: 5,
      utxo_count: 100,
      connected_peers: 8,
      sync_status: 'ok',
    };

    await new Promise<void>((resolve) => {
      ws.on('network_health', (health: NetworkHealth) => {
        expect(health.block_count).toBe(10);
        expect(health.connected_peers).toBe(8);
        resolve();
      });

      (ws as any).handleMessage({
        type: 'network_health',
        data: payload,
        timestamp: Date.now(),
      });
    });
  });

  test('emits analytics_update and mempool_update size correctly', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);

    const analyticsPromise = new Promise<void>((resolve) => {
      ws.on('analytics', (data: any) => {
        expect(data.metric).toBe('tps');
        expect(data.value).toBe(123);
        resolve();
      });
    });

    const mempoolPromise = new Promise<void>((resolve) => {
      ws.on('mempool_update', (size: number) => {
        expect(size).toBe(42);
        resolve();
      });
    });

    (ws as any).handleMessage({
      type: 'analytics_update',
      data: { metric: 'tps', value: 123 },
      timestamp: Date.now(),
    });
    (ws as any).handleMessage({
      type: 'mempool_update',
      data: { size: 42 },
      timestamp: Date.now(),
    });

    await Promise.all([analyticsPromise, mempoolPromise]);
  });

  test('emits security_alert with payload', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);

    await new Promise<void>((resolve) => {
      ws.on('security_alert', (alert: any) => {
        expect(alert.code).toBe('DOS');
        expect(alert.severity).toBe('high');
        resolve();
      });

      (ws as any).handleMessage({
        type: 'security_alert',
        data: { code: 'DOS', severity: 'high' },
        timestamp: Date.now(),
      });
    });
  });
});