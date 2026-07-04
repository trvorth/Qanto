use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use hkdf::Hkdf;
use qanto_core::qanto_native_crypto::QantoPQPrivateKey;
use rand::rngs::{OsRng, StdRng};
use rand::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Manager;
use tokio::sync::Mutex;
use zeroize::{Zeroize, ZeroizeOnDrop};

const WALLET_LOCK_TTL: Duration = Duration::from_secs(15 * 60);
const MAX_FAILED_UNLOCKS: u32 = 3;
const UNLOCK_LOCKOUT: Duration = Duration::from_secs(60 * 60);
const KDF_CONTEXT_SALT: &[u8] = b"qanto-wallet-seed-v1";
const ED25519_DERIVE_INFO: &[u8] = b"qanto-wallet-ed25519-v1";
const DILITHIUM3_DERIVE_INFO: &[u8] = b"qanto-wallet-dilithium3-v1";

#[derive(Clone, Serialize, Deserialize)]
struct KeystoreFile {
    version: u32,
    kdf: KdfParams,
    aead: AeadParams,
    ciphertext_b64: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct KdfParams {
    algorithm: String,
    salt_b64: String,
    m_cost_kib: u32,
    t_cost: u32,
    p_cost: u32,
    key_len: u32,
}

#[derive(Clone, Serialize, Deserialize)]
struct AeadParams {
    algorithm: String,
    nonce_b64: String,
}

#[derive(Serialize, Deserialize)]
struct WalletSecretPayload {
    mnemonic: String,
    ed25519_secret_key_b64: String,
    ed25519_public_key_b64: String,
    dilithium3_secret_key_b64: String,
    dilithium3_public_key_b64: String,
    address: String,
}

#[derive(ZeroizeOnDrop)]
pub struct UnlockedWallet {
    #[zeroize(skip)]
    pub address: String,
    #[zeroize(skip)]
    pub ed25519_public_key: [u8; 32],
    pub ed25519_secret_key: [u8; 32],
    #[zeroize(skip)]
    pub dilithium3_public_key: Vec<u8>,
    pub dilithium3_secret_key: Vec<u8>,
    pub mnemonic: String,
}

#[derive(ZeroizeOnDrop)]
pub struct UnlockedKeyMaterial {
    #[zeroize(skip)]
    pub address: String,
    #[zeroize(skip)]
    pub ed25519_public_key: [u8; 32],
    pub ed25519_secret_key: [u8; 32],
    #[zeroize(skip)]
    pub dilithium3_public_key: Vec<u8>,
    pub dilithium3_secret_key: Vec<u8>,
}

pub struct WalletSessionState {
    inner: Arc<Mutex<WalletSessionInner>>,
}

struct WalletSessionInner {
    unlocked: Option<UnlockedWallet>,
    unlocked_until: Option<Instant>,
    failed_unlocks: u32,
    unlock_lockout_until: Option<Instant>,
}

impl Default for WalletSessionState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(WalletSessionInner {
                unlocked: None,
                unlocked_until: None,
                failed_unlocks: 0,
                unlock_lockout_until: None,
            })),
        }
    }
}

impl Clone for WalletSessionState {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

fn data_dir<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, String> {
    if let Ok(override_dir) = std::env::var("QANTO_APP_DATA_DIR") {
        let trimmed = override_dir.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    app.path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))
}

pub fn keystore_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, String> {
    let mut p = data_dir(app)?;
    p.push("qanto");
    p.push("keystore.json");
    Ok(p)
}

fn validate_password_strength(password: &str) -> Result<(), String> {
    let p = password;
    if p.len() < 8 {
        return Err("password too short: minimum 8 characters".to_string());
    }
    let has_letter = p.chars().any(|c| c.is_ascii_alphabetic());
    let has_digit = p.chars().any(|c| c.is_ascii_digit());
    let has_special = p.chars().any(|c| !c.is_ascii_alphanumeric());
    if !(has_letter && has_digit && has_special) {
        return Err("password must include letters, digits, and special characters".to_string());
    }
    Ok(())
}

fn argon2_params() -> Result<Params, String> {
    Params::new(64 * 1024, 3, 1, Some(32)).map_err(|e| format!("argon2 params invalid: {e}"))
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let params = argon2_params()?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|e| format!("argon2 derivation failed: {e}"))?;
    Ok(out)
}

fn aead_encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<(Vec<u8>, [u8; 12]), String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| "encryption failed".to_string())?;
    Ok((ct, nonce))
}

