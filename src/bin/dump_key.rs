use qanto::node_keystore::Wallet;
use secrecy::Secret;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: dump_key <wallet_file> <password>");
        return;
    }
    let wallet_path = &args[1];
    let password = Secret::new(args[2].clone());

    match Wallet::from_file(wallet_path, &password) {
        Ok(wallet) => {
            eprintln!("Successfully loaded wallet!");
            println!("Address: {}", wallet.address());
            println!("Private Key (Hex): {}", wallet.expose_secret_key());
        }
        Err(e) => eprintln!("Error loading wallet: {}", e),
    }
}
