import { QantoClient } from '../client';
import { ValidationError } from '../types';

describe('QantoClient validation', () => {
  let client: QantoClient;
  beforeEach(() => {
    client = new QantoClient({ network: 'local' });
  });

  test('getBlock throws ValidationError for empty blockId', async () => {
    await expect(client.getBlock('')).rejects.toBeInstanceOf(ValidationError);
  });

  test('getBalance throws ValidationError for invalid address', async () => {
    await expect(client.getBalance('invalid')).rejects.toBeInstanceOf(ValidationError);
  });

  test('submitTransaction throws ValidationError for invalid tx', async () => {
    // Missing required fields
    const badTx: any = { id: 'tx-bad', inputs: [], outputs: [], timestamp: -1 };
    await expect(client.submitTransaction(badTx)).rejects.toBeInstanceOf(ValidationError);
  });
});