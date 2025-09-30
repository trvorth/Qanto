# Debug target calculation for difficulty 1.0
import math

# Constants from the code
SCALE = 1000  # 1e3 for reasonable precision
difficulty_value = 1.0

# Calculate scaled difficulty
difficulty_scaled = int(difficulty_value * SCALE)
print(f"Difficulty scaled: {difficulty_scaled}")

# Max target is 2^256 - 1
max_target = 2**256 - 1
print(f"Max target: {hex(max_target)}")

# Target calculation: max_target / difficulty_scaled
target = max_target // difficulty_scaled
print(f"Target for difficulty 1.0: {hex(target)}")

# Convert to 32-byte representation
target_bytes = target.to_bytes(32, 'big')
print(f"Target bytes (hex): {target_bytes.hex()}")
print(f"Target bytes length: {len(target_bytes)}")

# Show what a typical hash might look like
print(f"\nFor comparison, a typical hash might be:")
print(f"0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef")
print(f"Target is much larger, so most hashes should meet it")
