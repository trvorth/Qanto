// src/import_wallet.rs

use crate::{config::Config, password_utils::prompt_for_password, wallet::Wallet};
use clap::{Arg, Command};

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("import_wallet")
        .about("Import a Qanto wallet from a private key")
        .arg(
            Arg::new("private_key")
                .help("Private key to import")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Output wallet file path")
                .default_value("wallet.key"),
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .default_value("config.toml"),
        )
        .get_matches();

    let private_key = matches.get_one::<String>("private_key").unwrap();
    let output_path = matches.get_one::<String>("output").unwrap();
    let config_path = matches.get_one::<String>("config").unwrap();

    // Load configuration (for potential future use)
    let _config = Config::load(config_path)?;

    // Use unified password reading function for consistency
    let password = prompt_for_password(true, Some("Enter password for imported wallet:"))?;
    let wallet = Wallet::from_private_key(private_key)?;
    wallet.save_to_file(output_path, &password)?;

    println!(
        "Wallet imported and saved to '{}'. Address: {}",
        output_path,
        wallet.address()
    );
    println!("IMPORTANT: Please back up this file securely and remember your password.");

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {
        // Placeholder test to prevent test runner warnings
    }
}
