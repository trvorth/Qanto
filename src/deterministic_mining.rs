use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Deterministic nonce generator for testing purposes
pub struct DeterministicNonceGenerator {
    rng: ChaCha8Rng,
    counter: Arc<AtomicU64>,
}

impl DeterministicNonceGenerator {
    /// Create a new deterministic nonce generator with a fixed seed
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
            counter: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Generate the next deterministic nonce
    pub fn next_nonce(&mut self) -> u64 {
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        // Combine seed-based randomness with counter for deterministic but varied nonces
        self.rng.gen::<u64>().wrapping_add(counter)
    }

    /// Get a thread-safe clone that shares the same counter
    pub fn clone_shared(&self) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(42), // Fixed seed for clones
            counter: Arc::clone(&self.counter),
        }
    }
}

/// Environment variable to enable deterministic mining for tests
pub const DETERMINISTIC_MINING_SEED_ENV: &str = "QANTO_TEST_MINING_SEED";

/// Get deterministic nonce if test environment is detected
pub fn get_deterministic_nonce_if_testing() -> Option<u64> {
    if let Ok(seed_str) = std::env::var(DETERMINISTIC_MINING_SEED_ENV) {
        if let Ok(seed) = seed_str.parse::<u64>() {
            let mut generator = DeterministicNonceGenerator::new(seed);
            return Some(generator.next_nonce());
        }
    }
    None
}

/// Get nonce with optional deterministic behavior for testing
pub fn get_nonce_with_deterministic_fallback() -> u64 {
    get_deterministic_nonce_if_testing().unwrap_or_else(|| rand::thread_rng().gen::<u64>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_nonce_generation() {
        let mut gen1 = DeterministicNonceGenerator::new(12345);
        let mut gen2 = DeterministicNonceGenerator::new(12345);

        // Same seed should produce same sequence
        assert_eq!(gen1.next_nonce(), gen2.next_nonce());
        assert_eq!(gen1.next_nonce(), gen2.next_nonce());
    }

    #[test]
    fn test_different_seeds_produce_different_nonces() {
        let mut gen1 = DeterministicNonceGenerator::new(12345);
        let mut gen2 = DeterministicNonceGenerator::new(54321);

        // Different seeds should produce different sequences
        assert_ne!(gen1.next_nonce(), gen2.next_nonce());
    }

    #[test]
    fn test_env_var_deterministic_nonce() {
        std::env::set_var(DETERMINISTIC_MINING_SEED_ENV, "42");

        // Create a new generator each time to simulate different calls
        let nonce1 = {
            let mut gen = DeterministicNonceGenerator::new(42);
            gen.next_nonce()
        };
        let nonce2 = {
            let mut gen = DeterministicNonceGenerator::new(42);
            gen.next_nonce()
        };

        // Same seed should produce same first nonce
        assert_eq!(nonce1, nonce2);

        // But different calls to the same generator should produce different nonces
        let mut gen = DeterministicNonceGenerator::new(42);
        let first = gen.next_nonce();
        let second = gen.next_nonce();
        assert_ne!(first, second);

        std::env::remove_var(DETERMINISTIC_MINING_SEED_ENV);
    }
}
