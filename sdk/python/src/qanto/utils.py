"""Utility functions for the Qanto Network SDK."""

import re
import time
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional, Union
import asyncio
import logging
from decimal import Decimal, ROUND_HALF_UP

from .exceptions import ValidationError, InvalidAddressError, InvalidTransactionError
from .types import QantoBlock, Transaction, UTXO

# Configure logging
logger = logging.getLogger(__name__)

# Constants
QANTO_ADDRESS_PATTERN = re.compile(r'^qanto[a-zA-Z0-9]{39}$')
HEX_PATTERN = re.compile(r'^[0-9a-fA-F]+$')
QANTO_DECIMALS = 8
SATOSHI_PER_QANTO = 10 ** QANTO_DECIMALS


def is_valid_qanto_address(address: str) -> bool:
    """Validate Qanto address format.
    
    Args:
        address: The address to validate
        
    Returns:
        True if valid, False otherwise
    """
    if not isinstance(address, str):
        return False
    
    return bool(QANTO_ADDRESS_PATTERN.match(address))


def validate_qanto_address(address: str) -> None:
    """Validate Qanto address format, raise exception if invalid.
    
    Args:
        address: The address to validate
        
    Raises:
        InvalidAddressError: If address format is invalid
    """
    if not is_valid_qanto_address(address):
        raise InvalidAddressError(address)


def is_valid_transaction(transaction: Dict[str, Any]) -> bool:
    """Validate transaction structure.
    
    Args:
        transaction: Transaction dictionary to validate
        
    Returns:
        True if valid, False otherwise
    """
    try:
        # Check required fields
        required_fields = ['hash', 'inputs', 'outputs', 'timestamp']
        for field in required_fields:
            if field not in transaction:
                return False
        
        # Validate inputs
        if not isinstance(transaction['inputs'], list):
            return False
        
        for inp in transaction['inputs']:
            if not isinstance(inp, dict):
                return False
            if 'previous_hash' not in inp or 'output_index' not in inp:
                return False
        
        # Validate outputs
        if not isinstance(transaction['outputs'], list):
            return False
        
        for out in transaction['outputs']:
            if not isinstance(out, dict):
                return False
            if 'address' not in out or 'amount' not in out:
                return False
            if not is_valid_qanto_address(out['address']):
                return False
            if not isinstance(out['amount'], (int, float)):
                return False
        
        return True
    except Exception:
        return False


def validate_transaction(transaction: Dict[str, Any]) -> None:
    """Validate transaction structure, raise exception if invalid.
    
    Args:
        transaction: Transaction dictionary to validate
        
    Raises:
        InvalidTransactionError: If transaction structure is invalid
    """
    if not is_valid_transaction(transaction):
        raise InvalidTransactionError("Invalid transaction structure", transaction)


def is_valid_block(block: Dict[str, Any]) -> bool:
    """Validate block structure.
    
    Args:
        block: Block dictionary to validate
        
    Returns:
        True if valid, False otherwise
    """
    try:
        # Check required fields
        required_fields = ['hash', 'height', 'timestamp', 'transactions', 'previous_hash']
        for field in required_fields:
            if field not in block:
                return False
        
        # Validate transactions
        if not isinstance(block['transactions'], list):
            return False
        
        for tx in block['transactions']:
            if not is_valid_transaction(tx):
                return False
        
        return True
    except Exception:
        return False


def is_valid_utxo(utxo: Dict[str, Any]) -> bool:
    """Validate UTXO structure.
    
    Args:
        utxo: UTXO dictionary to validate
        
    Returns:
        True if valid, False otherwise
    """
    try:
        required_fields = ['transaction_hash', 'output_index', 'address', 'amount']
        for field in required_fields:
            if field not in utxo:
                return False
        
        if not is_valid_qanto_address(utxo['address']):
            return False
        
        if not isinstance(utxo['amount'], (int, float)):
            return False
        
        return True
    except Exception:
        return False


def hex_to_bytes(hex_string: str) -> bytes:
    """Convert hex string to bytes.
    
    Args:
        hex_string: Hex string to convert
        
    Returns:
        Bytes representation
        
    Raises:
        ValidationError: If hex string is invalid
    """
    if not isinstance(hex_string, str):
        raise ValidationError("Hex string must be a string")
    
    # Remove '0x' prefix if present
    if hex_string.startswith('0x'):
        hex_string = hex_string[2:]
    
    if not HEX_PATTERN.match(hex_string):
        raise ValidationError(f"Invalid hex string: {hex_string}")
    
    try:
        return bytes.fromhex(hex_string)
    except ValueError as e:
        raise ValidationError(f"Failed to convert hex to bytes: {e}")


