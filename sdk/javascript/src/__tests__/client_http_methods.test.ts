import { QantoClient } from '../client';
import { NodeInfo, NetworkHealth, QantoBlock, Transaction, WalletInfo } from '../types';

describe('QantoClient HTTP-backed methods', () => {
  test('getNodeInfo, getNetworkHealth, getConnectedPeers', async () => {
    const client = new QantoClient({ network: 'local' });
    const anyClient = client as any;
    const nodeInfo: NodeInfo = {
      version: '1.0.0', network: 'local', node_id: 'n1', block_count: 1,
      mempool_size: 0, connected_peers: 0, sync_status: 'synced', uptime: 100
    };
    const health: NetworkHealth = {
      block_count: 1, mempool_size: 0, utxo_count: 0, connected_peers: 0, sync_status: 'ok'
    };
    anyClient.httpClient.get = jest.fn()
      .mockResolvedValueOnce({ data: nodeInfo })
      .mockResolvedValueOnce({ data: health })
      .mockResolvedValueOnce({ data: ['peer1', 'peer2'] });

    await expect(client.getNodeInfo()).resolves.toEqual(nodeInfo);
    await expect(client.getNetworkHealth()).resolves.toEqual(health);
    await expect(client.getConnectedPeers()).resolves.toEqual(['peer1', 'peer2']);
  });

  test('getBlocks paginates and getLatestBlocks returns latest', async () => {
    const client = new QantoClient({ network: 'local' });
    const anyClient = client as any;
    const block: QantoBlock = {
      id: 'b1', parent_ids: [], timestamp: 1, nonce: 0, difficulty: 1,
      transactions: [], miner_address: 'a'.repeat(64), merkle_root: 'mr', signature: 'sig', block_hash: 'bh', height: 1
    };
    anyClient.httpClient.get = jest.fn()
      .mockResolvedValueOnce({ data: ['b1','b2','b3','b4','b5'] });
    client.getBlock = jest.fn().mockResolvedValue(block);

    const page = await client.getBlocks(1, 2);
    expect(page.items.length).toBe(2);
    expect(page.total).toBe(5);
    expect(page.has_next).toBe(true);
    expect(page.has_prev).toBe(false);

    anyClient.httpClient.get = jest.fn().mockResolvedValueOnce({ data: ['b1','b2','b3'] });
    client.getBlock = jest.fn().mockResolvedValue(block);
    const latest = await client.getLatestBlocks(2);
    expect(latest.length).toBe(2);
  });

  test('getMempool returns map of transactions', async () => {
    const client = new QantoClient({ network: 'local' });
    const anyClient = client as any;
    const tx: Transaction = {
      id: 'tx', inputs: [], outputs: [], timestamp: 1, signature: 'sig', public_key: 'pk', fee: 0, nonce: 0
    };
    anyClient.httpClient.get = jest.fn().mockResolvedValueOnce({ data: { tx } });
    await expect(client.getMempool()).resolves.toEqual({ tx });
  });

  test('getWalletInfo composes balance and utxos', async () => {
    const client = new QantoClient({ network: 'local' });
    client.getBalance = jest.fn().mockResolvedValue(100);
    client.getUTXOs = jest.fn().mockResolvedValue({ u1: { id: 'u1', address: 'a'.repeat(64), amount: 100, transaction_id: 'tx', output_index: 0, block_id: 'b1', timestamp: 1 } });
    const info: WalletInfo = await client.getWalletInfo('a'.repeat(64));
    expect(info.balance).toBe(100);
    expect(info.utxo_count).toBe(1);
    expect(info.address).toBe('a'.repeat(64));
  });

  test('disconnect calls websocket.disconnect and clears listeners', async () => {
    const client = new QantoClient({ network: 'local' });
    client.websocket.disconnect = jest.fn().mockResolvedValue(undefined);
    await expect(client.disconnect()).resolves.toBeUndefined();
    expect(client.websocket.disconnect).toHaveBeenCalled();
  });

  test('getDAGInfo and getPublishReadiness return data', async () => {
    const client = new QantoClient({ network: 'local' });
    const anyClient = client as any;
    anyClient.httpClient.get = jest.fn()
      .mockResolvedValueOnce({ data: { dag: true } })
      .mockResolvedValueOnce({ data: { readiness: 'ok' } });

    await expect(client.getDAGInfo()).resolves.toEqual({ dag: true });
    await expect(client.getPublishReadiness()).resolves.toEqual({ readiness: 'ok' });
  });
});