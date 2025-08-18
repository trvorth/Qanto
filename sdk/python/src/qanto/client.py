"""Main client for the Qanto Network SDK."""

import asyncio
import logging
from typing import Any, Dict, List, Optional, Union

import aiohttp
import requests
from pydantic import ValidationError as PydanticValidationError

from .types import (
    Network, DEFAULT_ENDPOINTS, QantoClientConfig, QantoBlock, Transaction,
    UTXO, NodeInfo, NetworkHealth, AnalyticsDashboardData, APIResponse,
    BalanceResponse, MempoolResponse, SubmitTransactionResponse
)
from .exceptions import (
    QantoError, NetworkError, APIError, ValidationError, TimeoutError,
    create_error_from_response
)
from .utils import (
    validate_qanto_address, validate_transaction, retry_sync, retry_async,
    RateLimiter
)
from .websocket import QantoWebSocket
from .graphql import QantoGraphQL

# Configure logging
logger = logging.getLogger(__name__)


class QantoClient:
    """Main client for interacting with the Qanto Network."""
    
    def __init__(self, config: Optional[QantoClientConfig] = None, network: Network = Network.MAINNET):
        """Initialize the Qanto client.
        
        Args:
            config: Client configuration
            network: Network to connect to
        """
        # Set default config if none provided
        if config is None:
            endpoints = DEFAULT_ENDPOINTS[network]
            config = QantoClientConfig(
                http_endpoint=endpoints.http,
                ws_endpoint=endpoints.websocket,
                graphql_endpoint=endpoints.graphql,
                network=network
            )
        
        self.config = config
        self.network = network
        
        # Initialize HTTP session
        self.session = requests.Session()
        self.session.headers.update({
            'User-Agent': f'Qanto-Python-SDK/1.0.0',
            'Content-Type': 'application/json',
            'Accept': 'application/json'
        })
        
        # Initialize async session (will be created when needed)
        self._async_session: Optional[aiohttp.ClientSession] = None
        
        # Initialize WebSocket and GraphQL clients
        self.websocket = QantoWebSocket(config.ws_endpoint, config)
        self.graphql = QantoGraphQL(config.graphql_endpoint, config)
        
        # Rate limiter
        self.rate_limiter = RateLimiter(
            max_requests=config.rate_limit_requests,
            time_window=config.rate_limit_window
        )
        
        logger.info(f"Initialized Qanto client for {network.value} network")
    
    def __enter__(self):
        """Context manager entry."""
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit."""
        self.close()
    
    async def __aenter__(self):
        """Async context manager entry."""
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.aclose()
    
    def close(self):
        """Close the client and cleanup resources."""
        if self.session:
            self.session.close()
        
        if self.websocket:
            self.websocket.disconnect()
    
    async def aclose(self):
        """Async close the client and cleanup resources."""
        if self._async_session:
            await self._async_session.close()
        
        if self.websocket:
            await self.websocket.adisconnect()
    
    @property
    def async_session(self) -> aiohttp.ClientSession:
        """Get or create async HTTP session."""
        if self._async_session is None or self._async_session.closed:
            timeout = aiohttp.ClientTimeout(total=self.config.timeout)
            self._async_session = aiohttp.ClientSession(
                timeout=timeout,
                headers={
                    'User-Agent': 'Qanto-Python-SDK/1.0.0',
                    'Content-Type': 'application/json',
                    'Accept': 'application/json'
                }
            )
        return self._async_session
    
    def _check_rate_limit(self):
        """Check rate limit before making request."""
        if not self.rate_limiter.is_allowed():
            wait_time = self.rate_limiter.time_until_allowed()
            raise APIError(f"Rate limit exceeded. Try again in {wait_time:.2f} seconds", status_code=429)
    
    def _make_request(self, method: str, endpoint: str, **kwargs) -> Dict[str, Any]:
        """Make HTTP request with error handling.
        
        Args:
            method: HTTP method
            endpoint: API endpoint
            **kwargs: Additional request arguments
            
        Returns:
            Response data
            
        Raises:
            QantoError: If request fails
        """
        self._check_rate_limit()
        
        url = f"{self.config.http_endpoint.rstrip('/')}/{endpoint.lstrip('/')}"
        
        def make_request():
            try:
                response = self.session.request(
                    method=method,
                    url=url,
                    timeout=self.config.timeout,
                    **kwargs
                )
                
                if response.status_code == 200:
                    return response.json()
                else:
                    error_data = None
                    try:
                        error_data = response.json()
                    except:
                        pass
                    
                    error_message = f"HTTP {response.status_code}: {response.reason}"
                    if error_data and 'message' in error_data:
                        error_message = error_data['message']
                    
                    raise create_error_from_response(response.status_code, error_message, error_data)
                    
            except requests.exceptions.Timeout:
                raise TimeoutError(f"Request timeout after {self.config.timeout}s")
            except requests.exceptions.ConnectionError as e:
                raise NetworkError(f"Connection error: {e}")
            except requests.exceptions.RequestException as e:
                raise NetworkError(f"Request error: {e}")
        
        return retry_sync(
            make_request,
            max_retries=self.config.max_retries,
            delay=1.0,
            exceptions=(NetworkError, TimeoutError)
        )
    
    async def _make_async_request(self, method: str, endpoint: str, **kwargs) -> Dict[str, Any]:
        """Make async HTTP request with error handling.
        
        Args:
            method: HTTP method
            endpoint: API endpoint
            **kwargs: Additional request arguments
            
        Returns:
            Response data
            
        Raises:
            QantoError: If request fails
        """
        self._check_rate_limit()
        
        url = f"{self.config.http_endpoint.rstrip('/')}/{endpoint.lstrip('/')}"
        
        async def make_request():
            try:
                async with self.async_session.request(
                    method=method,
                    url=url,
                    **kwargs
                ) as response:
                    if response.status == 200:
                        return await response.json()
                    else:
                        error_data = None
                        try:
                            error_data = await response.json()
                        except:
                            pass
                        
                        error_message = f"HTTP {response.status}: {response.reason}"
                        if error_data and 'message' in error_data:
                            error_message = error_data['message']
                        
                        raise create_error_from_response(response.status, error_message, error_data)
                        
            except asyncio.TimeoutError:
                raise TimeoutError(f"Request timeout after {self.config.timeout}s")
            except aiohttp.ClientConnectionError as e:
                raise NetworkError(f"Connection error: {e}")
            except aiohttp.ClientError as e:
                raise NetworkError(f"Request error: {e}")
        
        return await retry_async(
            make_request,
            max_retries=self.config.max_retries,
            delay=1.0,
            exceptions=(NetworkError, TimeoutError)
        )
    
    # Node Information
    def get_node_info(self) -> NodeInfo:
        """Get node information.
        
        Returns:
            Node information
        """
        data = self._make_request('GET', '/api/node/info')
        return NodeInfo(**data)
    
    async def aget_node_info(self) -> NodeInfo:
        """Get node information (async).
        
        Returns:
            Node information
        """
        data = await self._make_async_request('GET', '/api/node/info')
        return NodeInfo(**data)
    
    def get_network_health(self) -> NetworkHealth:
        """Get network health information.
        
        Returns:
            Network health data
        """
        data = self._make_request('GET', '/api/network/health')
        return NetworkHealth(**data)
    
    async def aget_network_health(self) -> NetworkHealth:
        """Get network health information (async).
        
        Returns:
            Network health data
        """
        data = await self._make_async_request('GET', '/api/network/health')
        return NetworkHealth(**data)
    
    # Blockchain Data
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
            endpoint = f'/api/blocks/{block_hash}'
        elif height is not None:
            endpoint = f'/api/blocks/height/{height}'
        else:
            raise ValidationError("Either block_hash or height must be provided")
        
        data = self._make_request('GET', endpoint)
        return QantoBlock(**data)
    
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
            endpoint = f'/api/blocks/{block_hash}'
        elif height is not None:
            endpoint = f'/api/blocks/height/{height}'
        else:
            raise ValidationError("Either block_hash or height must be provided")
        
        data = await self._make_async_request('GET', endpoint)
        return QantoBlock(**data)
    
    def get_latest_block(self) -> QantoBlock:
        """Get the latest block.
        
        Returns:
            Latest block data
        """
        data = self._make_request('GET', '/api/blocks/latest')
        return QantoBlock(**data)
    
    async def aget_latest_block(self) -> QantoBlock:
        """Get the latest block (async).
        
        Returns:
            Latest block data
        """
        data = await self._make_async_request('GET', '/api/blocks/latest')
        return QantoBlock(**data)
    
    def get_transaction(self, tx_hash: str) -> Transaction:
        """Get transaction by hash.
        
        Args:
            tx_hash: Transaction hash
            
        Returns:
            Transaction data
        """
        data = self._make_request('GET', f'/api/transactions/{tx_hash}')
        return Transaction(**data)
    
    async def aget_transaction(self, tx_hash: str) -> Transaction:
        """Get transaction by hash (async).
        
        Args:
            tx_hash: Transaction hash
            
        Returns:
            Transaction data
        """
        data = await self._make_async_request('GET', f'/api/transactions/{tx_hash}')
        return Transaction(**data)
    
    def get_balance(self, address: str) -> BalanceResponse:
        """Get balance for an address.
        
        Args:
            address: Qanto address
            
        Returns:
            Balance information
        """
        validate_qanto_address(address)
        data = self._make_request('GET', f'/api/addresses/{address}/balance')
        return BalanceResponse(**data)
    
    async def aget_balance(self, address: str) -> BalanceResponse:
        """Get balance for an address (async).
        
        Args:
            address: Qanto address
            
        Returns:
            Balance information
        """
        validate_qanto_address(address)
        data = await self._make_async_request('GET', f'/api/addresses/{address}/balance')
        return BalanceResponse(**data)
    
    def get_utxos(self, address: str) -> List[UTXO]:
        """Get UTXOs for an address.
        
        Args:
            address: Qanto address
            
        Returns:
            List of UTXOs
        """
        validate_qanto_address(address)
        data = self._make_request('GET', f'/api/addresses/{address}/utxos')
        return [UTXO(**utxo) for utxo in data.get('utxos', [])]
    
    async def aget_utxos(self, address: str) -> List[UTXO]:
        """Get UTXOs for an address (async).
        
        Args:
            address: Qanto address
            
        Returns:
            List of UTXOs
        """
        validate_qanto_address(address)
        data = await self._make_async_request('GET', f'/api/addresses/{address}/utxos')
        return [UTXO(**utxo) for utxo in data.get('utxos', [])]
    
    # Mempool
    def get_mempool_info(self) -> MempoolResponse:
        """Get mempool information.
        
        Returns:
            Mempool information
        """
        data = self._make_request('GET', '/api/mempool/info')
        return MempoolResponse(**data)
    
    async def aget_mempool_info(self) -> MempoolResponse:
        """Get mempool information (async).
        
        Returns:
            Mempool information
        """
        data = await self._make_async_request('GET', '/api/mempool/info')
        return MempoolResponse(**data)
    
    def get_mempool_transactions(self, limit: int = 50) -> List[Transaction]:
        """Get transactions from mempool.
        
        Args:
            limit: Maximum number of transactions to return
            
        Returns:
            List of mempool transactions
        """
        params = {'limit': limit}
        data = self._make_request('GET', '/api/mempool/transactions', params=params)
        return [Transaction(**tx) for tx in data.get('transactions', [])]
    
    async def aget_mempool_transactions(self, limit: int = 50) -> List[Transaction]:
        """Get transactions from mempool (async).
        
        Args:
            limit: Maximum number of transactions to return
            
        Returns:
            List of mempool transactions
        """
        params = {'limit': limit}
        data = await self._make_async_request('GET', '/api/mempool/transactions', params=params)
        return [Transaction(**tx) for tx in data.get('transactions', [])]
    
    # Transaction Submission
    def submit_transaction(self, transaction: Union[Dict[str, Any], Transaction]) -> SubmitTransactionResponse:
        """Submit a transaction to the network.
        
        Args:
            transaction: Transaction to submit
            
        Returns:
            Submission response
        """
        if isinstance(transaction, Transaction):
            tx_data = transaction.dict()
        else:
            validate_transaction(transaction)
            tx_data = transaction
        
        data = self._make_request('POST', '/api/transactions/submit', json=tx_data)
        return SubmitTransactionResponse(**data)
    
    async def asubmit_transaction(self, transaction: Union[Dict[str, Any], Transaction]) -> SubmitTransactionResponse:
        """Submit a transaction to the network (async).
        
        Args:
            transaction: Transaction to submit
            
        Returns:
            Submission response
        """
        if isinstance(transaction, Transaction):
            tx_data = transaction.dict()
        else:
            validate_transaction(transaction)
            tx_data = transaction
        
        data = await self._make_async_request('POST', '/api/transactions/submit', json=tx_data)
        return SubmitTransactionResponse(**data)
    
    # Analytics
    def get_analytics_dashboard(self) -> AnalyticsDashboardData:
        """Get analytics dashboard data.
        
        Returns:
            Analytics dashboard data
        """
        data = self._make_request('GET', '/api/analytics/dashboard')
        return AnalyticsDashboardData(**data)
    
    async def aget_analytics_dashboard(self) -> AnalyticsDashboardData:
        """Get analytics dashboard data (async).
        
        Returns:
            Analytics dashboard data
        """
        data = await self._make_async_request('GET', '/api/analytics/dashboard')
        return AnalyticsDashboardData(**data)
    
    # Utility Methods
    def ping(self) -> bool:
        """Ping the node to check connectivity.
        
        Returns:
            True if node is reachable
        """
        try:
            self._make_request('GET', '/api/ping')
            return True
        except QantoError:
            return False
    
    async def aping(self) -> bool:
        """Ping the node to check connectivity (async).
        
        Returns:
            True if node is reachable
        """
        try:
            await self._make_async_request('GET', '/api/ping')
            return True
        except QantoError:
            return False
    
    def get_version(self) -> str:
        """Get SDK version.
        
        Returns:
            SDK version string
        """
        return "1.0.0"
    
    def get_network(self) -> Network:
        """Get current network.
        
        Returns:
            Current network
        """
        return self.network
    
    def get_endpoints(self) -> Dict[str, str]:
        """Get current endpoints.
        
        Returns:
            Dictionary of endpoints
        """
        return {
            'http': self.config.http_endpoint,
            'websocket': self.config.ws_endpoint,
            'graphql': self.config.graphql_endpoint
        }