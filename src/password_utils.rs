use anyhow::{anyhow, Result};
use secrecy::{ExposeSecret, SecretString};
use std::io::{self, Write};

/// Unified password reading function that ensures consistent behavior across all binaries.
/// This function:
/// 1. First checks for WALLET_PASSWORD environment variable
/// 2. Trims whitespace and newlines from environment variable
/// 3. Falls back to interactive prompt if no environment variable
/// 4. Validates that passwords are not empty
/// 5. Supports password confirmation for new wallet creation
pub fn prompt_for_password(confirm: bool, message: Option<&str>) -> Result<SecretString> {
    // Check for WALLET_PASSWORD environment variable first
    if let Ok(env_password) = std::env::var("WALLET_PASSWORD") {
        // Trim whitespace and newlines from environment variable
        let trimmed_password = env_password.trim().to_string();

        // Validate that the password is not empty after trimming
        if trimmed_password.is_empty() {
            // Match integration test expectation exactly
            return Err(anyhow!(
                "WALLET_PASSWORD environment variable is set but empty!"
            ));
        }

        println!("Using password from WALLET_PASSWORD environment variable.");
        return Ok(SecretString::new(trimmed_password));
    }

    // Fallback to interactive prompt if no environment variable
    let prompt_message = message.unwrap_or("Enter wallet password:");
    print!("{prompt_message} ");
    io::stdout().flush()?;

    let password: SecretString = rpassword::read_password()?.into();

    // Validate that the password is not empty
    if password.expose_secret().trim().is_empty() {
        return Err(anyhow!("Password cannot be empty."));
    }

    if confirm {
        print!("Confirm wallet password: ");
        io::stdout().flush()?;
        let confirmation: SecretString = rpassword::read_password()?.into();

        // Compare trimmed versions to handle any whitespace inconsistencies
        if password.expose_secret().trim() != confirmation.expose_secret().trim() {
            return Err(anyhow!("Passwords do not match."));
        }
    }

    // Return the trimmed password to ensure consistency
    Ok(SecretString::new(
        password.expose_secret().trim().to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_env_password_trimming() {
        // Test that environment variable passwords are properly trimmed
        env::set_var("WALLET_PASSWORD", "  test_password  \n");
        let result = prompt_for_password(false, None);
        assert!(result.is_ok());
        let password = result.unwrap();
        assert_eq!(password.expose_secret(), "test_password");
        env::remove_var("WALLET_PASSWORD");
    }

    #[test]
    fn test_empty_env_password() {
        // Test that empty environment variable passwords are rejected
        env::set_var("WALLET_PASSWORD", "   \n  ");
        let result = prompt_for_password(false, None);
        assert!(result.is_err());
        env::remove_var("WALLET_PASSWORD");
    }
}
