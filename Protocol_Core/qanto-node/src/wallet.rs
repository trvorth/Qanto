//! --- Qanto Wallet ---
//! v0.1.0 - Basic Wallet Implementation
//! This version implements a basic wallet with mnemonic generation, key derivation,
//! and encryption/decryption of wallet files.
//!
//! Features:
//! - Mnemonic generation and seed derivation
//! - Key derivation using Argon2id
//! - Encryption of wallet files using AES-256-GCM
//! - Decryption of wallet files using passphrase
//! - Storage of Ed25519 and post-quantum keys
//! - Serialization and deserialization of wallet files

use crate::post_quantum_crypto::{generate_pq_keypair, QantoPQPrivateKey, QantoPQPublicKey};
use crate::qanto_compat::ed25519_dalek::{
    Signer, SigningKey as Ed25519SigningKey, VerifyingKey as Ed25519VerifyingKey,
};
use aes_gcm::aead::{Aead, AeadCore, OsRng};
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use anyhow::Result;
use argon2::{password_hash::SaltString, Argon2};
use bip39::Mnemonic;
use rand::Rng;
use secrecy::{ExposeSecret, Secret, SecretVec};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use typenum::Unsigned;
use zeroize::{Zeroize, ZeroizeOnDrop};

// Bump the version to reflect the file format change from the security fix.
const WALLET_FILE_VERSION: u8 = 5;

#[derive(thiserror::Error, Debug)]
pub enum WalletError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    #[error("Encryption/decryption error: {0}")]
    Encryption(String),
    #[error("Passphrase hashing or verification error: {0}")]
    Passphrase(String),
    #[error("Cryptographic operation failed: {0}")]
    Crypto(String),
    #[error("Invalid wallet file format or version")]
    InvalidFormat,
    #[error("Invalid private key length or format")]
    InvalidKeyLength,
    #[error("Mnemonic generation or parsing error: {0}")]
    Mnemonic(#[from] bip39::Error),
    #[error("Failed to convert slice to array: {0}")]
    SliceToArrayError(#[from] std::array::TryFromSliceError),
    #[error("Hex decoding error: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("Post-quantum crypto error: {0}")]
    PqCrypto(String),
    #[error("UTF8 conversion error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("Atomic save failed: {0}")]
    AtomicSave(String),
}

#[derive(Serialize, Deserialize)]
struct PlainWalletData {
    signing_key: Vec<u8>,
    verifying_key: Vec<u8>,
    qr_secret_key: Vec<u8>,
    qr_public_key: Vec<u8>,
    mnemonic: String,
}

struct WalletData {
    signing_key: SecretVec<u8>,
    verifying_key: Vec<u8>,
    qr_secret_key: SecretVec<u8>,
    qr_public_key: Vec<u8>,
    mnemonic: Secret<String>,
}

impl Serialize for WalletData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        PlainWalletData {
            signing_key: self.signing_key.expose_secret().clone(),
            verifying_key: self.verifying_key.clone(),
            qr_secret_key: self.qr_secret_key.expose_secret().clone(),
            qr_public_key: self.qr_public_key.clone(),
            mnemonic: self.mnemonic.expose_secret().clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for WalletData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let plain = PlainWalletData::deserialize(deserializer)?;
        Ok(WalletData {
            signing_key: SecretVec::new(plain.signing_key),
            verifying_key: plain.verifying_key,
            qr_secret_key: SecretVec::new(plain.qr_secret_key),
            qr_public_key: plain.qr_public_key,
            mnemonic: Secret::new(plain.mnemonic),
        })
    }
}

impl Drop for WalletData {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl Zeroize for WalletData {
    fn zeroize(&mut self) {
        self.verifying_key.zeroize();
        self.qr_public_key.zeroize();
    }
}

#[derive(ZeroizeOnDrop)]
pub struct Wallet {
    data: WalletData,
}

impl Wallet {
    /// Creates a new wallet with random entropy.
    pub fn new() -> Result<Self, WalletError> {
        let mut entropy = [0u8; 32];
        rand::thread_rng().fill(&mut entropy);
        let mnemonic = Mnemonic::from_entropy(&entropy)?;
        Self::from_mnemonic(mnemonic.to_string().as_str())
    }

