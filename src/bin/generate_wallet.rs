use qanto::wallet::Wallet;
use rpassword::prompt_password;
use secrecy::SecretString;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wallet = Wallet::new()?;
    let address = wallet.address();
    println!("Generated new wallet with address: {address}");

    // Check for WALLET_PASSWORD environment variable first
    let pass = std::env::var("WALLET_PASSWORD").unwrap_or_else(|_| {
        // Fallback to interactive prompt if no environment variable
        prompt_password("Create a password to encrypt the new wallet: ")
            .expect("Failed to read password")
    });

    // Validate that the password is not empty
    if pass.is_empty() {
        return Err("Password cannot be empty.".into());
    }

    // Log when using environment variable
    if std::env::var("WALLET_PASSWORD").is_ok() {
        println!("Using password from WALLET_PASSWORD environment variable.");
    }

    let secret_pass = SecretString::new(pass);

    wallet.save_to_file("wallet.key", &secret_pass)?;
    println!("Wallet saved to wallet.key");
    Ok(())
}
