# The Ultimate QantoWallet Guide

Welcome to your central command for managing Qanto assets! The `qantowallet` is a powerful and secure command-line tool that lets you create, import, and inspect your Qanto wallets with ease.

This guide is designed for users of all skill levels, providing clear, step-by-step instructions for every feature.

---

## Prerequisites

Before you begin, make sure you have the Qanto node software compiled on your system. If you haven't done so, navigate to the project's root directory and run the following command:

```bash
cargo build
```

---

## 1. Creating a New Wallet

This is the first step for any new user. Generating a wallet creates a unique address for you on the Qanto network and, most importantly, the secret keys needed to control your funds.

**Command:**
```bash
cargo run --bin qantowallet -- generate
```

**What Happens:**

1.  **Password Prompt:** You will be asked to enter a strong password. This password encrypts your wallet file, making it useless to anyone who doesn't have it.
    ```
    Enter a strong password to encrypt your new wallet:
    Confirm password:
    ```
2.  **Wallet Generation:** The tool securely generates a new wallet.
3.  **Critical Information:** The CLI will display your new wallet's public address and your **24-word mnemonic recovery phrase**.

**Example Output:**
```
✅ New wallet generated successfully!
   Saved to: "wallet.key"

🔒 Security Information:
   Address: 74fd2aae70ae8e0930b87a3dcb3b77f5b71d956659849f067360d3486604db41

⚠️ CRITICAL: Please write down this mnemonic phrase and store it in a secure, offline location.
   This is the ONLY way to recover your wallet.

   Mnemonic Phrase: orchard benefit ... [22 more words] ... ivory
```

> ### Security First!
> * **Write down your mnemonic phrase** on paper and store it in multiple safe, offline locations.
> * **Never** store your mnemonic phrase digitally (e.g., in a text file, email, or cloud storage). Anyone who gets this phrase can steal your funds.
> * Think of your password as your key and your mnemonic as the master key.

---

## 2. Checking Your Wallet's Information

Once your wallet is created, you can easily check its details, including your public address and other key info.

**Command:**
```bash
cargo run --release --bin qantowallet -- show-keys
```

**What Happens:**

1.  **Password Prompt:** You'll be asked for the password you created earlier to decrypt the wallet file.
2.  **Display Information:** The tool will display your public address, the path to your wallet file, and other details.

**Example Output:**
```
Enter password for wallet 'wallet.key':

--- Qanto Wallet Information ---
Address:          74fd2aae70ae8e0930b87a3dcb3b77f5b71d956659849f067360d3486604db41
Wallet File:      "wallet.key"
Is Encrypted:     true
Private Key:      [Your secret private key will be displayed here]
Mnemonic Phrase:  [Your 24-word mnemonic phrase will be displayed here]
---------------------------------
```

---

## 3. Importing an Existing Wallet

If you have a 24-word mnemonic phrase from a previous wallet, you can use it to restore access to your funds on a new machine.

**Command:**
```bash
cargo run --bin qantowallet -- import --mnemonic "your 24 word phrase goes here inside quotes"
```

**Note:** Replace `"your 24 word phrase goes here inside quotes"` with your actual mnemonic phrase.

**What Happens:**

1.  **Password Prompt:** You will be asked to set a *new* password for the wallet file on this specific machine.
2.  **Wallet Restoration:** The tool will derive your keys and address from the mnemonic phrase.
3.  **File Creation:** It will save a new `wallet.key` file, encrypted with your new password.

**Example Output:**
```
Importing wallet from mnemonic phrase...
Enter a strong password to encrypt the imported wallet:
Confirm password:

✅ Wallet imported and saved successfully!
   Saved to: "wallet.key"
   Address:  74fd2aae70ae8e0930b87a3dcb3b77f5b71d956659849f067360d3486604db41
```

---

## 4. Using a Custom Wallet Path

By default, the wallet file is named `wallet.key`. You can specify a different name or location for any of the commands using the `--wallet-path` flag.

**Example: Generate a wallet named `my_qanto_backup.wallet`**
```bash
cargo run --bin qantowallet -- generate --wallet-path my_qanto_backup.wallet
```

**Example: Get info from a custom wallet file**
```bash
cargo run --bin qantowallet -- info --wallet-path my_qanto_backup.wallet
```

**Example: Import a wallet to a custom file**
```bash
cargo run --bin qantowallet -- import --mnemonic "your phrase..." --wallet-path my_imported.wallet
```

This guide covers all the essential functions of the `qantowallet` CLI. With these commands, you have complete control over your Qanto wallets in a secure and straightforward way.