    /// Imports a wallet from a 32-byte or 64-byte hex-encoded Ed25519 private key.
    /// A new, non-deterministic PQ keypair will be generated.
    pub fn from_private_key(private_key_hex: &str) -> Result<Self, WalletError> {
        let key_bytes = hex::decode(private_key_hex.trim())?;

        if key_bytes.len() != 32 && key_bytes.len() != 64 {
            return Err(WalletError::InvalidKeyLength);
        }

        let key_array: [u8; 32] = key_bytes[..32]
            .try_into()
            .map_err(|_| WalletError::InvalidKeyLength)?;

        // FIX: Handle the Result from `from_bytes` before using the key.
        let signing_key = Ed25519SigningKey::from_bytes(&key_array)
            .map_err(|e| WalletError::Crypto(e.to_string()))?;
        let verifying_key = signing_key.verifying_key();

        let (pk, sk) =
            generate_pq_keypair(None).map_err(|e| WalletError::PqCrypto(e.to_string()))?;

        Ok(Self {
            data: WalletData {
                signing_key: SecretVec::new(signing_key.to_bytes().to_vec()),
                verifying_key: verifying_key.to_bytes().to_vec(),
                qr_secret_key: SecretVec::new(sk.as_bytes().to_vec()),
                qr_public_key: pk.as_bytes().to_vec(),
                mnemonic: Secret::new("".to_string()),
            },
        })
    }

    /// Creates a wallet deterministically from a BIP39 mnemonic phrase.
    pub fn from_mnemonic(mnemonic_phrase: &str) -> Result<Self, WalletError> {
        let mnemonic = Mnemonic::parse(mnemonic_phrase)?;
        let seed = mnemonic.to_seed("");

        let ed25519_seed: &[u8; 32] = seed.as_ref()[..32]
            .try_into()
            .map_err(|_| WalletError::InvalidKeyLength)?;
        // FIX: Handle the Result from `from_bytes` before using the key.
        let signing_key = Ed25519SigningKey::from_bytes(ed25519_seed)
            .map_err(|e| WalletError::Crypto(e.to_string()))?;
        let verifying_key = signing_key.verifying_key();

        let pq_seed: &[u8; 32] = seed.as_ref()[32..]
            .try_into()
            .map_err(|_| WalletError::InvalidKeyLength)?;
        let (pk, sk) = generate_pq_keypair(Some(*pq_seed))
            .map_err(|e| WalletError::PqCrypto(e.to_string()))?;

        Ok(Self {
            data: WalletData {
                signing_key: SecretVec::new(signing_key.to_bytes().to_vec()),
                verifying_key: verifying_key.to_bytes().to_vec(),
                qr_secret_key: SecretVec::new(sk.as_bytes().to_vec()),
                qr_public_key: pk.as_bytes().to_vec(),
                mnemonic: Secret::new(mnemonic.to_string()),
            },
        })
    }

    /// Returns the post-quantum (Dilithium) keypair.
    pub fn get_keypair(&self) -> Result<(QantoPQPrivateKey, QantoPQPublicKey), WalletError> {
        let sk_bytes = self.data.qr_secret_key.expose_secret();
        let pk_bytes = &self.data.qr_public_key;

        let sk = QantoPQPrivateKey::from_bytes(sk_bytes.as_slice()).map_err(|_| {
            WalletError::PqCrypto("Failed to deserialize post-quantum secret key".to_string())
        })?;
        let pk = QantoPQPublicKey::from_bytes(pk_bytes.as_slice()).map_err(|_| {
            WalletError::PqCrypto("Failed to deserialize post-quantum public key".to_string())
        })?;

        Ok((sk, pk))
    }

    /// Returns the Ed25519 signing key.
    pub fn get_signing_key(&self) -> Result<Ed25519SigningKey, WalletError> {
        let signing_key_bytes: &[u8] = self.data.signing_key.expose_secret();
        let key_array: [u8; 32] = signing_key_bytes
            .try_into()
            .map_err(|_| WalletError::InvalidKeyLength)?;
        // FIX: Handle the Result from `from_bytes`.
        let signing_key = Ed25519SigningKey::from_bytes(&key_array)
            .map_err(|e| WalletError::Crypto(e.to_string()))?;
        Ok(signing_key)
    }

    /// Signs a message with the Ed25519 private key.
    pub fn sign(
        &self,
        message: &[u8],
    ) -> Result<crate::qanto_compat::ed25519_dalek::Signature, WalletError> {
        let signing_key = self.get_signing_key()?;
        Ok(signing_key.sign(message))
    }

    /// Verifies a signature with the Ed25519 public key.
    pub fn verify(
        &self,
        message: &[u8],
        signature: &crate::qanto_compat::ed25519_dalek::Signature,
    ) -> Result<(), WalletError> {
        let key_array: [u8; 32] = self
            .data
            .verifying_key
            .as_slice()
            .try_into()
            .map_err(|_| WalletError::InvalidKeyLength)?;
        let verifying_key = Ed25519VerifyingKey::from_bytes(&key_array)
            .map_err(|e| WalletError::Crypto(e.to_string()))?;
        verifying_key
            .verify_strict(message, signature)
            .map_err(|e| WalletError::Crypto(e.to_string()))
    }

