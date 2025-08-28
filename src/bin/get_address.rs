use qanto::wallet::Wallet;
use secrecy::SecretString;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check for WALLET_PASSWORD environment variable first
    let pass = std::env::var("WALLET_PASSWORD").unwrap_or_else(|_| {
        // Fallback to interactive prompt if no environment variable
        rpassword::prompt_password("Enter wallet password: ").expect("Failed to read password")
    });

    let password = SecretString::new(pass);
    let wallet = Wallet::from_file("test_wallet.key", &password)?;
    println!("Wallet address: {}", wallet.address());
    Ok(())
}
