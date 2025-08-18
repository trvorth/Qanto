"""Exception classes for the Qanto Network SDK."""

from typing import Any, Dict, Optional


class QantoError(Exception):
    """Base exception for Qanto SDK errors."""
    
    def __init__(self, message: str, code: Optional[int] = None, details: Optional[Dict[str, Any]] = None):
        super().__init__(message)
        self.message = message
        self.code = code
        self.details = details or {}
    
    def __str__(self) -> str:
        if self.code:
            return f"[{self.code}] {self.message}"
        return self.message
    
    def __repr__(self) -> str:
        return f"{self.__class__.__name__}(message='{self.message}', code={self.code})"


class NetworkError(QantoError):
    """Network-related errors (connection, timeout, etc.)."""
    pass


class APIError(QantoError):
    """API-related errors (HTTP errors, invalid responses, etc.)."""
    
    def __init__(self, message: str, status_code: Optional[int] = None, response_data: Optional[Dict[str, Any]] = None):
        super().__init__(message, code=status_code, details=response_data)
        self.status_code = status_code
        self.response_data = response_data or {}


class WebSocketError(QantoError):
    """WebSocket-related errors."""
    pass


class GraphQLError(QantoError):
    """GraphQL-related errors."""
    
    def __init__(self, message: str, query: Optional[str] = None, variables: Optional[Dict[str, Any]] = None):
        super().__init__(message)
        self.query = query
        self.variables = variables or {}
    
    def __repr__(self) -> str:
        return f"{self.__class__.__name__}(message='{self.message}', query='{self.query}')"


class ValidationError(QantoError):
    """Data validation errors."""
    
    def __init__(self, message: str, field: Optional[str] = None, value: Optional[Any] = None):
        super().__init__(message)
        self.field = field
        self.value = value
    
    def __repr__(self) -> str:
        return f"{self.__class__.__name__}(message='{self.message}', field='{self.field}', value='{self.value}')"


class AuthenticationError(QantoError):
    """Authentication and authorization errors."""
    pass


class TransactionError(QantoError):
    """Transaction-related errors."""
    
    def __init__(self, message: str, transaction_hash: Optional[str] = None):
        super().__init__(message)
        self.transaction_hash = transaction_hash
    
    def __repr__(self) -> str:
        return f"{self.__class__.__name__}(message='{self.message}', transaction_hash='{self.transaction_hash}')"


class BlockchainError(QantoError):
    """Blockchain-related errors."""
    pass


class ConfigurationError(QantoError):
    """Configuration-related errors."""
    pass


class RateLimitError(APIError):
    """Rate limiting errors."""
    
    def __init__(self, message: str, retry_after: Optional[int] = None):
        super().__init__(message, status_code=429)
        self.retry_after = retry_after
    
    def __repr__(self) -> str:
        return f"{self.__class__.__name__}(message='{self.message}', retry_after={self.retry_after})"


class TimeoutError(NetworkError):
    """Request timeout errors."""
    
    def __init__(self, message: str, timeout: Optional[float] = None):
        super().__init__(message)
        self.timeout = timeout
    
    def __repr__(self) -> str:
        return f"{self.__class__.__name__}(message='{self.message}', timeout={self.timeout})"


class InsufficientFundsError(TransactionError):
    """Insufficient funds for transaction."""
    
    def __init__(self, message: str, required: Optional[int] = None, available: Optional[int] = None):
        super().__init__(message)
        self.required = required
        self.available = available
    
    def __repr__(self) -> str:
        return f"{self.__class__.__name__}(message='{self.message}', required={self.required}, available={self.available})"


class InvalidAddressError(ValidationError):
    """Invalid address format error."""
    
    def __init__(self, address: str):
        super().__init__(f"Invalid Qanto address format: {address}", field="address", value=address)
        self.address = address


class InvalidTransactionError(ValidationError):
    """Invalid transaction format error."""
    
    def __init__(self, message: str, transaction_data: Optional[Dict[str, Any]] = None):
        super().__init__(message, field="transaction")
        self.transaction_data = transaction_data or {}


class SubscriptionError(WebSocketError):
    """WebSocket subscription errors."""
    
    def __init__(self, message: str, subscription_type: Optional[str] = None):
        super().__init__(message)
        self.subscription_type = subscription_type
    
    def __repr__(self) -> str:
        return f"{self.__class__.__name__}(message='{self.message}', subscription_type='{self.subscription_type}')"


class ConnectionError(NetworkError):
    """Connection-related errors."""
    
    def __init__(self, message: str, endpoint: Optional[str] = None):
        super().__init__(message)
        self.endpoint = endpoint
    
    def __repr__(self) -> str:
        return f"{self.__class__.__name__}(message='{self.message}', endpoint='{self.endpoint}')"


# Error code mappings
HTTP_ERROR_MAPPING = {
    400: ValidationError,
    401: AuthenticationError,
    403: AuthenticationError,
    404: APIError,
    408: TimeoutError,
    429: RateLimitError,
    500: APIError,
    502: NetworkError,
    503: NetworkError,
    504: TimeoutError,
}


def create_error_from_response(status_code: int, message: str, response_data: Optional[Dict[str, Any]] = None) -> QantoError:
    """Create appropriate error from HTTP response."""
    error_class = HTTP_ERROR_MAPPING.get(status_code, APIError)
    
    if error_class == RateLimitError:
        retry_after = None
        if response_data and 'retry_after' in response_data:
            retry_after = response_data['retry_after']
        return RateLimitError(message, retry_after=retry_after)
    elif error_class == TimeoutError:
        return TimeoutError(message)
    elif issubclass(error_class, APIError):
        return error_class(message, status_code=status_code, response_data=response_data)
    else:
        return error_class(message)