    /// Returns the public address (hex-encoded Ed25519 public key).
    pub fn address(&self) -> String {
        hex::encode(&self.data.verifying_key)
    }

    /// Returns a secret reference to the mnemonic phrase.
    pub fn mnemonic(&self) -> &Secret<String> {
        &self.data.mnemonic
    }

    /// Saves the wallet to a file, encrypted with a passphrase.
    pub fn save_to_file<P: AsRef<Path>>(
        &self,
        path: P,
        passphrase: &Secret<String>,
    ) -> Result<(), WalletError> {
        let plaintext = bincode::serialize(&self.data)?;
        let salt = SaltString::generate(&mut OsRng);

        let mut key_bytes = [0u8; 32];
        Argon2::default()
            .hash_password_into(
                passphrase.expose_secret().as_bytes(),
                // FIX: Provide the salt as a byte slice from the full salt string.
                salt.as_str().as_bytes(),
                &mut key_bytes,
            )
            .map_err(|e| WalletError::Passphrase(e.to_string()))?;

        let key: &Key<Aes256Gcm> = (&key_bytes).into();
        let cipher = Aes256Gcm::new(key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_slice())
            .map_err(|e| WalletError::Encryption(e.to_string()))?;

        let salt_bytes = salt.as_str().as_bytes();
        let salt_len = (salt_bytes.len() as u32).to_le_bytes();

        let mut file_contents = Vec::new();
        file_contents.push(WALLET_FILE_VERSION);
        file_contents.extend_from_slice(&salt_len);
        file_contents.extend_from_slice(salt_bytes);
        file_contents.extend_from_slice(&nonce);
        file_contents.extend_from_slice(&ciphertext);

        let path_ref = path.as_ref();
        let mut temp_path = path_ref.as_os_str().to_owned();
        temp_path.push(".tmp");
        let temp_path = PathBuf::from(temp_path);

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            file.set_permissions(perms)?;
        }
        file.write_all(&file_contents)?;
        file.sync_all()?;

        fs::rename(&temp_path, path_ref).map_err(|e| {
            WalletError::AtomicSave(format!(
                "Failed to rename temp wallet file '{}' to '{}': {}",
                temp_path.display(),
                path_ref.display(),
                e
            ))
        })?;

        Ok(())
    }

    /// Loads a wallet from an encrypted file.
    pub fn from_file<P: AsRef<Path>>(
        path: P,
        passphrase: &Secret<String>,
    ) -> Result<Self, WalletError> {
        let file_contents = fs::read(path)?;
        if file_contents.is_empty() {
            return Err(WalletError::InvalidFormat);
        }

        if file_contents[0] != WALLET_FILE_VERSION {
            return Err(WalletError::InvalidFormat);
        }

        if file_contents.len() < 5 {
            return Err(WalletError::InvalidFormat);
        }
        let salt_len = u32::from_le_bytes(file_contents[1..5].try_into().unwrap()) as usize;
        let salt_end = 5 + salt_len;

        let nonce_len = <Aes256Gcm as AeadCore>::NonceSize::to_usize();
        let nonce_end = salt_end + nonce_len;

        if file_contents.len() <= nonce_end {
            return Err(WalletError::InvalidFormat);
        }

        let salt_str = std::str::from_utf8(&file_contents[5..salt_end])?;
        // Use `new` to parse the full PHC string format of the salt.
        let salt_string = SaltString::from_b64(salt_str)
            .map_err(|e| WalletError::Passphrase(format!("Invalid salt format: {e}")))?;

        let nonce: &Nonce<<Aes256Gcm as AeadCore>::NonceSize> =
            (&file_contents[salt_end..nonce_end]).into();
        let ciphertext = &file_contents[nonce_end..];

        let mut key_bytes = [0u8; 32];
        Argon2::default()
            .hash_password_into(
                passphrase.expose_secret().as_bytes(),
                // FIX: Provide the salt as a byte slice from the full salt string.
                salt_string.as_str().as_bytes(),
                &mut key_bytes,
            )
            .map_err(|_| WalletError::Passphrase("Password verification failed.".to_string()))?;

        let key: &Key<Aes256Gcm> = (&key_bytes).into();
        let cipher = Aes256Gcm::new(key);

        let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|_| {
            WalletError::Encryption("Decryption failed. Check password.".to_string())
        })?;

        let data: WalletData = bincode::deserialize(&plaintext)?;
        Ok(Self { data })
    }

    /// Changes the password of an existing encrypted wallet file.
    pub fn change_password<P: AsRef<Path>>(
        path: P,
        old_passphrase: &Secret<String>,
        new_passphrase: &Secret<String>,
    ) -> Result<(), WalletError> {
        let wallet = Self::from_file(&path, old_passphrase)?;
        wallet.save_to_file(&path, new_passphrase)?;
        Ok(())
    }
}
