use clap::Parser;
use secrecy::SecretString;
use std::path::PathBuf;

use qanto::config::Config;
use qanto::wallet::Wallet;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Generate an encrypted wallet non-interactively using WALLET_PASSWORD"
)]
struct Cli {
    /// Optional config file to resolve default wallet path
    #[arg(long)]
    config: Option<PathBuf>,
    /// Explicit output file path for the wallet
    #[arg(long)]
    output: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Require WALLET_PASSWORD from environment to avoid interactive prompt
    let pass = std::env::var("WALLET_PASSWORD")
        .map_err(|_| "WALLET_PASSWORD not set. Export a password before running gen_wallet.")?;
    let password = SecretString::new(pass);

    // Determine output path: CLI > config > default
    let output_path = if let Some(out) = cli.output {
        out
    } else if let Some(cfg_path) = cli.config {
        let cfg = Config::load(cfg_path.to_str().unwrap())
            .map_err(|e| format!("Failed to load config: {e}"))?;
        PathBuf::from(cfg.wallet_path)
    } else {
        PathBuf::from("wallet.key")
    };

    // Create and save wallet
    let wallet = Wallet::new()?;
    wallet.save_to_file(&output_path, &password)?;

    println!("✅ Wallet generated: {}", output_path.display());
    println!("Address: {}", wallet.address());

    Ok(())
}
