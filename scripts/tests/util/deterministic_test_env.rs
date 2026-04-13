use std::env;

/// Environment variable for deterministic mining seed
pub const DETERMINISTIC_MINING_SEED: &str = "QANTO_TEST_MINING_SEED";

/// Environment variable for deterministic time seed
pub const DETERMINISTIC_TIME_SEED: &str = "QANTO_TEST_TIME_SEED";

/// Setup deterministic environment for tests
pub fn setup_deterministic_test_env() {
    // Set deterministic mining seed if not already set
    if env::var(DETERMINISTIC_MINING_SEED).is_err() {
        env::set_var(DETERMINISTIC_MINING_SEED, "42");
    }

    // Set deterministic time seed if not already set
    if env::var(DETERMINISTIC_TIME_SEED).is_err() {
        env::set_var(DETERMINISTIC_TIME_SEED, "1234567890");
    }

    // Ensure single-threaded execution for deterministic behavior
    env::set_var("RUST_TEST_THREADS", "1");
}

/// Cleanup deterministic environment after tests
pub fn cleanup_deterministic_test_env() {
    env::remove_var(DETERMINISTIC_MINING_SEED);
    env::remove_var(DETERMINISTIC_TIME_SEED);
    env::remove_var("RUST_TEST_THREADS");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_deterministic_env() {
        setup_deterministic_test_env();

        assert_eq!(env::var(DETERMINISTIC_MINING_SEED).unwrap(), "42");
        assert_eq!(env::var(DETERMINISTIC_TIME_SEED).unwrap(), "1234567890");
        assert_eq!(env::var("RUST_TEST_THREADS").unwrap(), "1");

        cleanup_deterministic_test_env();
    }
}
