"""GraphQL client for the Qanto Network SDK."""

import asyncio
import json
import logging
from typing import Any, Dict, List, Optional, Union

import aiohttp
import requests
from gql import Client, gql
from gql.transport.aiohttp import AIOHTTPTransport
from gql.transport.requests import RequestsHTTPTransport
from gql.transport.websockets import WebsocketsTransport
from graphql import DocumentNode

from .types import (
    QantoClientConfig, QantoBlock, Transaction, UTXO, NodeInfo,
    NetworkHealth, AnalyticsDashboardData, GraphQLSubscriptionOptions
)
from .exceptions import GraphQLError, NetworkError, ValidationError
from .utils import retry_sync, retry_async, validate_qanto_address

# Configure logging
logger = logging.getLogger(__name__)


class QantoGraphQL:
    """GraphQL client for the Qanto Network."""
    
    def __init__(self, endpoint: str, config: Optional[QantoClientConfig] = None):
        """Initialize GraphQL client.
        
        Args:
            endpoint: GraphQL endpoint URL
            config: Client configuration
        """
        self.endpoint = endpoint
        self.config = config or QantoClientConfig()
        
        # Initialize sync client
        sync_transport = RequestsHTTPTransport(
            url=endpoint,
            timeout=self.config.timeout,
            headers={
                'User-Agent': 'Qanto-Python-SDK/1.0.0',
                'Content-Type': 'application/json'
            }
        )
        self.sync_client = Client(transport=sync_transport, fetch_schema_from_transport=False)
        
        # Initialize async client
        async_transport = AIOHTTPTransport(
            url=endpoint,
            timeout=self.config.timeout,
            headers={
                'User-Agent': 'Qanto-Python-SDK/1.0.0',
                'Content-Type': 'application/json'
            }
        )
        self.async_client = Client(transport=async_transport, fetch_schema_from_transport=False)
        
        # WebSocket transport for subscriptions
        ws_endpoint = endpoint.replace('http://', 'ws://').replace('https://', 'wss://')
        self.ws_transport = WebsocketsTransport(url=ws_endpoint)
        self.subscription_client = Client(transport=self.ws_transport, fetch_schema_from_transport=False)
        
        logger.info(f"Initialized GraphQL client for {endpoint}")
    
    def _execute_query(self, query: Union[str, DocumentNode], variables: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """Execute GraphQL query synchronously.
        
        Args:
            query: GraphQL query string or DocumentNode
            variables: Query variables
            
        Returns:
            Query result
            
        Raises:
            GraphQLError: If query execution fails
        """
        if isinstance(query, str):
            query = gql(query)
        
        def execute():
            try:
                result = self.sync_client.execute(query, variable_values=variables)
                return result
            except Exception as e:
                raise GraphQLError(f"GraphQL query failed: {e}", query=str(query), variables=variables)
        
        return retry_sync(
            execute,
            max_retries=self.config.max_retries,
            delay=1.0,
            exceptions=(NetworkError, GraphQLError)
        )
    
    async def _execute_async_query(self, query: Union[str, DocumentNode], variables: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """Execute GraphQL query asynchronously.
        
        Args:
            query: GraphQL query string or DocumentNode
            variables: Query variables
            
        Returns:
            Query result
            
        Raises:
            GraphQLError: If query execution fails
        """
        if isinstance(query, str):
            query = gql(query)
        
        async def execute():
            try:
                result = await self.async_client.execute_async(query, variable_values=variables)
                return result
            except Exception as e:
                raise GraphQLError(f"GraphQL query failed: {e}", query=str(query), variables=variables)
        
        return await retry_async(
            execute,
            max_retries=self.config.max_retries,
            delay=1.0,
            exceptions=(NetworkError, GraphQLError)
        )
    
    # Blockchain Information
    def get_blockchain_info(self) -> Dict[str, Any]:
        """Get blockchain information.
        
        Returns:
            Blockchain information
        """
        query = """
        query GetBlockchainInfo {
            blockchainInfo {
                height
                bestBlockHash
                difficulty
                totalSupply
                circulatingSupply
                networkHashRate
                averageBlockTime
                totalTransactions
            }
        }
        """
        
        result = self._execute_query(query)
        return result['blockchainInfo']
    
    async def aget_blockchain_info(self) -> Dict[str, Any]:
        """Get blockchain information (async).
        
        Returns:
            Blockchain information
        """
        query = """
        query GetBlockchainInfo {
            blockchainInfo {
                height
                bestBlockHash
                difficulty
                totalSupply
                circulatingSupply
                networkHashRate
                averageBlockTime
                totalTransactions
            }
        }
        """
        
        result = await self._execute_async_query(query)
        return result['blockchainInfo']
    
    # Blocks
    def get_block(self, block_hash: Optional[str] = None, height: Optional[int] = None) -> QantoBlock:
        """Get block by hash or height.
        
        Args:
            block_hash: Block hash
            height: Block height
            
        Returns:
            Block data
            
        Raises:
            ValidationError: If neither hash nor height provided
        """
        if block_hash:
            query = """
            query GetBlockByHash($hash: String!) {
                block(hash: $hash) {
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
                            previousHash
                            outputIndex
                            scriptSig
                            sequence
                        }
                        outputs {
                            address
                            amount
                            scriptPubKey
                        }
                        timestamp
                        fee
                        size
                    }
                }
            }
            """
            variables = {'hash': block_hash}
        elif height is not None:
            query = """
            query GetBlockByHeight($height: Int!) {
                block(height: $height) {
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
                            previousHash
                            outputIndex
                            scriptSig
                            sequence
                        }
                        outputs {
                            address
                            amount
                            scriptPubKey
                        }
                        timestamp
                        fee
                        size
                    }
                }
            }
            """
            variables = {'height': height}
        else:
            raise ValidationError("Either block_hash or height must be provided")
        
        result = self._execute_query(query, variables)
        return QantoBlock(**result['block'])
    
    async def aget_block(self, block_hash: Optional[str] = None, height: Optional[int] = None) -> QantoBlock:
        """Get block by hash or height (async).
        
        Args:
            block_hash: Block hash
            height: Block height
            
        Returns:
            Block data
            
        Raises:
            ValidationError: If neither hash nor height provided
        """
        if block_hash:
            query = """
            query GetBlockByHash($hash: String!) {
                block(hash: $hash) {
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
                            previousHash
                            outputIndex
                            scriptSig
                            sequence
                        }
                        outputs {
                            address
                            amount
                            scriptPubKey
                        }
                        timestamp
                        fee
                        size
                    }
                }
            }
            """
            variables = {'hash': block_hash}
        elif height is not None:
            query = """
            query GetBlockByHeight($height: Int!) {
                block(height: $height) {
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
                            previousHash
                            outputIndex
                            scriptSig
                            sequence
                        }
                        outputs {
                            address
                            amount
                            scriptPubKey
                        }
                        timestamp
                        fee
                        size
                    }
                }
            }
            """
            variables = {'height': height}
        else:
            raise ValidationError("Either block_hash or height must be provided")
        
        result = await self._execute_async_query(query, variables)
        return QantoBlock(**result['block'])
    
    def get_blocks(self, limit: int = 10, offset: int = 0) -> List[QantoBlock]:
        """Get multiple blocks.
        
        Args:
            limit: Maximum number of blocks to return
            offset: Number of blocks to skip
            
        Returns:
            List of blocks
        """
        query = """
        query GetBlocks($limit: Int!, $offset: Int!) {
            blocks(limit: $limit, offset: $offset) {
                hash
                height
                timestamp
                previousHash
                merkleRoot
                nonce
                difficulty
                size
                transactionCount
            }
        }
        """
        
        variables = {'limit': limit, 'offset': offset}
        result = self._execute_query(query, variables)
        return [QantoBlock(**block) for block in result['blocks']]
    
    async def aget_blocks(self, limit: int = 10, offset: int = 0) -> List[QantoBlock]:
        """Get multiple blocks (async).
        
        Args:
            limit: Maximum number of blocks to return
            offset: Number of blocks to skip
            
        Returns:
            List of blocks
        """
        query = """
        query GetBlocks($limit: Int!, $offset: Int!) {
            blocks(limit: $limit, offset: $offset) {
                hash
                height
                timestamp
                previousHash
                merkleRoot
                nonce
                difficulty
                size
                transactionCount
            }
        }
        """
        
        variables = {'limit': limit, 'offset': offset}
        result = await self._execute_async_query(query, variables)
        return [QantoBlock(**block) for block in result['blocks']]
    
    # Transactions
    def get_transaction(self, tx_hash: str) -> Transaction:
        """Get transaction by hash.
        
        Args:
            tx_hash: Transaction hash
            
        Returns:
            Transaction data
        """
        query = """
        query GetTransaction($hash: String!) {
            transaction(hash: $hash) {
                hash
                inputs {
                    previousHash
                    outputIndex
                    scriptSig
                    sequence
                }
                outputs {
                    address
                    amount
                    scriptPubKey
                }
                timestamp
                fee
                size
                blockHash
                blockHeight
                confirmations
            }
        }
        """
        
        variables = {'hash': tx_hash}
        result = self._execute_query(query, variables)
        return Transaction(**result['transaction'])
    
    async def aget_transaction(self, tx_hash: str) -> Transaction:
        """Get transaction by hash (async).
        
        Args:
            tx_hash: Transaction hash
            
        Returns:
            Transaction data
        """
        query = """
        query GetTransaction($hash: String!) {
            transaction(hash: $hash) {
                hash
                inputs {
                    previousHash
                    outputIndex
                    scriptSig
                    sequence
                }
                outputs {
                    address
                    amount
                    scriptPubKey
                }
                timestamp
                fee
                size
                blockHash
                blockHeight
                confirmations
            }
        }
        """
        
        variables = {'hash': tx_hash}
        result = await self._execute_async_query(query, variables)
        return Transaction(**result['transaction'])
    
    # Address Information
    def get_balance(self, address: str) -> Dict[str, Any]:
        """Get balance for an address.
        
        Args:
            address: Qanto address
            
        Returns:
            Balance information
        """
        validate_qanto_address(address)
        
        query = """
        query GetBalance($address: String!) {
            address(address: $address) {
                balance
                unconfirmedBalance
                totalReceived
                totalSent
                transactionCount
            }
        }
        """
        
        variables = {'address': address}
        result = self._execute_query(query, variables)
        return result['address']
    
    async def aget_balance(self, address: str) -> Dict[str, Any]:
        """Get balance for an address (async).
        
        Args:
            address: Qanto address
            
        Returns:
            Balance information
        """
        validate_qanto_address(address)
        
        query = """
        query GetBalance($address: String!) {
            address(address: $address) {
                balance
                unconfirmedBalance
                totalReceived
                totalSent
                transactionCount
            }
        }
        """
        
        variables = {'address': address}
        result = await self._execute_async_query(query, variables)
        return result['address']
    
    def get_utxos(self, address: str) -> List[UTXO]:
        """Get UTXOs for an address.
        
        Args:
            address: Qanto address
            
        Returns:
            List of UTXOs
        """
        validate_qanto_address(address)
        
        query = """
        query GetUTXOs($address: String!) {
            address(address: $address) {
                utxos {
                    transactionHash
                    outputIndex
                    address
                    amount
                    scriptPubKey
                    confirmations
                    spendable
                }
            }
        }
        """
        
        variables = {'address': address}
        result = self._execute_query(query, variables)
        return [UTXO(**utxo) for utxo in result['address']['utxos']]
    
    async def aget_utxos(self, address: str) -> List[UTXO]:
        """Get UTXOs for an address (async).
        
        Args:
            address: Qanto address
            
        Returns:
            List of UTXOs
        """
        validate_qanto_address(address)
        
        query = """
        query GetUTXOs($address: String!) {
            address(address: $address) {
                utxos {
                    transactionHash
                    outputIndex
                    address
                    amount
                    scriptPubKey
                    confirmations
                    spendable
                }
            }
        }
        """
        
        variables = {'address': address}
        result = await self._execute_async_query(query, variables)
        return [UTXO(**utxo) for utxo in result['address']['utxos']]
    
    # Mempool
    def get_mempool_info(self) -> Dict[str, Any]:
        """Get mempool information.
        
        Returns:
            Mempool information
        """
        query = """
        query GetMempoolInfo {
            mempool {
                size
                bytes
                usage
                maxMempool
                mempoolMinFee
                minRelayTxFee
            }
        }
        """
        
        result = self._execute_query(query)
        return result['mempool']
    
    async def aget_mempool_info(self) -> Dict[str, Any]:
        """Get mempool information (async).
        
        Returns:
            Mempool information
        """
        query = """
        query GetMempoolInfo {
            mempool {
                size
                bytes
                usage
                maxMempool
                mempoolMinFee
                minRelayTxFee
            }
        }
        """
        
        result = await self._execute_async_query(query)
        return result['mempool']
    
    # Network Statistics
    def get_network_stats(self) -> Dict[str, Any]:
        """Get network statistics.
        
        Returns:
            Network statistics
        """
        query = """
        query GetNetworkStats {
            networkStats {
                totalNodes
                activeNodes
                networkHashRate
                difficulty
                averageBlockTime
                transactionsPerSecond
                totalTransactions
                totalBlocks
            }
        }
        """
        
        result = self._execute_query(query)
        return result['networkStats']
    
    async def aget_network_stats(self) -> Dict[str, Any]:
        """Get network statistics (async).
        
        Returns:
            Network statistics
        """
        query = """
        query GetNetworkStats {
            networkStats {
                totalNodes
                activeNodes
                networkHashRate
                difficulty
                averageBlockTime
                transactionsPerSecond
                totalTransactions
                totalBlocks
            }
        }
        """
        
        result = await self._execute_async_query(query)
        return result['networkStats']
    
    # Mutations
    def submit_transaction(self, transaction: Dict[str, Any]) -> Dict[str, Any]:
        """Submit a transaction to the network.
        
        Args:
            transaction: Transaction data
            
        Returns:
            Submission result
        """
        mutation = """
        mutation SubmitTransaction($transaction: TransactionInput!) {
            submitTransaction(transaction: $transaction) {
                success
                transactionHash
                message
                error
            }
        }
        """
        
        variables = {'transaction': transaction}
        result = self._execute_query(mutation, variables)
        return result['submitTransaction']
    
    async def asubmit_transaction(self, transaction: Dict[str, Any]) -> Dict[str, Any]:
        """Submit a transaction to the network (async).
        
        Args:
            transaction: Transaction data
            
        Returns:
            Submission result
        """
        mutation = """
        mutation SubmitTransaction($transaction: TransactionInput!) {
            submitTransaction(transaction: $transaction) {
                success
                transactionHash
                message
                error
            }
        }
        """
        
        variables = {'transaction': transaction}
        result = await self._execute_async_query(mutation, variables)
        return result['submitTransaction']
    
    # Subscriptions (Note: These require WebSocket connection)
    async def subscribe_to_blocks(self, options: Optional[GraphQLSubscriptionOptions] = None) -> Any:
        """Subscribe to new blocks.
        
        Args:
            options: Subscription options
            
        Returns:
            Async iterator for block updates
            
        Note:
            This requires a WebSocket connection to the GraphQL endpoint.
        """
        subscription = """
        subscription BlockSubscription {
            blockAdded {
                hash
                height
                timestamp
                previousHash
                transactionCount
                size
            }
        }
        """
        
        try:
            async with self.subscription_client as session:
                async for result in session.subscribe(gql(subscription)):
                    yield result['blockAdded']
        except Exception as e:
            raise GraphQLError(f"Block subscription failed: {e}")
    
    async def subscribe_to_transactions(self, address: Optional[str] = None, options: Optional[GraphQLSubscriptionOptions] = None) -> Any:
        """Subscribe to new transactions.
        
        Args:
            address: Optional address filter
            options: Subscription options
            
        Returns:
            Async iterator for transaction updates
            
        Note:
            This requires a WebSocket connection to the GraphQL endpoint.
        """
        if address:
            validate_qanto_address(address)
            subscription = """
            subscription TransactionSubscription($address: String!) {
                transactionAdded(address: $address) {
                    hash
                    inputs {
                        previousHash
                        outputIndex
                    }
                    outputs {
                        address
                        amount
                    }
                    timestamp
                    fee
                }
            }
            """
            variables = {'address': address}
        else:
            subscription = """
            subscription TransactionSubscription {
                transactionAdded {
                    hash
                    inputs {
                        previousHash
                        outputIndex
                    }
                    outputs {
                        address
                        amount
                    }
                    timestamp
                    fee
                }
            }
            """
            variables = None
        
        try:
            async with self.subscription_client as session:
                async for result in session.subscribe(gql(subscription), variable_values=variables):
                    yield result['transactionAdded']
        except Exception as e:
            raise GraphQLError(f"Transaction subscription failed: {e}")
    
    def close(self) -> None:
        """Close the GraphQL client."""
        # Close sync client
        if hasattr(self.sync_client.transport, 'close'):
            self.sync_client.transport.close()
        
        logger.info("GraphQL client closed")
    
    async def aclose(self) -> None:
        """Close the GraphQL client (async)."""
        # Close async client
        if hasattr(self.async_client.transport, 'close'):
            await self.async_client.transport.close()
        
        # Close subscription client
        if hasattr(self.ws_transport, 'close'):
            await self.ws_transport.close()
        
        logger.info("GraphQL client closed")