fn aead_decrypt(key: &[u8; 32], nonce: &[u8; 12], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| "invalid password or corrupted keystore".to_string())
}

fn base64_engine() -> base64::engine::general_purpose::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

async fn read_keystore<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<KeystoreFile, String> {
    let path = keystore_path(app)?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("keystore read error: {e}"))?;
    serde_json::from_slice::<KeystoreFile>(&bytes)
        .map_err(|e| format!("keystore parse error: {e}"))
}

async fn write_keystore<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    data: &KeystoreFile,
) -> Result<(), String> {
    let path = keystore_path(app)?;
    let dir = path
        .parent()
        .ok_or_else(|| "invalid keystore path".to_string())?;
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| format!("keystore dir create error: {e}"))?;
    let bytes = serde_json::to_vec_pretty(data).map_err(|e| format!("keystore serialize error: {e}"))?;
    tokio::fs::write(&path, bytes)
        .await
        .map_err(|e| format!("keystore write error: {e}"))?;
    Ok(())
}

fn decode_b64(s: &str) -> Result<Vec<u8>, String> {
    base64_engine()
        .decode(s)
        .map_err(|_| "invalid base64 in keystore".to_string())
}

fn encode_b64(bytes: &[u8]) -> String {
    base64_engine().encode(bytes)
}

fn hkdf_expand_seed(seed: &[u8], info: &[u8]) -> Result<[u8; 32], String> {
    let hkdf = Hkdf::<Sha256>::new(Some(KDF_CONTEXT_SALT), seed);
    let mut output = [0u8; 32];
    hkdf.expand(info, &mut output)
        .map_err(|_| "hkdf expansion failed".to_string())?;
    Ok(output)
}

fn build_wallet_payload(mnemonic: String) -> Result<WalletSecretPayload, String> {
    let mnemonic = bip39::Mnemonic::parse(mnemonic.trim())
        .map_err(|e| format!("mnemonic parse error: {e}"))?;
    let normalized_mnemonic = mnemonic.to_string();
    let mut seed = mnemonic.to_seed("");

    let mut ed_secret = hkdf_expand_seed(seed.as_slice(), ED25519_DERIVE_INFO)?;
    let mut dilithium_seed = hkdf_expand_seed(seed.as_slice(), DILITHIUM3_DERIVE_INFO)?;
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&ed_secret);
    let verifying_key = signing_key.verifying_key();
    let ed_public = verifying_key.to_bytes();
    let address = hex::encode(ed_public);

    let mut pq_rng = StdRng::from_seed(dilithium_seed);
    let pq_sk = QantoPQPrivateKey::generate(&mut pq_rng);
    let pq_pk = pq_sk.public_key();

    let pq_secret_bytes = pq_sk.as_bytes().to_vec();
    let pq_public_bytes = pq_pk.as_bytes().to_vec();

    seed.zeroize();
    dilithium_seed.zeroize();

    Ok(WalletSecretPayload {
        mnemonic: normalized_mnemonic,
        ed25519_secret_key_b64: encode_b64(&ed_secret),
        ed25519_public_key_b64: encode_b64(&ed_public),
        dilithium3_secret_key_b64: encode_b64(&pq_secret_bytes),
        dilithium3_public_key_b64: encode_b64(&pq_public_bytes),
        address,
    })
}

fn generate_mnemonic() -> Result<String, String> {
    let mut entropy = [0u8; 32];
    OsRng.fill_bytes(&mut entropy);
    let m = bip39::Mnemonic::from_entropy(&entropy).map_err(|e| format!("mnemonic error: {e}"))?;
    Ok(m.to_string())
}

pub async fn has_wallet_impl<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<bool, String> {
    let path = keystore_path(app)?;
    match tokio::fs::read(&path).await {
        Ok(bytes) => match serde_json::from_slice::<KeystoreFile>(&bytes) {
            Ok(parsed) => Ok(parsed.version == 1),
            Err(_) => Ok(false),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("keystore access error: {e}")),
    }
}

