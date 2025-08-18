import { GraphQLClient, gql } from 'graphql-request';
import {
  QantoBlock,
  Transaction,
  NodeInfo,
  NetworkHealth,
  AnalyticsDashboardData,
  GraphQLError
} from './types';

export interface GraphQLSubscriptionOptions {
  onData?: (data: any) => void;
  onError?: (error: Error) => void;
  onComplete?: () => void;
}

export class QantoGraphQL {
  private client: GraphQLClient;
  private subscriptions = new Map<string, any>();

  constructor(endpoint: string, headers?: Record<string, string>) {
    this.client = new GraphQLClient(endpoint, {
      headers: headers || {}
    });
  }

  /**
   * Get blockchain information
   */
  async getBlockchainInfo(): Promise<NodeInfo> {
    const query = gql`
      query GetBlockchainInfo {
        blockchainInfo {
          version
          network
          blockHeight
          bestBlockHash
          difficulty
          totalSupply
          circulatingSupply
          mempoolSize
          connectedPeers
          syncProgress
          isTestnet
        }
      }
    `;

    try {
      const data = await this.client.request(query);
      return data.blockchainInfo;
    } catch (error) {
      throw new GraphQLError(`Failed to get blockchain info: ${(error as Error).message}`);
    }
  }

  /**
   * Get block by hash or height
   */
  async getBlock(hashOrHeight: string | number): Promise<QantoBlock> {
    const query = gql`
      query GetBlock($identifier: String!) {
        block(identifier: $identifier) {
          hash
          height
          timestamp
          previousHash
          merkleRoot
          nonce
          difficulty
          size
          transactionCount
          transactions {
            hash
            inputs {
              previousOutput {
                hash
                index
              }
              scriptSig
              sequence
            }
            outputs {
              value
              scriptPubKey
              address
            }
            lockTime
            version
            size
            fee
            confirmations
            timestamp
          }
          miner
          reward
          fees
        }
      }
    `;

    try {
      const data = await this.client.request(query, {
        identifier: hashOrHeight.toString()
      });
      return data.block;
    } catch (error) {
      throw new GraphQLError(`Failed to get block: ${(error as Error).message}`);
    }
  }

  /**
   * Get transaction by hash
   */
  async getTransaction(hash: string): Promise<Transaction> {
    const query = gql`
      query GetTransaction($hash: String!) {
        transaction(hash: $hash) {
          hash
          inputs {
            previousOutput {
              hash
              index
            }
            scriptSig
            sequence
          }
          outputs {
            value
            scriptPubKey
            address
          }
          lockTime
          version
          size
          fee
          confirmations
          timestamp
          blockHash
          blockHeight
        }
      }
    `;

    try {
      const data = await this.client.request(query, { hash });
      return data.transaction;
    } catch (error) {
      throw new GraphQLError(`Failed to get transaction: ${(error as Error).message}`);
    }
  }

  /**
   * Get balance for an address
   */
  async getBalance(address: string): Promise<{ confirmed: number; unconfirmed: number; total: number }> {
    const query = gql`
      query GetBalance($address: String!) {
        balance(address: $address) {
          confirmed
          unconfirmed
          total
        }
      }
    `;

    try {
      const data = await this.client.request(query, { address });
      return data.balance;
    } catch (error) {
      throw new GraphQLError(`Failed to get balance: ${(error as Error).message}`);
    }
  }

  /**
   * Get mempool information
   */
  async getMempoolInfo(): Promise<{ size: number; bytes: number; usage: number; maxMempool: number }> {
    const query = gql`
      query GetMempoolInfo {
        mempoolInfo {
          size
          bytes
          usage
          maxMempool
        }
      }
    `;

    try {
      const data = await this.client.request(query);
      return data.mempoolInfo;
    } catch (error) {
      throw new GraphQLError(`Failed to get mempool info: ${(error as Error).message}`);
    }
  }

