import { Transaction, QantoBlock, UTXO } from './types';

/**
 * Validates a Qanto address format
 * @param address The address to validate
 * @returns True if the address is valid
 */
export function validateAddress(address: string): boolean {
  if (!address || typeof address !== 'string') {
    return false;
  }
  
  // Qanto addresses should be 64-character hexadecimal strings
  const addressRegex = /^[0-9a-fA-F]{64}$/;
  return addressRegex.test(address);
}

/**
 * Validates a transaction object
 * @param transaction The transaction to validate
 * @returns True if the transaction is valid
 */
export function validateTransaction(transaction: Transaction): boolean {
  if (!transaction || typeof transaction !== 'object') {
    return false;
  }
  
  // Check required fields
  if (!transaction.id || typeof transaction.id !== 'string') {
    return false;
  }
  
  if (!Array.isArray(transaction.inputs) || !Array.isArray(transaction.outputs)) {
    return false;
  }
  
  if (typeof transaction.timestamp !== 'number' || transaction.timestamp <= 0) {
    return false;
  }
  
  if (!transaction.signature || typeof transaction.signature !== 'string') {
    return false;
  }
  
  if (!transaction.public_key || typeof transaction.public_key !== 'string') {
    return false;
  }
  
  if (typeof transaction.fee !== 'number' || transaction.fee < 0) {
    return false;
  }
  
  if (typeof transaction.nonce !== 'number' || transaction.nonce < 0) {
    return false;
  }
  
  // Validate inputs
  for (const input of transaction.inputs) {
    if (!input.previous_output_id || typeof input.previous_output_id !== 'string') {
      return false;
    }
    if (!input.signature || typeof input.signature !== 'string') {
      return false;
    }
    if (!input.public_key || typeof input.public_key !== 'string') {
      return false;
    }
  }
  
  // Validate outputs
  for (const output of transaction.outputs) {
    if (!output.id || typeof output.id !== 'string') {
      return false;
    }
    if (!validateAddress(output.address)) {
      return false;
    }
    if (typeof output.amount !== 'number' || output.amount <= 0) {
      return false;
    }
  }
  
  return true;
}

/**
 * Validates a block object
 * @param block The block to validate
 * @returns True if the block is valid
 */
export function validateBlock(block: QantoBlock): boolean {
  if (!block || typeof block !== 'object') {
    return false;
  }
  
  // Check required fields
  if (!block.id || typeof block.id !== 'string') {
    return false;
  }
  
  if (!Array.isArray(block.parent_ids)) {
    return false;
  }
  
  if (typeof block.timestamp !== 'number' || block.timestamp <= 0) {
    return false;
  }
  
  if (typeof block.nonce !== 'number' || block.nonce < 0) {
    return false;
  }
  
  if (typeof block.difficulty !== 'number' || block.difficulty <= 0) {
    return false;
  }
  
  if (!Array.isArray(block.transactions)) {
    return false;
  }
  
  if (!validateAddress(block.miner_address)) {
    return false;
  }
  
  if (!block.merkle_root || typeof block.merkle_root !== 'string') {
    return false;
  }
  
  if (!block.signature || typeof block.signature !== 'string') {
    return false;
  }
  
  if (!block.block_hash || typeof block.block_hash !== 'string') {
    return false;
  }
  
  if (typeof block.height !== 'number' || block.height < 0) {
    return false;
  }
  
  // Validate all transactions in the block
  for (const transaction of block.transactions) {
    if (!validateTransaction(transaction)) {
      return false;
    }
  }
  
  return true;
}

/**
 * Validates a UTXO object
 * @param utxo The UTXO to validate
 * @returns True if the UTXO is valid
 */
export function validateUTXO(utxo: UTXO): boolean {
  if (!utxo || typeof utxo !== 'object') {
    return false;
  }
  
  if (!utxo.id || typeof utxo.id !== 'string') {
    return false;
  }
  
  if (!validateAddress(utxo.address)) {
    return false;
  }
  
  if (typeof utxo.amount !== 'number' || utxo.amount <= 0) {
    return false;
  }
  
  if (!utxo.transaction_id || typeof utxo.transaction_id !== 'string') {
    return false;
  }
  
  if (typeof utxo.output_index !== 'number' || utxo.output_index < 0) {
    return false;
  }
  
  if (!utxo.block_id || typeof utxo.block_id !== 'string') {
    return false;
  }
  
  if (typeof utxo.timestamp !== 'number' || utxo.timestamp <= 0) {
    return false;
  }
  
  return true;
}

/**
 * Converts a hex string to bytes
 * @param hex The hex string to convert
 * @returns Uint8Array of bytes
 */
export function hexToBytes(hex: string): Uint8Array {
  if (hex.length % 2 !== 0) {
    throw new Error('Hex string must have even length');
  }
  
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substr(i, 2), 16);
  }
  
  return bytes;
}

/**
 * Converts bytes to a hex string
 * @param bytes The bytes to convert
 * @returns Hex string representation
 */
export function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('');
}

/**
 * Formats a timestamp to a human-readable date string
 * @param timestamp Unix timestamp in seconds
 * @returns Formatted date string
 */
export function formatTimestamp(timestamp: number): string {
  return new Date(timestamp * 1000).toISOString();
}

/**
 * Formats an amount with proper decimal places
 * @param amount The amount in base units
 * @param decimals Number of decimal places (default: 8)
 * @returns Formatted amount string
 */
export function formatAmount(amount: number, decimals: number = 8): string {
  return (amount / Math.pow(10, decimals)).toFixed(decimals);
}

/**
 * Parses an amount string to base units
 * @param amountStr The amount string
 * @param decimals Number of decimal places (default: 8)
 * @returns Amount in base units
 */
export function parseAmount(amountStr: string, decimals: number = 8): number {
  const amount = parseFloat(amountStr);
  if (isNaN(amount)) {
    throw new Error('Invalid amount format');
  }
  return Math.round(amount * Math.pow(10, decimals));
}

/**
 * Calculates the hash of a string using a simple hash function
 * Note: This is a basic implementation for demonstration purposes
 * In production, you should use a proper cryptographic hash function
 * @param input The input string to hash
 * @returns Hash string
 */
export function simpleHash(input: string): string {
  let hash = 0;
  for (let i = 0; i < input.length; i++) {
    const char = input.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash = hash & hash; // Convert to 32-bit integer
  }
  return Math.abs(hash).toString(16);
}

/**
 * Delays execution for a specified number of milliseconds
 * @param ms Milliseconds to delay
 * @returns Promise that resolves after the delay
 */
export function delay(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * Retries a function with exponential backoff
 * @param fn The function to retry
 * @param maxAttempts Maximum number of attempts
 * @param baseDelay Base delay in milliseconds
 * @returns Promise that resolves with the function result
 */
export async function retryWithBackoff<T>(
  fn: () => Promise<T>,
  maxAttempts: number = 3,
  baseDelay: number = 1000
): Promise<T> {
  let lastError: Error;
  
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    try {
      return await fn();
    } catch (error) {
      lastError = error as Error;
      
      if (attempt < maxAttempts) {
        const delayMs = baseDelay * Math.pow(2, attempt - 1);
        await delay(delayMs);
      }
    }
  }
  
  throw lastError!;
}