pub async fn create_wallet_impl<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    password: &str,
) -> Result<String, String> {
    validate_password_strength(password)?;
    if has_wallet_impl(app).await? {
        return Err("wallet already exists".to_string());
    }

    let mnemonic = generate_mnemonic()?;
    let payload = build_wallet_payload(mnemonic)?;
    let payload_json =
        serde_json::to_vec(&payload).map_err(|e| format!("wallet payload serialize error: {e}"))?;

    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let salt_vec = salt.to_vec();

    let key = derive_key(password, &salt_vec)?;
    let (ciphertext, nonce) = aead_encrypt(&key, &payload_json)?;

    let params = argon2_params()?;
    let kdf = KdfParams {
        algorithm: "argon2id".to_string(),
        salt_b64: encode_b64(&salt_vec),
        m_cost_kib: params.m_cost(),
        t_cost: params.t_cost(),
        p_cost: params.p_cost(),
        key_len: 32,
    };
    let aead = AeadParams {
        algorithm: "aes-256-gcm".to_string(),
        nonce_b64: encode_b64(&nonce),
    };

    let file = KeystoreFile {
        version: 1,
        kdf,
        aead,
        ciphertext_b64: encode_b64(&ciphertext),
    };

    write_keystore(app, &file).await?;
    Ok(payload.address)
}

pub async fn unlock_wallet_impl(
    app: &tauri::AppHandle<impl tauri::Runtime>,
    state: &WalletSessionState,
    password: &str,
) -> Result<String, String> {
    let now = Instant::now();
    {
        let inner = state.inner.lock().await;
        if let Some(until) = inner.unlock_lockout_until {
            if now < until {
                return Err("wallet unlock is temporarily locked due to failed attempts".to_string());
            }
        }
        if let Some(until) = inner.unlocked_until {
            if now < until && inner.unlocked.is_some() {
                return Ok(inner
                    .unlocked
                    .as_ref()
                    .map(|u| u.address.clone())
                    .unwrap_or_default());
            }
        }
    }

    let file = read_keystore(app).await?;
    if file.version != 1 {
        return Err("unsupported keystore version".to_string());
    }

    let salt = decode_b64(&file.kdf.salt_b64)?;
    let nonce_bytes = decode_b64(&file.aead.nonce_b64)?;
    if nonce_bytes.len() != 12 {
        return Err("invalid keystore nonce".to_string());
    }
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&nonce_bytes);

    let ct = decode_b64(&file.ciphertext_b64)?;
    let key = derive_key(password, &salt)?;
    let mut pt = aead_decrypt(&key, &nonce, &ct)?;

    let payload: WalletSecretPayload = serde_json::from_slice(&pt)
        .map_err(|_| "invalid password or corrupted keystore".to_string())?;
    pt.zeroize();

    let ed_sk = decode_b64(&payload.ed25519_secret_key_b64)?;
    if ed_sk.len() != 32 {
        return Err("invalid ed25519 secret key length".to_string());
    }
    let mut ed_sk_arr = [0u8; 32];
    ed_sk_arr.copy_from_slice(&ed_sk);

    let ed_pk = decode_b64(&payload.ed25519_public_key_b64)?;
    if ed_pk.len() != 32 {
        return Err("invalid ed25519 public key length".to_string());
    }
    let mut ed_pk_arr = [0u8; 32];
    ed_pk_arr.copy_from_slice(&ed_pk);

    let pq_sk = decode_b64(&payload.dilithium3_secret_key_b64)?;
    let pq_pk = decode_b64(&payload.dilithium3_public_key_b64)?;

    let unlocked = UnlockedWallet {
        address: payload.address.clone(),
        ed25519_public_key: ed_pk_arr,
        ed25519_secret_key: ed_sk_arr,
        dilithium3_public_key: pq_pk,
        dilithium3_secret_key: pq_sk,
        mnemonic: payload.mnemonic,
    };

    let mut inner = state.inner.lock().await;
    inner.unlocked = Some(unlocked);
    inner.unlocked_until = Some(Instant::now() + WALLET_LOCK_TTL);
    inner.failed_unlocks = 0;
    inner.unlock_lockout_until = None;

    let addr = inner
        .unlocked
        .as_ref()
        .map(|u| u.address.clone())
        .unwrap_or_default();

    let state_clone = state.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(WALLET_LOCK_TTL).await;
        let now = Instant::now();
        let mut inner = state_clone.inner.lock().await;
        if let Some(until) = inner.unlocked_until {
            if now >= until {
                inner.unlocked = None;
                inner.unlocked_until = None;
            }
        }
    });

    Ok(addr)
}

