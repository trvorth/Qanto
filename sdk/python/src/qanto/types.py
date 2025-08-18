"""Type definitions for the Qanto Network SDK."""

from datetime import datetime
from enum import Enum
from typing import Any, Dict, List, Optional, Union

from pydantic import BaseModel, Field


class Network(str, Enum):
    """Supported networks."""
    MAINNET = "mainnet"
    TESTNET = "testnet"
    LOCAL = "local"


class NETWORKS:
    """Network constants."""
    MAINNET = Network.MAINNET
    TESTNET = Network.TESTNET
    LOCAL = Network.LOCAL


class NetworkEndpoints(BaseModel):
    """Network endpoint configuration."""
    http: str
    websocket: str
    graphql: str


DEFAULT_ENDPOINTS = {
    Network.MAINNET: NetworkEndpoints(
        http="https://api.qanto.network",
        websocket="wss://ws.qanto.network",
        graphql="https://graphql.qanto.network"
    ),
    Network.TESTNET: NetworkEndpoints(
        http="https://testnet-api.qanto.network",
        websocket="wss://testnet-ws.qanto.network",
        graphql="https://testnet-graphql.qanto.network"
    ),
    Network.LOCAL: NetworkEndpoints(
        http="http://localhost:8080",
        websocket="ws://localhost:8081",
        graphql="http://localhost:8082/graphql"
    ),
}


# Core blockchain types
class TransactionInput(BaseModel):
    """Transaction input."""
    previous_output: Dict[str, Any]
    script_sig: str
    sequence: int


class TransactionOutput(BaseModel):
    """Transaction output."""
    value: int
    script_pub_key: str
    address: Optional[str] = None


class Transaction(BaseModel):
    """Blockchain transaction."""
    hash: str
    inputs: List[TransactionInput]
    outputs: List[TransactionOutput]
    lock_time: int
    version: int
    size: int
    fee: int
    confirmations: int
    timestamp: datetime
    block_hash: Optional[str] = None
    block_height: Optional[int] = None


class QantoBlock(BaseModel):
    """Blockchain block."""
    hash: str
    height: int
    timestamp: datetime
    previous_hash: str
    merkle_root: str
    nonce: int
    difficulty: float
    size: int
    transaction_count: int
    transactions: List[Transaction]
    miner: Optional[str] = None
    reward: int
    fees: int


class UTXO(BaseModel):
    """Unspent transaction output."""
    hash: str
    index: int
    value: int
    script_pub_key: str
    address: str
    confirmations: int
    spendable: bool


# Network and node information
class NodeInfo(BaseModel):
    """Blockchain node information."""
    version: str
    network: str
    block_height: int
    best_block_hash: str
    difficulty: float
    total_supply: int
    circulating_supply: int
    mempool_size: int
    connected_peers: int
    sync_progress: float
    is_testnet: bool


class NetworkHealth(BaseModel):
    """Network health metrics."""
    block_count: int
    mempool_size: int
    utxo_count: int
    difficulty: float
    hash_rate: float
    connected_peers: int
    network_hash_ps: float
    avg_block_time: float
    total_transactions: int


# Analytics and dashboard data
class NetworkHealthMetrics(BaseModel):
    """Network health metrics for analytics."""
    blocks_per_second: float
    transactions_per_second: float
    average_block_time: float
    network_hash_rate: float
    difficulty_adjustment: float
    mempool_congestion: float


class AnalyticsDashboardData(BaseModel):
    """Analytics dashboard data."""
    network_health: NetworkHealthMetrics
    recent_blocks: List[QantoBlock]
    recent_transactions: List[Transaction]
    mempool_stats: Dict[str, Any]
    network_stats: Dict[str, Any]
    performance_metrics: Dict[str, Any]


# WebSocket message types
class WebSocketMessageType(str, Enum):
    """WebSocket message types."""
    BLOCK_NOTIFICATION = "block_notification"
    TRANSACTION_CONFIRMATION = "transaction_confirmation"
    NETWORK_HEALTH = "network_health"
    ANALYTICS_UPDATE = "analytics_update"
    MEMPOOL_UPDATE = "mempool_update"
    SECURITY_ALERT = "security_alert"
    SUBSCRIPTION_CONFIRMATION = "subscription_confirmation"
    ERROR = "error"


class WebSocketMessage(BaseModel):
    """WebSocket message."""
    type: WebSocketMessageType
    data: Any
    timestamp: datetime = Field(default_factory=datetime.now)


class SubscriptionType(str, Enum):
    """WebSocket subscription types."""
    BLOCKS = "blocks"
    TRANSACTIONS = "transactions"
    NETWORK_HEALTH = "network_health"
    ANALYTICS = "analytics"
    SECURITY_ALERTS = "security_alerts"


class SubscriptionMessage(BaseModel):
    """WebSocket subscription message."""
    action: str  # "subscribe" or "unsubscribe"
    subscription_type: SubscriptionType
    filters: Optional[Dict[str, Any]] = None


# GraphQL types
class GraphQLSubscriptionOptions(BaseModel):
    """GraphQL subscription options."""
    on_data: Optional[Any] = None
    on_error: Optional[Any] = None
    on_complete: Optional[Any] = None


# Client configuration
class QantoClientConfig(BaseModel):
    """Qanto client configuration."""
    network: Network = Network.MAINNET
    http_endpoint: Optional[str] = None
    websocket_endpoint: Optional[str] = None
    graphql_endpoint: Optional[str] = None
    timeout: int = 30
    max_retries: int = 3
    retry_delay: float = 1.0
    headers: Optional[Dict[str, str]] = None


# API response types
class APIResponse(BaseModel):
    """Generic API response."""
    success: bool
    data: Optional[Any] = None
    error: Optional[str] = None
    timestamp: datetime = Field(default_factory=datetime.now)


class BalanceResponse(BaseModel):
    """Balance API response."""
    confirmed: int
    unconfirmed: int
    total: int


class MempoolResponse(BaseModel):
    """Mempool API response."""
    size: int
    bytes: int
    usage: int
    max_mempool: int
    transactions: List[str]


class SubmitTransactionResponse(BaseModel):
    """Submit transaction response."""
    hash: str
    success: bool
    message: Optional[str] = None


# Wallet and transaction building types
class WalletInfo(BaseModel):
    """Wallet information."""
    address: str
    balance: BalanceResponse
    utxos: List[UTXO]
    transaction_count: int


class TransactionBuilder(BaseModel):
    """Transaction builder configuration."""
    inputs: List[UTXO]
    outputs: List[TransactionOutput]
    fee_rate: float
    change_address: Optional[str] = None


class SignedTransaction(BaseModel):
    """Signed transaction."""
    hex: str
    hash: str
    size: int
    fee: int
    inputs: List[TransactionInput]
    outputs: List[TransactionOutput]