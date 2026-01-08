import { QantoClient } from '../client';

describe('QantoClient ping and waitForConnection', () => {
  test('ping returns true on /health success and false on failure', async () => {
    const client = new QantoClient({ network: 'local' });
    const anyClient = client as any;

    anyClient.httpClient.get = jest.fn().mockResolvedValue({ data: {} });
    await expect(client.ping()).resolves.toBe(true);

    anyClient.httpClient.get = jest.fn().mockRejectedValue(new Error('offline'));
    await expect(client.ping()).resolves.toBe(false);
  });

  test('waitForConnection resolves when ping eventually returns true', async () => {
    const client = new QantoClient({ network: 'local' });
    // Stub ping to be false then true
    (client as any).ping = jest.fn()
      .mockResolvedValueOnce(false)
      .mockResolvedValueOnce(true);

    const result = await client.waitForConnection(2000);
    expect(result).toBe(true);
    expect((client as any).ping).toHaveBeenCalledTimes(2);
  });
});