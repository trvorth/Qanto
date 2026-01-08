use qanto_core::qanto_native_crypto::{QantoKeyPair, QantoPQPrivateKey};
use std::fs::File;
use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::open("stolen_key.txt")?; // Using the binary file directly
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;

    // Try to deserialize as QantoKeyPair
    match bincode::deserialize::<QantoKeyPair>(&bytes) {
        Ok(keypair) => match keypair {
            QantoKeyPair::PostQuantum { private, .. } => {
                let raw_bytes = private.as_bytes();
                println!("{}", hex::encode(raw_bytes));
            }
            _ => eprintln!("Not a PostQuantum keypair"),
        },
        Err(e) => {
            eprintln!("Failed to deserialize as QantoKeyPair: {}", e);
            // Try as QantoPQPrivateKey directly
            match bincode::deserialize::<QantoPQPrivateKey>(&bytes) {
                Ok(sk) => {
                    let raw_bytes = sk.as_bytes();
                    println!("{}", hex::encode(raw_bytes));
                }
                Err(e2) => {
                    eprintln!("Failed to deserialize as QantoPQPrivateKey: {}", e2);
                }
            }
        }
    }
    Ok(())
}
