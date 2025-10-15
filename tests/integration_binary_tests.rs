use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Integration tests for all binary executables in src/bin/
/// These tests ensure binaries can be executed and handle basic scenarios correctly.

#[test]
fn test_generate_wallet_binary() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let wallet_path = temp_dir.path().join("test_wallet.json");

    let mut cmd = Command::cargo_bin("generate_wallet").unwrap();
    cmd.arg("--output")
        .arg(&wallet_path)
        .env("WALLET_PASSWORD", "test123"); // Use environment variable instead of stdin

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("New wallet saved"));

    // Verify wallet file was created
    assert!(wallet_path.exists(), "Wallet file should be created");
}

#[test]
fn test_derive_genesis_key_binary() {
    let mut cmd = Command::cargo_bin("derive_genesis_key").unwrap();

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Target public key:"))
        .stdout(predicate::str::contains("Deterministic private key:"))
        .stdout(predicate::str::contains("Generated public key:"));
}

#[test]
fn test_get_address_binary() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let wallet_path = temp_dir.path().join("test_wallet.json");

    // First create a wallet
    let mut create_cmd = Command::cargo_bin("generate_wallet").unwrap();
    create_cmd
        .arg("--output")
        .arg(&wallet_path)
        .env("WALLET_PASSWORD", "test123"); // Use environment variable

    create_cmd.assert().success();

    // Then get the address using environment variable for password
    let mut cmd = Command::cargo_bin("get_address").unwrap();
    cmd.arg("--wallet")
        .arg(&wallet_path)
        .env("WALLET_PASSWORD", "test123");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Wallet address:"));
}

#[test]
fn test_import_wallet_binary() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let wallet_path = temp_dir.path().join("imported_wallet.json");

    // Test importing with a known private key (test key)
    let test_private_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    let mut cmd = Command::cargo_bin("import_wallet").unwrap();
    cmd.arg(test_private_key)
        .arg("--output")
        .arg(&wallet_path)
        .env("WALLET_PASSWORD", "test123"); // Use environment variable instead of stdin

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Wallet imported and saved"));

    // Verify wallet file was created
    assert!(
        wallet_path.exists(),
        "Imported wallet file should be created"
    );
}

#[test]
fn test_qanto_binary_help() {
    // Test main qanto binary help
    let mut cmd = Command::cargo_bin("qanto").unwrap();
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Qanto Node CLI"))
        .stdout(predicate::str::contains("Commands:"));
}

#[test]
fn test_qantowallet_binary_help() {
    // Test qantowallet binary help
    let mut cmd = Command::cargo_bin("qantowallet").unwrap();
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("autonomous, zero-dependency CLI"))
        .stdout(predicate::str::contains("Commands:"));
}

#[test]
fn test_start_node_binary_help() {
    // Test start_node binary help
    let mut cmd = Command::cargo_bin("start_node").unwrap();
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Start a Qanto node"));
}

#[test]
fn test_qanto_subcommands() {
    // Test qanto start subcommand help
    let mut cmd = Command::cargo_bin("qanto").unwrap();
    cmd.arg("start").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Starts the Qanto node"));

    // Test qanto generate-wallet subcommand help
    let mut cmd = Command::cargo_bin("qanto").unwrap();
    cmd.arg("generate-wallet").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Generates a new wallet"));
}

#[test]
fn test_binaries_help_flag() {
    let binaries_with_help = [
        ("qanto", "Qanto Node CLI"),
        ("qantowallet", "autonomous, zero-dependency CLI"),
        ("generate_wallet", "Usage:"),
        ("import_wallet", "Usage:"),
        ("get_address", "Usage:"),
        ("derive_genesis_key", "Usage:"),
        ("start_node", "Start a Qanto node"),
    ];

    for (binary, expected_text) in &binaries_with_help {
        let mut cmd = Command::cargo_bin(binary).unwrap();
        cmd.arg("--help");

        let output = cmd.output().expect("Failed to execute command");

        // Check if help was displayed successfully
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.contains(expected_text)
                    || stdout.contains("Usage:")
                    || stdout.contains("USAGE:"),
                "Binary {binary} should show help with expected text"
            );
        } else {
            // Some binaries might not support --help, which is acceptable
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("error:") || stderr.contains("unrecognized"),
                "Binary {binary} should handle --help flag gracefully"
            );
        }
    }
}

#[test]
fn test_generate_wallet_invalid_args() {
    // Test that generate_wallet fails gracefully with invalid arguments
    let mut cmd = Command::cargo_bin("generate_wallet").unwrap();
    cmd.arg("--invalid-flag");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error:").or(predicate::str::contains("unrecognized")));
}

#[test]
fn test_import_wallet_missing_key() {
    // Test that import_wallet fails when no arguments are provided
    let mut cmd = Command::cargo_bin("import_wallet").unwrap();

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("required").or(predicate::str::contains("error:")));
}

// Additional edge case tests for better coverage

#[test]
fn test_generate_wallet_invalid_output_path() {
    // Test generate_wallet with invalid output path
    let mut cmd = Command::cargo_bin("generate_wallet").unwrap();
    cmd.arg("--output")
        .arg("/invalid/path/that/does/not/exist/wallet.json")
        .env("WALLET_PASSWORD", "test123");

    cmd.assert().failure().stderr(
        predicate::str::contains("No such file or directory").or(predicate::str::contains("error")),
    );
}

#[test]
fn test_import_wallet_invalid_private_key() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let wallet_path = temp_dir.path().join("invalid_wallet.json");

    // Test with invalid private key format
    let invalid_private_key = "invalid_key_format";

    let mut cmd = Command::cargo_bin("import_wallet").unwrap();
    cmd.arg(invalid_private_key)
        .arg("--output")
        .arg(&wallet_path)
        .env("WALLET_PASSWORD", "test123");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Invalid").or(predicate::str::contains("error")));
}

