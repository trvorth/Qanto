use clap::Parser;
use qanto::node_keystore::Wallet;
use secrecy::SecretString;
use std::path::PathBuf;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long, default_value = "wallet.key")]
    wallet: PathBuf,

    #[arg(short, long)]
    password: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let password = args.password.unwrap_or_else(|| {
        std::env::var("QANTO_WALLET_PASSWORD").unwrap_or_else(|_| "password".to_string())
    });

    let secret_password = SecretString::new(password);
    let wallet = Wallet::from_file(&args.wallet, &secret_password)?;

    let (sk, _) = wallet.get_keypair()?;
    let hex_sk = hex::encode(sk.as_bytes());

    println!("{}", hex_sk);

    Ok(())
}