def bytes_to_hex(data: bytes, prefix: bool = False) -> str:
    """Convert bytes to hex string.
    
    Args:
        data: Bytes to convert
        prefix: Whether to add '0x' prefix
        
    Returns:
        Hex string representation
    """
    hex_str = data.hex()
    return f"0x{hex_str}" if prefix else hex_str


def format_timestamp(timestamp: Union[int, float], format_str: str = "%Y-%m-%d %H:%M:%S UTC") -> str:
    """Format timestamp to human-readable string.
    
    Args:
        timestamp: Unix timestamp
        format_str: Format string for datetime
        
    Returns:
        Formatted timestamp string
    """
    dt = datetime.fromtimestamp(timestamp, tz=timezone.utc)
    return dt.strftime(format_str)


def parse_timestamp(timestamp_str: str) -> int:
    """Parse timestamp string to Unix timestamp.
    
    Args:
        timestamp_str: Timestamp string in ISO format
        
    Returns:
        Unix timestamp
    """
    try:
        dt = datetime.fromisoformat(timestamp_str.replace('Z', '+00:00'))
        return int(dt.timestamp())
    except ValueError as e:
        raise ValidationError(f"Invalid timestamp format: {e}")


def format_amount(amount: Union[int, float, Decimal], decimals: int = QANTO_DECIMALS) -> str:
    """Format amount with proper decimal places.
    
    Args:
        amount: Amount to format (in smallest units)
        decimals: Number of decimal places
        
    Returns:
        Formatted amount string
    """
    if isinstance(amount, (int, float)):
        amount = Decimal(str(amount))
    
    divisor = Decimal(10 ** decimals)
    formatted = (amount / divisor).quantize(Decimal('0.' + '0' * decimals), rounding=ROUND_HALF_UP)
    
    # Remove trailing zeros
    return str(formatted.normalize())


def parse_amount(amount_str: str, decimals: int = QANTO_DECIMALS) -> int:
    """Parse amount string to smallest units.
    
    Args:
        amount_str: Amount string to parse
        decimals: Number of decimal places
        
    Returns:
        Amount in smallest units
    """
    try:
        amount = Decimal(amount_str)
        multiplier = Decimal(10 ** decimals)
        return int(amount * multiplier)
    except (ValueError, TypeError) as e:
        raise ValidationError(f"Invalid amount format: {e}")


def qanto_to_satoshi(qanto_amount: Union[int, float, Decimal]) -> int:
    """Convert QANTO to satoshi (smallest unit).
    
    Args:
        qanto_amount: Amount in QANTO
        
    Returns:
        Amount in satoshi
    """
    if isinstance(qanto_amount, (int, float)):
        qanto_amount = Decimal(str(qanto_amount))
    
    return int(qanto_amount * SATOSHI_PER_QANTO)


def satoshi_to_qanto(satoshi_amount: int) -> Decimal:
    """Convert satoshi to QANTO.
    
    Args:
        satoshi_amount: Amount in satoshi
        
    Returns:
        Amount in QANTO
    """
    return Decimal(satoshi_amount) / SATOSHI_PER_QANTO


