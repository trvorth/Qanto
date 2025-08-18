"""WebSocket client for the Qanto Network SDK."""

import asyncio
import json
import logging
from typing import Any, Callable, Dict, List, Optional, Set
from enum import Enum

import websockets
from websockets.exceptions import ConnectionClosed, WebSocketException

from .types import (
    QantoClientConfig, WebSocketMessage, WebSocketMessageType,
    SubscriptionType, SubscriptionMessage
)
from .exceptions import WebSocketError, SubscriptionError, NetworkError
from .utils import retry_async

# Configure logging
logger = logging.getLogger(__name__)


class ConnectionState(Enum):
    """WebSocket connection states."""
    DISCONNECTED = "disconnected"
    CONNECTING = "connecting"
    CONNECTED = "connected"
    RECONNECTING = "reconnecting"
    CLOSED = "closed"


class QantoWebSocket:
    """WebSocket client for real-time Qanto Network data."""
    
    def __init__(self, endpoint: str, config: Optional[QantoClientConfig] = None):
        """Initialize WebSocket client.
        
        Args:
            endpoint: WebSocket endpoint URL
            config: Client configuration
        """
        self.endpoint = endpoint
        self.config = config or QantoClientConfig()
        
        # Connection state
        self.state = ConnectionState.DISCONNECTED
        self.websocket: Optional[websockets.WebSocketServerProtocol] = None
        
        # Subscriptions
        self.subscriptions: Dict[str, SubscriptionMessage] = {}
        self.message_handlers: Dict[WebSocketMessageType, List[Callable]] = {}
        
        # Reconnection
        self.auto_reconnect = True
        self.reconnect_attempts = 0
        self.max_reconnect_attempts = 10
        self.reconnect_delay = 1.0
        self.reconnect_backoff = 2.0
        
        # Heartbeat
        self.heartbeat_interval = 30.0
        self.heartbeat_task: Optional[asyncio.Task] = None
        self.last_pong = 0
        
        # Tasks
        self.connection_task: Optional[asyncio.Task] = None
        self.message_task: Optional[asyncio.Task] = None
        
        logger.info(f"Initialized WebSocket client for {endpoint}")
    
    @property
    def is_connected(self) -> bool:
        """Check if WebSocket is connected.
        
        Returns:
            True if connected, False otherwise
        """
        return self.state == ConnectionState.CONNECTED and self.websocket is not None
    
    def on(self, message_type: WebSocketMessageType, handler: Callable[[Dict[str, Any]], None]):
        """Register message handler.
        
        Args:
            message_type: Type of message to handle
            handler: Handler function
        """
        if message_type not in self.message_handlers:
            self.message_handlers[message_type] = []
        self.message_handlers[message_type].append(handler)
        logger.debug(f"Registered handler for {message_type.value}")
    
    def off(self, message_type: WebSocketMessageType, handler: Optional[Callable] = None):
        """Unregister message handler.
        
        Args:
            message_type: Type of message
            handler: Specific handler to remove (if None, removes all)
        """
        if message_type in self.message_handlers:
            if handler is None:
                self.message_handlers[message_type].clear()
            else:
                try:
                    self.message_handlers[message_type].remove(handler)
                except ValueError:
                    pass
        logger.debug(f"Unregistered handler for {message_type.value}")
    
    async def connect(self) -> None:
        """Connect to WebSocket server.
        
        Raises:
            WebSocketError: If connection fails
        """
        if self.state in [ConnectionState.CONNECTED, ConnectionState.CONNECTING]:
            return
        
        self.state = ConnectionState.CONNECTING
        logger.info(f"Connecting to WebSocket: {self.endpoint}")
        
        try:
            # Connect with timeout
            self.websocket = await asyncio.wait_for(
                websockets.connect(
                    self.endpoint,
                    ping_interval=self.heartbeat_interval,
                    ping_timeout=10,
                    close_timeout=10
                ),
                timeout=self.config.timeout
            )
            
            self.state = ConnectionState.CONNECTED
            self.reconnect_attempts = 0
            
            # Start message handling task
            self.message_task = asyncio.create_task(self._handle_messages())
            
            # Resubscribe to existing subscriptions
            await self._resubscribe()
            
            # Emit connection event
            await self._emit_event(WebSocketMessageType.CONNECTION, {
                'type': 'connected',
                'endpoint': self.endpoint
            })
            
            logger.info("WebSocket connected successfully")
            
        except asyncio.TimeoutError:
            self.state = ConnectionState.DISCONNECTED
            raise WebSocketError(f"Connection timeout after {self.config.timeout}s")
        except Exception as e:
            self.state = ConnectionState.DISCONNECTED
            raise WebSocketError(f"Failed to connect: {e}")
    
    async def disconnect(self) -> None:
        """Disconnect from WebSocket server."""
        self.auto_reconnect = False
        self.state = ConnectionState.CLOSED
        
        # Cancel tasks
        if self.message_task:
            self.message_task.cancel()
            try:
                await self.message_task
            except asyncio.CancelledError:
                pass
        
        if self.heartbeat_task:
            self.heartbeat_task.cancel()
            try:
                await self.heartbeat_task
            except asyncio.CancelledError:
                pass
        
        # Close WebSocket
        if self.websocket:
            await self.websocket.close()
            self.websocket = None
        
        # Clear subscriptions
        self.subscriptions.clear()
        
        # Emit disconnection event
        await self._emit_event(WebSocketMessageType.CONNECTION, {
            'type': 'disconnected',
            'endpoint': self.endpoint
        })
        
        logger.info("WebSocket disconnected")
    
    def adisconnect(self) -> None:
        """Sync wrapper for disconnect (for compatibility)."""
        if asyncio.get_event_loop().is_running():
            asyncio.create_task(self.disconnect())
        else:
            asyncio.run(self.disconnect())
    
    async def subscribe(self, subscription_type: SubscriptionType, filters: Optional[Dict[str, Any]] = None) -> str:
        """Subscribe to real-time updates.
        
        Args:
            subscription_type: Type of subscription
            filters: Optional filters for the subscription
            
        Returns:
            Subscription ID
            
        Raises:
            SubscriptionError: If subscription fails
        """
        if not self.is_connected:
            await self.connect()
        
        subscription_id = f"{subscription_type.value}_{len(self.subscriptions)}"
        
        subscription = SubscriptionMessage(
            id=subscription_id,
            type=subscription_type,
            filters=filters
        )
        
        message = {
            'type': 'subscribe',
            'subscription': subscription.dict()
        }
        
        try:
            await self.websocket.send(json.dumps(message))
            self.subscriptions[subscription_id] = subscription
            logger.info(f"Subscribed to {subscription_type.value} with ID {subscription_id}")
            return subscription_id
            
        except Exception as e:
            raise SubscriptionError(f"Failed to subscribe to {subscription_type.value}: {e}", subscription_type.value)
    
    async def unsubscribe(self, subscription_id: str) -> None:
        """Unsubscribe from updates.
        
        Args:
            subscription_id: Subscription ID to unsubscribe from
            
        Raises:
            SubscriptionError: If unsubscription fails
        """
        if subscription_id not in self.subscriptions:
            raise SubscriptionError(f"Subscription {subscription_id} not found")
        
        if not self.is_connected:
            # Just remove from local subscriptions if not connected
            del self.subscriptions[subscription_id]
            return
        
        message = {
            'type': 'unsubscribe',
            'subscription_id': subscription_id
        }
        
        try:
            await self.websocket.send(json.dumps(message))
            del self.subscriptions[subscription_id]
            logger.info(f"Unsubscribed from {subscription_id}")
            
        except Exception as e:
            raise SubscriptionError(f"Failed to unsubscribe from {subscription_id}: {e}")
    
    async def subscribe_to_blocks(self, filters: Optional[Dict[str, Any]] = None) -> str:
        """Subscribe to new blocks.
        
        Args:
            filters: Optional filters
            
        Returns:
            Subscription ID
        """
        return await self.subscribe(SubscriptionType.BLOCKS, filters)
    
    async def subscribe_to_transactions(self, filters: Optional[Dict[str, Any]] = None) -> str:
        """Subscribe to new transactions.
        
        Args:
            filters: Optional filters (e.g., {'address': 'qanto...'})
            
        Returns:
            Subscription ID
        """
        return await self.subscribe(SubscriptionType.TRANSACTIONS, filters)
    
    async def subscribe_to_network_health(self) -> str:
        """Subscribe to network health updates.
        
        Returns:
            Subscription ID
        """
        return await self.subscribe(SubscriptionType.NETWORK_HEALTH)
    
    async def subscribe_to_analytics(self) -> str:
        """Subscribe to analytics updates.
        
        Returns:
            Subscription ID
        """
        return await self.subscribe(SubscriptionType.ANALYTICS)
    
    async def subscribe_to_security_alerts(self) -> str:
        """Subscribe to security alerts.
        
        Returns:
            Subscription ID
        """
        return await self.subscribe(SubscriptionType.SECURITY_ALERTS)
    
    async def _handle_messages(self) -> None:
        """Handle incoming WebSocket messages."""
        try:
            async for raw_message in self.websocket:
                try:
                    message_data = json.loads(raw_message)
                    message = WebSocketMessage(**message_data)
                    
                    # Handle different message types
                    if message.type == WebSocketMessageType.PING:
                        await self._handle_ping(message)
                    elif message.type == WebSocketMessageType.PONG:
                        await self._handle_pong(message)
                    else:
                        await self._emit_event(message.type, message.data)
                    
                except json.JSONDecodeError as e:
                    logger.error(f"Failed to parse WebSocket message: {e}")
                except Exception as e:
                    logger.error(f"Error handling WebSocket message: {e}")
                    
        except ConnectionClosed:
            logger.info("WebSocket connection closed")
            await self._handle_disconnection()
        except WebSocketException as e:
            logger.error(f"WebSocket error: {e}")
            await self._handle_disconnection()
        except Exception as e:
            logger.error(f"Unexpected error in message handler: {e}")
            await self._handle_disconnection()
    
    async def _handle_ping(self, message: WebSocketMessage) -> None:
        """Handle ping message.
        
        Args:
            message: Ping message
        """
        pong_message = {
            'type': 'pong',
            'data': message.data
        }
        await self.websocket.send(json.dumps(pong_message))
    
    async def _handle_pong(self, message: WebSocketMessage) -> None:
        """Handle pong message.
        
        Args:
            message: Pong message
        """
        import time
        self.last_pong = time.time()
    
    async def _handle_disconnection(self) -> None:
        """Handle WebSocket disconnection."""
        if self.state == ConnectionState.CLOSED:
            return
        
        self.state = ConnectionState.DISCONNECTED
        
        # Emit disconnection event
        await self._emit_event(WebSocketMessageType.CONNECTION, {
            'type': 'disconnected',
            'endpoint': self.endpoint,
            'will_reconnect': self.auto_reconnect
        })
        
        # Attempt reconnection if enabled
        if self.auto_reconnect and self.reconnect_attempts < self.max_reconnect_attempts:
            await self._reconnect()
    
    async def _reconnect(self) -> None:
        """Attempt to reconnect to WebSocket server."""
        self.state = ConnectionState.RECONNECTING
        self.reconnect_attempts += 1
        
        delay = self.reconnect_delay * (self.reconnect_backoff ** (self.reconnect_attempts - 1))
        delay = min(delay, 60)  # Cap at 60 seconds
        
        logger.info(f"Reconnecting in {delay}s (attempt {self.reconnect_attempts}/{self.max_reconnect_attempts})")
        
        await asyncio.sleep(delay)
        
        try:
            await self.connect()
        except Exception as e:
            logger.error(f"Reconnection attempt {self.reconnect_attempts} failed: {e}")
            
            if self.reconnect_attempts >= self.max_reconnect_attempts:
                logger.error("Max reconnection attempts reached, giving up")
                self.state = ConnectionState.CLOSED
                await self._emit_event(WebSocketMessageType.CONNECTION, {
                    'type': 'reconnect_failed',
                    'endpoint': self.endpoint,
                    'attempts': self.reconnect_attempts
                })
            else:
                # Try again
                await self._reconnect()
    
    async def _resubscribe(self) -> None:
        """Resubscribe to existing subscriptions after reconnection."""
        if not self.subscriptions:
            return
        
        logger.info(f"Resubscribing to {len(self.subscriptions)} subscriptions")
        
        # Store current subscriptions
        current_subscriptions = dict(self.subscriptions)
        self.subscriptions.clear()
        
        # Resubscribe to each
        for sub_id, subscription in current_subscriptions.items():
            try:
                await self.subscribe(subscription.type, subscription.filters)
            except Exception as e:
                logger.error(f"Failed to resubscribe to {sub_id}: {e}")
    
    async def _emit_event(self, message_type: WebSocketMessageType, data: Dict[str, Any]) -> None:
        """Emit event to registered handlers.
        
        Args:
            message_type: Type of message
            data: Message data
        """
        handlers = self.message_handlers.get(message_type, [])
        
        for handler in handlers:
            try:
                if asyncio.iscoroutinefunction(handler):
                    await handler(data)
                else:
                    handler(data)
            except Exception as e:
                logger.error(f"Error in message handler: {e}")
    
    def get_subscriptions(self) -> Dict[str, SubscriptionMessage]:
        """Get current subscriptions.
        
        Returns:
            Dictionary of current subscriptions
        """
        return dict(self.subscriptions)
    
    def get_connection_state(self) -> ConnectionState:
        """Get current connection state.
        
        Returns:
            Current connection state
        """
        return self.state
    
    async def send_message(self, message: Dict[str, Any]) -> None:
        """Send custom message to WebSocket server.
        
        Args:
            message: Message to send
            
        Raises:
            WebSocketError: If sending fails
        """
        if not self.is_connected:
            raise WebSocketError("WebSocket not connected")
        
        try:
            await self.websocket.send(json.dumps(message))
        except Exception as e:
            raise WebSocketError(f"Failed to send message: {e}")