use qanto::wallet::Wallet;
use secrecy::SecretString;
use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: cargo run --bin import_wallet \"<private_key_or_mnemonic>\"");
        std::process::exit(1);
    }
    let key_or_mnemonic = &args[1];
    let wallet = if key_or_mnemonic.split_whitespace().count() >= 12 {
        println!("Attempting to import from mnemonic...");
        Wallet::from_mnemonic(key_or_mnemonic)?
    } else {
        println!("Attempting to import from private key...");
        Wallet::from_private_key(key_or_mnemonic)?
    };
    // Check for WALLET_PASSWORD environment variable first
    let password = std::env::var("WALLET_PASSWORD").unwrap_or_else(|_| {
        // Fallback to interactive prompt if no environment variable
        rpassword::prompt_password("Create a password to encrypt the imported wallet: ")
            .expect("Failed to read password")
    });

    // Validate that the password is not empty
    if password.is_empty() {
        return Err("Password cannot be empty.".into());
    }

    // Log when using environment variable
    if std::env::var("WALLET_PASSWORD").is_ok() {
        println!("Using password from WALLET_PASSWORD environment variable.");
    }

    let secret_password = SecretString::new(password);
    wallet.save_to_file("wallet.key", &secret_password)?;
    println!(
        "Wallet imported and saved successfully!\n  Address: {}",
        wallet.address()
    );
    Ok(())
}
