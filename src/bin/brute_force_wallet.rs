use qanto::node_keystore::Wallet;
use secrecy::Secret;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: brute_force_wallet <wallet_file>");
        std::process::exit(1);
    }
    let path = &args[1];

    let passwords = vec![
        "temp",
        "testnet-1-password",
        "testnet-1-password\n",
        "qanto",
        "password",
        "admin",
        "123456",
        "qanto-testnet-1",
        "",
        "ubuntu",
        "root",
    ];

    for p in passwords {
        let password = Secret::new(p.to_string());
        match Wallet::from_file(path, &password) {
            Ok(wallet) => {
                println!("Success! Password is: '{}'", p);
                let (sk, _) = wallet.get_keypair().unwrap();
                let hex_sk = hex::encode(sk.as_bytes());
                println!("Private Key (hex): {}", hex_sk);
                return;
            }
            Err(_) => {
                // println!("Failed with '{}'", p);
            }
        }
    }
    println!("All passwords failed.");
}
