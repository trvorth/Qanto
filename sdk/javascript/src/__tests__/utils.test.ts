import {
  validateAddress,
  validateTransaction,
  validateBlock,
  validateUTXO,
  hexToBytes,
  bytesToHex,
  formatTimestamp,
  formatAmount,
  parseAmount,
  simpleHash,
  delay,
  retryWithBackoff
} from '../utils';
import { Transaction, QantoBlock, UTXO } from '../types';

describe('utils', () => {
  const validAddress = 'a'.repeat(64);

  test('validateAddress returns true for 64-char hex and false otherwise', () => {
    expect(validateAddress(validAddress)).toBe(true);
    expect(validateAddress('')).toBe(false);
    expect(validateAddress('zzzz')).toBe(false);
  });

  test('hexToBytes and bytesToHex roundtrip', () => {
    const hex = 'deadbeef00ff';
    const bytes = hexToBytes(hex);
    expect(bytes).toBeInstanceOf(Uint8Array);
    const roundtrip = bytesToHex(bytes);
    expect(roundtrip).toBe(hex);
  });

  test('formatTimestamp returns ISO string', () => {
    const ts = 1_700_000_000; // seconds
    const formatted = formatTimestamp(ts);
    expect(() => new Date(formatted)).not.toThrow();
    expect(formatted).toMatch(/T/);
  });

  test('formatAmount and parseAmount convert between display and base units', () => {
    const base = 123456789; // base units
    const display = formatAmount(base, 8);
    expect(display).toBe('1.23456789');
    const parsed = parseAmount(display, 8);
    expect(parsed).toBe(base);

    expect(() => parseAmount('not-a-number', 8)).toThrow('Invalid amount format');
  });

  test('simpleHash returns hex-like string', () => {
    const h = simpleHash('hello');
    expect(typeof h).toBe('string');
    expect(h).toMatch(/^[0-9a-f]+$/);
  });

  test('delay resolves after given milliseconds', async () => {
    const start = Date.now();
    await delay(5);
    const elapsed = Date.now() - start;
    expect(elapsed).toBeGreaterThanOrEqual(5);
  });

  test('retryWithBackoff succeeds after transient failures', async () => {
    let attempts = 0;
    const fn = async () => {
      attempts += 1;
      if (attempts < 3) throw new Error('transient');
      return 'ok';
    };
    const result = await retryWithBackoff(fn, 3, 1);
    expect(result).toBe('ok');
    expect(attempts).toBe(3);
  });

  test('retryWithBackoff throws after max attempts', async () => {
    const fn = async () => {
      throw new Error('always');
    };
    await expect(retryWithBackoff(fn, 2, 1)).rejects.toThrow('always');
  });

  test('validateTransaction returns true for valid transaction', () => {
    const tx: Transaction = {
      id: 'tx1',
      inputs: [{ previous_output_id: 'prev1', signature: 'sig', public_key: 'pk' }],
      outputs: [{ id: 'out1', address: validAddress, amount: 10 }],
      timestamp: 1_700_000_000,
      signature: 'sig',
      public_key: 'pk',
      fee: 1,
      nonce: 0
    };
    expect(validateTransaction(tx)).toBe(true);
  });

  test('validateTransaction returns false for invalid transaction', () => {
    const badTx: any = { id: '', inputs: [], outputs: [], timestamp: -1 };
    expect(validateTransaction(badTx as Transaction)).toBe(false);
  });

  test('validateBlock returns true for valid block', () => {
    const tx: Transaction = {
      id: 'tx1',
      inputs: [{ previous_output_id: 'prev1', signature: 'sig', public_key: 'pk' }],
      outputs: [{ id: 'out1', address: validAddress, amount: 10 }],
      timestamp: 1_700_000_000,
      signature: 'sig',
      public_key: 'pk',
      fee: 1,
      nonce: 0
    };
    const block: QantoBlock = {
      id: 'b1',
      parent_ids: ['p1'],
      timestamp: 1_700_000_000,
      nonce: 0,
      difficulty: 1,
      transactions: [tx],
      miner_address: validAddress,
      merkle_root: 'mr',
      signature: 'sig',
      block_hash: 'bh',
      height: 1
    };
    expect(validateBlock(block)).toBe(true);
  });

  test('validateBlock returns false for invalid block', () => {
    const badBlock: any = { id: '', parent_ids: 'oops', timestamp: -1 };
    expect(validateBlock(badBlock as QantoBlock)).toBe(false);
  });

  test('validateUTXO returns true for valid utxo and false for invalid', () => {
    const utxo: UTXO = {
      id: 'u1',
      address: validAddress,
      amount: 100,
      transaction_id: 'tx1',
      output_index: 0,
      block_id: 'b1',
      timestamp: 1_700_000_000
    };
    expect(validateUTXO(utxo)).toBe(true);

    const bad: any = { id: '', address: 'bad', amount: -1 };
    expect(validateUTXO(bad as UTXO)).toBe(false);
  });
});