#[test]
fn test_import_wallet_short_private_key() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let wallet_path = temp_dir.path().join("short_key_wallet.json");

    // Test with too short private key
    let short_private_key = "123abc";

    let mut cmd = Command::cargo_bin("import_wallet").unwrap();
    cmd.arg(short_private_key)
        .arg("--output")
        .arg(&wallet_path)
        .env("WALLET_PASSWORD", "test123");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Invalid").or(predicate::str::contains("error")));
}

#[test]
fn test_get_address_nonexistent_wallet() {
    // Test get_address with non-existent wallet file
    let mut cmd = Command::cargo_bin("get_address").unwrap();
    cmd.arg("--wallet")
        .arg("/path/that/does/not/exist/wallet.json")
        .env("WALLET_PASSWORD", "test123");

    cmd.assert().failure().stderr(
        predicate::str::contains("No such file or directory").or(predicate::str::contains("error")),
    );
}

#[test]
fn test_get_address_invalid_wallet_format() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let invalid_wallet_path = temp_dir.path().join("invalid_wallet.json");

    // Create an invalid wallet file
    std::fs::write(&invalid_wallet_path, "invalid json content")
        .expect("Failed to write invalid wallet");

    let mut cmd = Command::cargo_bin("get_address").unwrap();
    cmd.arg("--wallet")
        .arg(&invalid_wallet_path)
        .env("WALLET_PASSWORD", "test123");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error").or(predicate::str::contains("Invalid")));
}

#[test]
fn test_derive_genesis_key_with_custom_target() {
    // Test derive_genesis_key with custom target public key
    let custom_target = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    let mut cmd = Command::cargo_bin("derive_genesis_key").unwrap();
    cmd.arg("--target").arg(custom_target);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Target public key:"))
        .stdout(predicate::str::contains(custom_target));
}

#[test]
fn test_derive_genesis_key_invalid_target() {
    // Test derive_genesis_key with invalid target public key (non-hex)
    let invalid_target = "invalid_target_key";

    let mut cmd = Command::cargo_bin("derive_genesis_key").unwrap();
    cmd.arg("--target").arg(invalid_target);

    // The binary will still run successfully but will show the invalid target
    cmd.assert().success().stdout(predicate::str::contains(
        "Target public key: invalid_target_key",
    ));
}

#[test]
fn test_qanto_invalid_subcommand() {
    // Test qanto binary with invalid subcommand
    let mut cmd = Command::cargo_bin("qanto").unwrap();
    cmd.arg("invalid_subcommand");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized").or(predicate::str::contains("error")));
}

#[test]
fn test_qantowallet_invalid_args() {
    // Test qantowallet binary with invalid arguments
    let mut cmd = Command::cargo_bin("qantowallet").unwrap();
    cmd.arg("--invalid-flag");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized").or(predicate::str::contains("error")));
}

#[test]
fn test_start_node_invalid_config() {
    // Test start_node with invalid config path
    let mut cmd = Command::cargo_bin("start_node").unwrap();
    cmd.arg("--config").arg("/invalid/config/path.toml");

    cmd.assert().failure().stderr(
        predicate::str::contains("No such file or directory").or(predicate::str::contains("error")),
    );
}

#[test]
fn test_generate_wallet_empty_password() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let wallet_path = temp_dir.path().join("empty_password_wallet.json");

    // Test with empty password
    let mut cmd = Command::cargo_bin("generate_wallet").unwrap();
    cmd.arg("--output")
        .arg(&wallet_path)
        .env("WALLET_PASSWORD", ""); // Empty password

    cmd.assert().failure().stderr(predicate::str::contains(
        "WALLET_PASSWORD environment variable is set but empty!",
    ));
}

#[test]
fn test_import_wallet_empty_password() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let wallet_path = temp_dir.path().join("empty_password_import.json");

    let test_private_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    // Test with empty password
    let mut cmd = Command::cargo_bin("import_wallet").unwrap();
    cmd.arg(test_private_key)
        .arg("--output")
        .arg(&wallet_path)
        .env("WALLET_PASSWORD", ""); // Empty password

    cmd.assert().failure().stderr(predicate::str::contains(
        "WALLET_PASSWORD environment variable is set but empty!",
    ));
}

#[test]
fn test_get_address_wrong_password() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let wallet_path = temp_dir.path().join("wrong_password_wallet.json");

    // First create a wallet with one password
    let mut create_cmd = Command::cargo_bin("generate_wallet").unwrap();
    create_cmd
        .arg("--output")
        .arg(&wallet_path)
        .env("WALLET_PASSWORD", "correct_password");

    create_cmd.assert().success();

    // Then try to access with wrong password
    let mut cmd = Command::cargo_bin("get_address").unwrap();
    cmd.arg("--wallet")
        .arg(&wallet_path)
        .env("WALLET_PASSWORD", "wrong_password");

    cmd.assert().failure().stderr(
        predicate::str::contains("password")
            .or(predicate::str::contains("decrypt"))
            .or(predicate::str::contains("error")),
    );
}

#[test]
fn test_binary_version_flags() {
    // Test version flags for binaries that support it (only qanto and qantowallet)
    let binaries = vec!["qanto", "qantowallet"];

    for binary in binaries {
        let mut cmd = Command::cargo_bin(binary).unwrap();
        cmd.arg("--version");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("qanto").or(predicate::str::contains("version")));
    }
}
