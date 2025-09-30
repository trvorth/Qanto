use clap::{Arg, Command};
use qanto::wallet::Wallet;
use secrecy::SecretString;
use std::io::{self, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    // Prompt for password
    print!("Enter password for new wallet: ");
    io::stdout().flush()?;
    let password = rpassword::read_password()?;

    print!("Confirm password: ");
    io::stdout().flush()?;
    let confirm_password = rpassword::read_password()?;

    if password != confirm_password {
        eprintln!("Passwords do not match!");
        std::process::exit(1);
    }

    let password = SecretString::new(password);
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
