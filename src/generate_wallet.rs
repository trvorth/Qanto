// src/generate_wallet.rs

use crate::{password_utils::prompt_for_password, wallet::Wallet};
use clap::{Arg, Command};

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("generate_wallet")
        .about("Generate a new Qanto wallet")
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Output wallet file path")
                .default_value("wallet.key"),
        )
        .get_matches();

    let output_path = matches.get_one::<String>("output").unwrap();

    println!("Generating new wallet...");

    // Use unified password reading function for consistency
    let password = prompt_for_password(true, Some("Enter password for new wallet:"))?;

    let wallet = Wallet::new()?;
    wallet.save_to_file(output_path, &password)?;

    println!(
        "New wallet saved to '{}'. Address: {}",
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
