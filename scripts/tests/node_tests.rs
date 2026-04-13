//! Node Tests
//!
//! This module provides comprehensive tests for node functionality
//! including node creation, configuration, and initialization.

#[cfg(test)]
mod tests {
    use qanto::config::Config;
    use qanto::node::*;
    use qanto::wallet::Wallet;
    use rand::Rng;
    use serial_test::serial;
    use std::fs as std_fs;
    use std::sync::Arc;

    #[tokio::test]
    #[serial]
    async fn test_node_creation_and_config_save() -> Result<(), Box<dyn std::error::Error>> {
        // Setup paths for test artifacts.
        let db_path = "qantodag_db_test_node_creation";
        if std::path::Path::new(db_path).exists() {
            let _ = std_fs::remove_dir_all(db_path);
        }
        // let _ = tracing_subscriber::fmt::try_init();

        // Create a new wallet for the test.
        let wallet =
            Wallet::new().map_err(|e| format!("Failed to create new wallet for test: {e}"))?;
        let wallet_arc = Arc::new(wallet);
        let _genesis_validator_addr = wallet_arc.address();

        // Use a random ID to prevent test conflicts.
        let rand_id: u32 = rand::thread_rng().gen();
        let temp_config_path = format!("./temp_test_config_{rand_id}.toml");
        let temp_identity_path = format!("./temp_p2p_identity_{rand_id}.key");
        let temp_peer_cache_path = format!("./temp_peer_cache_{rand_id}.json");

        // Create a default config for the test.
        // Initialize config with desired genesis validator using struct update to avoid reassign after Default
        let test_config = Config {
            genesis_validator: wallet_arc.address(),
            ..Default::default()
        };
        test_config
            .save(&temp_config_path)
            .map_err(|e| format!("Failed to save initial temp config for test: {e}"))?;

        // Attempt to create a new node instance.
        let node_instance_result = Node::new(
            test_config,
            temp_config_path.clone(),
            wallet_arc.clone(),
            &temp_identity_path,
            temp_peer_cache_path.clone(),
        )
        .await;

        // --- Teardown ---
        if std::path::Path::new(db_path).exists() {
            let _ = std_fs::remove_dir_all(db_path);
        }
        let _ = std_fs::remove_file(&temp_config_path);
        let _ = std_fs::remove_file(&temp_identity_path);
        let _ = std_fs::remove_file(&temp_peer_cache_path);

        // Assert that node creation was successful.
        assert!(
            node_instance_result.is_ok(),
            "Node::new failed: {:?}",
            node_instance_result.err()
        );

        Ok(())
    }
}
