// src/get_address.rs

use crate::{config::Config, wallet::Wallet};
use clap::{Arg, Command};
use secrecy::SecretString;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("get_address")
        .about("Get wallet address from configuration")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .default_value("config.toml"),
        )
        .arg(
            Arg::new("wallet")
                .short('w')
                .long("wallet")
                .value_name("FILE")
                .help("Wallet file path (overrides config)"),
        )
        .get_matches();

    // Load configuration
    let config_path = matches.get_one::<String>("config").unwrap();
    let config = Config::load(config_path)?;

    // Use CLI wallet path or config wallet path
    let wallet_path = matches
        .get_one::<String>("wallet")
        .map(|s| s.to_string())
        .unwrap_or_else(|| config.wallet_path.clone());

    // Check for WALLET_PASSWORD environment variable first
    let pass = std::env::var("WALLET_PASSWORD").unwrap_or_else(|_| {
        // Fallback to interactive prompt if no environment variable
        rpassword::prompt_password("Enter wallet password: ").expect("Failed to read password")
    });

    let password = SecretString::new(pass);
    let wallet = Wallet::from_file(&wallet_path, &password)?;
    println!("Wallet address: {}", wallet.address());
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {
        // Placeholder test to prevent test runner warnings
    }
}