pub async fn export_mnemonic_impl<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    password: &str,
) -> Result<String, String> {
    let file = read_keystore(app).await?;
    if file.version != 1 {
        return Err("unsupported keystore version".to_string());
    }
    let salt = decode_b64(&file.kdf.salt_b64)?;
    let nonce_bytes = decode_b64(&file.aead.nonce_b64)?;
    if nonce_bytes.len() != 12 {
        return Err("invalid keystore nonce".to_string());
    }
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&nonce_bytes);
    let ct = decode_b64(&file.ciphertext_b64)?;

    let key = derive_key(password, &salt)?;
    let mut pt = aead_decrypt(&key, &nonce, &ct)?;
    let payload: WalletSecretPayload = serde_json::from_slice(&pt)
        .map_err(|_| "invalid password or corrupted keystore".to_string())?;
    pt.zeroize();
    Ok(payload.mnemonic)
}

pub async fn record_failed_unlock(state: &WalletSessionState) {
    let mut inner = state.inner.lock().await;
    inner.failed_unlocks = inner.failed_unlocks.saturating_add(1);
    if inner.failed_unlocks >= MAX_FAILED_UNLOCKS {
        inner.unlock_lockout_until = Some(Instant::now() + UNLOCK_LOCKOUT);
    }
}

pub async fn get_key_material(state: &WalletSessionState) -> Option<UnlockedKeyMaterial> {
    let now = Instant::now();
    let mut inner = state.inner.lock().await;
    if let Some(until) = inner.unlocked_until {
        if now >= until {
            inner.unlocked = None;
            inner.unlocked_until = None;
            return None;
        }
    }
    inner.unlocked.as_ref().map(|u| UnlockedKeyMaterial {
        address: u.address.clone(),
        ed25519_public_key: u.ed25519_public_key,
        ed25519_secret_key: u.ed25519_secret_key,
        dilithium3_public_key: u.dilithium3_public_key.clone(),
        dilithium3_secret_key: u.dilithium3_secret_key.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "pqcrypto-legacy")]
    use pqcrypto_dilithium::dilithium3;
    #[cfg(feature = "pqcrypto-legacy")]
    use pqcrypto_traits::sign::{DetachedSignature, PublicKey, SecretKey};

    const TEST_MNEMONIC: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    #[test]
    fn deterministic_mnemonic_derivation_is_stable() {
        let first = build_wallet_payload(TEST_MNEMONIC.to_string()).expect("first payload");
        let second = build_wallet_payload(TEST_MNEMONIC.to_string()).expect("second payload");

        assert_eq!(first.mnemonic, second.mnemonic);
        assert_eq!(first.address, second.address);
        assert_eq!(first.ed25519_secret_key_b64, second.ed25519_secret_key_b64);
        assert_eq!(first.ed25519_public_key_b64, second.ed25519_public_key_b64);
        assert_eq!(first.dilithium3_secret_key_b64, second.dilithium3_secret_key_b64);
        assert_eq!(first.dilithium3_public_key_b64, second.dilithium3_public_key_b64);
    }

    #[test]
    fn different_mnemonics_produce_distinct_keys() {
        let first = build_wallet_payload(TEST_MNEMONIC.to_string()).expect("first payload");
        let second = build_wallet_payload(
            "legal winner thank year wave sausage worth useful legal winner thank yellow"
                .to_string(),
        )
        .expect("second payload");

        assert_ne!(first.address, second.address);
        assert_ne!(first.ed25519_secret_key_b64, second.ed25519_secret_key_b64);
        assert_ne!(first.dilithium3_secret_key_b64, second.dilithium3_secret_key_b64);
    }

    #[cfg(feature = "pqcrypto-legacy")]
    #[test]
    fn derived_dilithium_keys_work_with_standard_verifier() {
        let payload = build_wallet_payload(TEST_MNEMONIC.to_string()).expect("payload");
        let public_key_bytes = decode_b64(&payload.dilithium3_public_key_b64).expect("public key");
        let secret_key_bytes = decode_b64(&payload.dilithium3_secret_key_b64).expect("secret key");

        let public_key =
            dilithium3::PublicKey::from_bytes(&public_key_bytes).expect("standard public key");
        let secret_key =
            dilithium3::SecretKey::from_bytes(&secret_key_bytes).expect("standard secret key");

        let message = b"qanto-wallet-deterministic-dilithium3";
        let signature = dilithium3::detached_sign(message, &secret_key);

        dilithium3::verify_detached_signature(&signature, message, &public_key)
            .expect("standard dilithium3 verification");
    }
}
