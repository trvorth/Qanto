use qanto::node_keystore::Wallet;
use secrecy::Secret;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: decrypt_wallet <wallet_file> <password>");
        std::process::exit(1);
    }
    let path = &args[1];
    let password = Secret::new(args[2].clone());

    let wallet = Wallet::from_file(path, &password)?;
    eprintln!("Ed25519 Address: {}", wallet.address());
    let (sk, _) = wallet.get_keypair()?;

    let sk_bytes = sk.as_bytes();
    let hex_sk = hex::encode(sk_bytes);

    print!("{}", hex_sk); // Use print! to avoid newline if piping
    Ok(())
}