def hash_data(data: Union[str, bytes]) -> str:
    """Hash data using QanHash32x algorithm (ported from Rust).
    
    Args:
        data: Data to hash (string or bytes)
        
    Returns:
        Hex-encoded hash
    """
    if isinstance(data, str):
        data = data.encode('utf-8')
    
    # Ported QanHash32x implementation
    KECCAK_ROUNDS = 24
    FOLD_ROUNDS = 16
    KECCAK_ROUND_CONSTANTS = [
        0x0000000000000001, 0x0000000000008082, 0x800000000000808a,
        0x8000000080008000, 0x000000000000808b, 0x0000000080000001,
        0x8000000080008081, 0x8000000000008009, 0x000000000000008a,
        0x0000000000000088, 0x0000000080008009, 0x000000008000000a,
        0x000000008000808b, 0x800000000000008b, 0x8000000000008089,
        0x8000000000008003, 0x8000000000008002, 0x8000000000000080,
        0x000000000000800a, 0x800000008000000a, 0x8000000080008081,
        0x8000000000008080, 0x0000000080000001, 0x8000000080008008
    ]
    
    def keccak_f(state):
        ROTATION_CONSTANTS = [1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44]
        PI_LANES = [10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1]
        for round_constant in KECCAK_ROUND_CONSTANTS:
            c = [0] * 5
            for x in range(5):
                c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20]
            for x in range(5):
                d = c[(x + 4) % 5] ^ ((c[(x + 1) % 5] << 1) | (c[(x + 1) % 5] >> 63))
                for y in range(5):
                    state[x + 5 * y] ^= d
            temp = state[1]
            for i in range(24):
                j = PI_LANES[i]
                next_temp = state[j]
                state[j] = ((temp << ROTATION_CONSTANTS[i]) | (temp >> (64 - ROTATION_CONSTANTS[i]))) & ((1 << 64) - 1)
                temp = next_temp
            for y_step in range(5):
                y = y_step * 5
                t = state[y:y+5]
                for x in range(5):
                    state[y + x] = t[x] ^ ((~t[(x + 1) % 5]) & t[(x + 2) % 5])
            state[0] ^= round_constant
    
    state = [0] * 25
    rate_in_bytes = 136
    for chunk in [data[i:i+rate_in_bytes] for i in range(0, len(data), rate_in_bytes)]:
        for i, word_chunk in enumerate([chunk[j:j+8] for j in range(0, len(chunk), 8)]):
            word = [0] * 8
            word[:len(word_chunk)] = word_chunk
            state[i] ^= int.from_bytes(bytes(word), 'little')
        keccak_f(state)
    for _ in range(FOLD_ROUNDS):
        for i in range(len(state)):
            lane = state[i]
            state[i] = lane ^ ((lane * 0x9E3779B97F4A7C15) & ((1 << 64) - 1))
            state[i] = ((state[i] << 13) | (state[i] >> (64 - 13))) & ((1 << 64) - 1)
        keccak_f(state)
    out = bytearray(32)
    for i in range(4):
        out[i*8:(i+1)*8] = state[i].to_bytes(8, 'little')
    return out.hex()


def simple_hash(data: Union[str, bytes]) -> str:
    """Generate simple QanHash.
    
    Args:
        data: Data to hash
        
    Returns:
        Hex-encoded hash
    """
    # Use the same QanHash implementation as hash_data for consistency
    return hash_data(data)


def calculate_transaction_fee(transaction: Dict[str, Any], fee_rate: float = 0.001) -> int:
    """Calculate transaction fee based on size and fee rate.
    
    Args:
        transaction: Transaction dictionary
        fee_rate: Fee rate per byte
        
    Returns:
        Calculated fee in satoshi
    """
    # Estimate transaction size (simplified)
    base_size = 100  # Base transaction overhead
    input_size = len(transaction.get('inputs', [])) * 150  # ~150 bytes per input
    output_size = len(transaction.get('outputs', [])) * 35  # ~35 bytes per output
    
    estimated_size = base_size + input_size + output_size
    fee = int(estimated_size * fee_rate * SATOSHI_PER_QANTO)
    
    return max(fee, 1000)  # Minimum fee of 1000 satoshi


async def retry_async(func, max_retries: int = 3, delay: float = 1.0, backoff: float = 2.0, exceptions: tuple = (Exception,)):
    """Retry async function with exponential backoff.
    
    Args:
        func: Async function to retry
        max_retries: Maximum number of retries
        delay: Initial delay between retries
        backoff: Backoff multiplier
        exceptions: Exceptions to catch and retry on
        
    Returns:
        Function result
        
    Raises:
        Last exception if all retries fail
    """
    last_exception = None
    
    for attempt in range(max_retries + 1):
        try:
            return await func()
        except exceptions as e:
            last_exception = e
            if attempt < max_retries:
                wait_time = delay * (backoff ** attempt)
                logger.warning(f"Attempt {attempt + 1} failed, retrying in {wait_time}s: {e}")
                await asyncio.sleep(wait_time)
            else:
                logger.error(f"All {max_retries + 1} attempts failed")
    
    raise last_exception