  /**
   * Get network statistics
   */
  async getNetworkStats(): Promise<NetworkHealth> {
    const query = gql`
      query GetNetworkStats {
        networkStats {
          blockCount
          mempoolSize
          utxoCount
          difficulty
          hashRate
          connectedPeers
          networkHashPs
          avgBlockTime
          totalTransactions
        }
      }
    `;

    try {
      const data = await this.client.request(query);
      return data.networkStats;
    } catch (error) {
      throw new GraphQLError(`Failed to get network stats: ${(error as Error).message}`);
    }
  }

  /**
   * Submit a transaction
   */
  async submitTransaction(transactionHex: string): Promise<{ hash: string; success: boolean }> {
    const mutation = gql`
      mutation SubmitTransaction($transactionHex: String!) {
        submitTransaction(transactionHex: $transactionHex) {
          hash
          success
        }
      }
    `;

    try {
      const data = await this.client.request(mutation, { transactionHex });
      return data.submitTransaction;
    } catch (error) {
      throw new GraphQLError(`Failed to submit transaction: ${(error as Error).message}`);
    }
  }

  /**
   * Subscribe to new blocks
   */
  async subscribeToBlocks(options: GraphQLSubscriptionOptions = {}): Promise<string> {
    const subscription = gql`
      subscription NewBlocks {
        newBlock {
          hash
          height
          timestamp
          previousHash
          merkleRoot
          nonce
          difficulty
          size
          transactionCount
          miner
          reward
          fees
        }
      }
    `;

    const subscriptionId = `blocks_${Date.now()}`;
    
    try {
      // Note: This is a simplified implementation
      // In a real implementation, you'd need a WebSocket-based GraphQL client
      // for subscriptions like graphql-ws or similar
      console.warn('GraphQL subscriptions require WebSocket support. Use WebSocket client for real-time updates.');
      
      this.subscriptions.set(subscriptionId, {
        query: subscription,
        options
      });
      
      return subscriptionId;
    } catch (error) {
      throw new GraphQLError(`Failed to subscribe to blocks: ${(error as Error).message}`);
    }
  }

  /**
   * Subscribe to new transactions
   */
  async subscribeToTransactions(options: GraphQLSubscriptionOptions = {}): Promise<string> {
    const subscription = gql`
      subscription NewTransactions {
        newTransaction {
          hash
          inputs {
            previousOutput {
              hash
              index
            }
            scriptSig
            sequence
          }
          outputs {
            value
            scriptPubKey
            address
          }
          lockTime
          version
          size
          fee
          timestamp
        }
      }
    `;

    const subscriptionId = `transactions_${Date.now()}`;
    
    try {
      console.warn('GraphQL subscriptions require WebSocket support. Use WebSocket client for real-time updates.');
      
      this.subscriptions.set(subscriptionId, {
        query: subscription,
        options
      });
      
      return subscriptionId;
    } catch (error) {
      throw new GraphQLError(`Failed to subscribe to transactions: ${(error as Error).message}`);
    }
  }

  /**
   * Unsubscribe from a subscription
   */
  async unsubscribe(subscriptionId: string): Promise<void> {
    if (this.subscriptions.has(subscriptionId)) {
      this.subscriptions.delete(subscriptionId);
    }
  }

  /**
   * Get all active subscriptions
   */
  getActiveSubscriptions(): string[] {
    return Array.from(this.subscriptions.keys());
  }

  /**
   * Execute a custom GraphQL query
   */
  async query<T = any>(query: string, variables?: Record<string, any>): Promise<T> {
    try {
      return await this.client.request(query, variables);
    } catch (error) {
      throw new GraphQLError(`GraphQL query failed: ${(error as Error).message}`);
    }
  }

  /**
   * Execute a custom GraphQL mutation
   */
  async mutate<T = any>(mutation: string, variables?: Record<string, any>): Promise<T> {
    try {
      return await this.client.request(mutation, variables);
    } catch (error) {
      throw new GraphQLError(`GraphQL mutation failed: ${(error as Error).message}`);
    }
  }

  /**
   * Set request headers
   */
  setHeaders(headers: Record<string, string>): void {
    this.client.setHeaders(headers);
  }

  /**
   * Set a single header
   */
  setHeader(key: string, value: string): void {
    this.client.setHeader(key, value);
  }
}