def retry_sync(func, max_retries: int = 3, delay: float = 1.0, backoff: float = 2.0, exceptions: tuple = (Exception,)):
    """Retry sync function with exponential backoff.
    
    Args:
        func: Function to retry
        max_retries: Maximum number of retries
        delay: Initial delay between retries
        backoff: Backoff multiplier
        exceptions: Exceptions to catch and retry on
        
    Returns:
        Function result
        
    Raises:
        Last exception if all retries fail
    """
    last_exception = None
    
    for attempt in range(max_retries + 1):
        try:
            return func()
        except exceptions as e:
            last_exception = e
            if attempt < max_retries:
                wait_time = delay * (backoff ** attempt)
                logger.warning(f"Attempt {attempt + 1} failed, retrying in {wait_time}s: {e}")
                time.sleep(wait_time)
            else:
                logger.error(f"All {max_retries + 1} attempts failed")
    
    raise last_exception


def chunk_list(lst: List[Any], chunk_size: int) -> List[List[Any]]:
    """Split list into chunks of specified size.
    
    Args:
        lst: List to chunk
        chunk_size: Size of each chunk
        
    Returns:
        List of chunks
    """
    return [lst[i:i + chunk_size] for i in range(0, len(lst), chunk_size)]


def merge_dicts(*dicts: Dict[str, Any]) -> Dict[str, Any]:
    """Merge multiple dictionaries.
    
    Args:
        *dicts: Dictionaries to merge
        
    Returns:
        Merged dictionary
    """
    result = {}
    for d in dicts:
        if d:
            result.update(d)
    return result


def safe_get(data: Dict[str, Any], key: str, default: Any = None) -> Any:
    """Safely get value from dictionary with dot notation support.
    
    Args:
        data: Dictionary to get value from
        key: Key (supports dot notation like 'a.b.c')
        default: Default value if key not found
        
    Returns:
        Value or default
    """
    try:
        keys = key.split('.')
        value = data
        for k in keys:
            value = value[k]
        return value
    except (KeyError, TypeError):
        return default


def truncate_hash(hash_str: str, length: int = 8) -> str:
    """Truncate hash for display purposes.
    
    Args:
        hash_str: Hash string to truncate
        length: Number of characters to keep from start and end
        
    Returns:
        Truncated hash string
    """
    if len(hash_str) <= length * 2:
        return hash_str
    
    return f"{hash_str[:length]}...{hash_str[-length:]}"


def validate_network_config(config: Dict[str, Any]) -> None:
    """Validate network configuration.
    
    Args:
        config: Network configuration to validate
        
    Raises:
        ValidationError: If configuration is invalid
    """
    required_fields = ['http_endpoint', 'ws_endpoint', 'graphql_endpoint']
    
    for field in required_fields:
        if field not in config:
            raise ValidationError(f"Missing required field: {field}")
        
        if not isinstance(config[field], str):
            raise ValidationError(f"Field {field} must be a string")
        
        if not config[field].startswith(('http://', 'https://', 'ws://', 'wss://')):
            raise ValidationError(f"Field {field} must be a valid URL")


def get_current_timestamp() -> int:
    """Get current Unix timestamp.
    
    Returns:
        Current timestamp
    """
    return int(time.time())


def is_expired(timestamp: int, ttl: int) -> bool:
    """Check if timestamp is expired based on TTL.
    
    Args:
        timestamp: Timestamp to check
        ttl: Time to live in seconds
        
    Returns:
        True if expired, False otherwise
    """
    return get_current_timestamp() - timestamp > ttl


class RateLimiter:
    """Simple rate limiter implementation."""
    
    def __init__(self, max_requests: int, time_window: int):
        """Initialize rate limiter.
        
        Args:
            max_requests: Maximum requests allowed
            time_window: Time window in seconds
        """
        self.max_requests = max_requests
        self.time_window = time_window
        self.requests = []
    
    def is_allowed(self) -> bool:
        """Check if request is allowed.
        
        Returns:
            True if allowed, False otherwise
        """
        now = time.time()
        
        # Remove old requests
        self.requests = [req_time for req_time in self.requests if now - req_time < self.time_window]
        
        # Check if we can make a new request
        if len(self.requests) < self.max_requests:
            self.requests.append(now)
            return True
        
        return False
    
    def time_until_allowed(self) -> float:
        """Get time until next request is allowed.
        
        Returns:
            Time in seconds until next request is allowed
        """
        if not self.requests:
            return 0.0
        
        oldest_request = min(self.requests)
        return max(0.0, self.time_window - (time.time() - oldest_request))