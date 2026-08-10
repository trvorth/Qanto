use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use hkdf::Hkdf;
use qanto_core::protocol_identity::{
    account_address_v2, validate_account_address_v2, validator_id_v2, IDENTITY_VERSION_V2,
};
use qanto_core::qanto_native_crypto::{QantoPQPrivateKey, QantoPQPublicKey};
use rand::rngs::{OsRng, StdRng};
use rand::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Manager;
use tokio::sync::Mutex;
use zeroize::{ZeroizeOnDrop, Zeroizing};

const WALLET_LOCK_TTL: Duration = Duration::from_secs(15 * 60);
const MAX_FAILED_UNLOCKS: u32 = 3;
const UNLOCK_LOCKOUT: Duration = Duration::from_secs(60 * 60);
const LEGACY_KEYSTORE_VERSION: u32 = 1;
const IDENTITY_V2_KEYSTORE_VERSION: u32 = 2;
pub(crate) const WALLET_NETWORK_CHAIN_ID: u32 = 1234;
const KDF_CONTEXT_SALT: &[u8] = b"qanto-wallet-seed-v1";
const DILITHIUM3_DERIVE_INFO: &[u8] = b"qanto-wallet-dilithium3-v1";
const KEY_BINDING_CHALLENGE: &[u8] = b"QANTO\0WALLET\0ML-DSA-65\0KEY-BINDING\0V2";

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

#[derive(Serialize, Deserialize, ZeroizeOnDrop)]
struct WalletSecretPayload {
    mnemonic: String,
    // Present only when retaining a schema-v1 identity for migration/recovery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ed25519_secret_key_b64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ed25519_public_key_b64: Option<String>,
    dilithium3_secret_key_b64: String,
    dilithium3_public_key_b64: String,
    #[zeroize(skip)]
    address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[zeroize(skip)]
    identity_version: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[zeroize(skip)]
    network_chain_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[zeroize(skip)]
    validator_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[zeroize(skip)]
    legacy_ed25519_address: Option<String>,
}

#[derive(ZeroizeOnDrop)]
pub struct UnlockedWallet {
    #[zeroize(skip)]
    pub address: String,
    #[zeroize(skip)]
    pub validator_id: String,
    #[zeroize(skip)]
    pub identity_version: u16,
    #[zeroize(skip)]
    pub network_chain_id: u32,
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
    pub validator_id: String,
    #[zeroize(skip)]
    pub identity_version: u16,
    #[zeroize(skip)]
    pub network_chain_id: u32,
    #[zeroize(skip)]
    pub dilithium3_public_key: Vec<u8>,
    pub dilithium3_secret_key: Vec<u8>,
}

struct ValidatedWalletPayload {
    payload: WalletSecretPayload,
    dilithium3_public_key: Vec<u8>,
    dilithium3_secret_key: Vec<u8>,
    migrated_from_legacy: bool,
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

async fn read_keystore<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<KeystoreFile, String> {
    let path = keystore_path(app)?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("keystore read error: {e}"))?;
    serde_json::from_slice::<KeystoreFile>(&bytes).map_err(|e| format!("keystore parse error: {e}"))
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
    let bytes =
        serde_json::to_vec_pretty(data).map_err(|e| format!("keystore serialize error: {e}"))?;
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

fn is_supported_keystore_version(version: u32) -> bool {
    matches!(
        version,
        LEGACY_KEYSTORE_VERSION | IDENTITY_V2_KEYSTORE_VERSION
    )
}

fn validate_ml_dsa_keypair(secret_key: &[u8], public_key: &[u8]) -> Result<(), String> {
    let secret_key = QantoPQPrivateKey::from_bytes(secret_key)
        .map_err(|e| format!("invalid ML-DSA-65 secret key: {e}"))?;
    let public_key = QantoPQPublicKey::from_bytes(public_key)
        .map_err(|e| format!("invalid ML-DSA-65 public key: {e}"))?;
    let signature = secret_key
        .sign(KEY_BINDING_CHALLENGE)
        .map_err(|e| format!("invalid ML-DSA-65 secret key: {e}"))?;
    public_key
        .verify(KEY_BINDING_CHALLENGE, &signature)
        .map_err(|_| "ML-DSA-65 public/private key mismatch".to_string())
}

fn validate_legacy_ed25519_identity(payload: &WalletSecretPayload) -> Result<String, String> {
    let encoded_secret = payload
        .ed25519_secret_key_b64
        .as_deref()
        .ok_or_else(|| "legacy ed25519 secret key is missing".to_string())?;
    let encoded_public = payload
        .ed25519_public_key_b64
        .as_deref()
        .ok_or_else(|| "legacy ed25519 public key is missing".to_string())?;

    let ed_secret_bytes = Zeroizing::new(decode_b64(encoded_secret)?);
    if ed_secret_bytes.len() != 32 {
        return Err("invalid legacy ed25519 secret key length".to_string());
    }
    let mut ed_secret = Zeroizing::new([0u8; 32]);
    ed_secret.copy_from_slice(&ed_secret_bytes);

    let ed_public_bytes = decode_b64(encoded_public)?;
    if ed_public_bytes.len() != 32 {
        return Err("invalid legacy ed25519 public key length".to_string());
    }
    let mut ed_public = [0u8; 32];
    ed_public.copy_from_slice(&ed_public_bytes);

    let derived_ed_public = ed25519_dalek::SigningKey::from_bytes(&ed_secret)
        .verifying_key()
        .to_bytes();
    if derived_ed_public != ed_public {
        return Err("legacy ed25519 public/private key mismatch".to_string());
    }
    Ok(hex::encode(ed_public))
}

fn validate_wallet_payload(
    mut payload: WalletSecretPayload,
    keystore_version: u32,
) -> Result<ValidatedWalletPayload, String> {
    if !is_supported_keystore_version(keystore_version) {
        return Err("unsupported keystore version".to_string());
    }

    let pq_secret = Zeroizing::new(decode_b64(&payload.dilithium3_secret_key_b64)?);
    let pq_public = decode_b64(&payload.dilithium3_public_key_b64)?;
    validate_ml_dsa_keypair(&pq_secret, &pq_public)?;

    let expected_account = account_address_v2(WALLET_NETWORK_CHAIN_ID, &pq_public)
        .map_err(|e| format!("invalid wallet account identity: {e}"))?;
    let expected_validator = validator_id_v2(WALLET_NETWORK_CHAIN_ID, &pq_public)
        .map_err(|e| format!("invalid wallet validator identity: {e}"))?;

    let migrated_from_legacy = match keystore_version {
        LEGACY_KEYSTORE_VERSION => {
            let expected_legacy_address = validate_legacy_ed25519_identity(&payload)?;
            if payload.address != expected_legacy_address {
                return Err("legacy ed25519 address/key mismatch".to_string());
            }
            if payload.identity_version.is_some()
                || payload.network_chain_id.is_some()
                || payload.validator_id.is_some()
                || payload.legacy_ed25519_address.is_some()
            {
                return Err("legacy keystore contains inconsistent identity metadata".to_string());
            }

            payload.address = expected_account.clone();
            payload.identity_version = Some(IDENTITY_VERSION_V2);
            payload.network_chain_id = Some(WALLET_NETWORK_CHAIN_ID);
            payload.validator_id = Some(expected_validator.clone());
            payload.legacy_ed25519_address = Some(expected_legacy_address);
            true
        }
        IDENTITY_V2_KEYSTORE_VERSION => {
            if payload.identity_version != Some(IDENTITY_VERSION_V2) {
                return Err("wallet identity version mismatch".to_string());
            }
            if payload.network_chain_id != Some(WALLET_NETWORK_CHAIN_ID) {
                return Err("wallet network chain ID mismatch".to_string());
            }
            validate_account_address_v2(&payload.address, WALLET_NETWORK_CHAIN_ID, &pq_public)
                .map_err(|e| format!("wallet account address/key mismatch: {e}"))?;
            if payload.validator_id.as_deref() != Some(expected_validator.as_str()) {
                return Err("wallet validator ID/key mismatch".to_string());
            }
            match (
                payload.ed25519_secret_key_b64.as_ref(),
                payload.ed25519_public_key_b64.as_ref(),
                payload.legacy_ed25519_address.as_ref(),
            ) {
                (None, None, None) => {}
                (Some(_), Some(_), Some(declared_legacy_address)) => {
                    let expected_legacy_address = validate_legacy_ed25519_identity(&payload)?;
                    if declared_legacy_address != &expected_legacy_address {
                        return Err("legacy ed25519 migration identity mismatch".to_string());
                    }
                }
                _ => {
                    return Err("incomplete legacy ed25519 migration metadata".to_string());
                }
            }
            false
        }
        _ => return Err("unsupported keystore version".to_string()),
    };

    Ok(ValidatedWalletPayload {
        payload,
        dilithium3_public_key: pq_public,
        dilithium3_secret_key: pq_secret.to_vec(),
        migrated_from_legacy,
    })
}

pub(crate) fn validate_unlocked_identity(material: &UnlockedKeyMaterial) -> Result<(), String> {
    if material.identity_version != IDENTITY_VERSION_V2 {
        return Err("wallet identity version mismatch".to_string());
    }
    if material.network_chain_id != WALLET_NETWORK_CHAIN_ID {
        return Err("wallet network chain ID mismatch".to_string());
    }
    validate_ml_dsa_keypair(
        &material.dilithium3_secret_key,
        &material.dilithium3_public_key,
    )?;
    validate_account_address_v2(
        &material.address,
        material.network_chain_id,
        &material.dilithium3_public_key,
    )
    .map_err(|e| format!("wallet account address/key mismatch: {e}"))?;
    let expected_validator =
        validator_id_v2(material.network_chain_id, &material.dilithium3_public_key)
            .map_err(|e| format!("invalid wallet validator identity: {e}"))?;
    if material.validator_id != expected_validator {
        return Err("wallet validator ID/key mismatch".to_string());
    }
    Ok(())
}

fn build_wallet_payload(mnemonic: String) -> Result<WalletSecretPayload, String> {
    let mnemonic = bip39::Mnemonic::parse(mnemonic.trim())
        .map_err(|e| format!("mnemonic parse error: {e}"))?;
    let normalized_mnemonic = mnemonic.to_string();
    let seed = Zeroizing::new(mnemonic.to_seed(""));

    let dilithium_seed = Zeroizing::new(hkdf_expand_seed(seed.as_slice(), DILITHIUM3_DERIVE_INFO)?);

    let mut pq_rng = StdRng::from_seed(*dilithium_seed);
    let pq_sk = QantoPQPrivateKey::generate(&mut pq_rng);
    let pq_pk = pq_sk.public_key();

    let pq_secret_bytes = pq_sk.as_bytes().to_vec();
    let pq_public_bytes = pq_pk.as_bytes().to_vec();
    let address = account_address_v2(WALLET_NETWORK_CHAIN_ID, &pq_public_bytes)
        .map_err(|e| format!("wallet account identity derivation failed: {e}"))?;
    let validator_id = validator_id_v2(WALLET_NETWORK_CHAIN_ID, &pq_public_bytes)
        .map_err(|e| format!("wallet validator identity derivation failed: {e}"))?;

    Ok(WalletSecretPayload {
        mnemonic: normalized_mnemonic,
        ed25519_secret_key_b64: None,
        ed25519_public_key_b64: None,
        dilithium3_secret_key_b64: encode_b64(&pq_secret_bytes),
        dilithium3_public_key_b64: encode_b64(&pq_public_bytes),
        address,
        identity_version: Some(IDENTITY_VERSION_V2),
        network_chain_id: Some(WALLET_NETWORK_CHAIN_ID),
        validator_id: Some(validator_id),
        legacy_ed25519_address: None,
    })
}

fn generate_mnemonic() -> Result<String, String> {
    let mut entropy = [0u8; 32];
    OsRng.fill_bytes(&mut entropy);
    let m = bip39::Mnemonic::from_entropy(&entropy).map_err(|e| format!("mnemonic error: {e}"))?;
    Ok(m.to_string())
}

async fn load_validated_wallet_payload<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    password: &str,
) -> Result<ValidatedWalletPayload, String> {
    let file = read_keystore(app).await?;
    if !is_supported_keystore_version(file.version) {
        return Err("unsupported keystore version".to_string());
    }
    if file.kdf.algorithm != "argon2id" || file.aead.algorithm != "aes-256-gcm" {
        return Err("unsupported keystore cryptography".to_string());
    }

    let salt = decode_b64(&file.kdf.salt_b64)?;
    let nonce_bytes = decode_b64(&file.aead.nonce_b64)?;
    if nonce_bytes.len() != 12 {
        return Err("invalid keystore nonce".to_string());
    }
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&nonce_bytes);

    let ciphertext = decode_b64(&file.ciphertext_b64)?;
    let key = Zeroizing::new(derive_key(password, &salt)?);
    let plaintext = Zeroizing::new(aead_decrypt(&key, &nonce, &ciphertext)?);
    let payload: WalletSecretPayload = serde_json::from_slice(&plaintext)
        .map_err(|_| "invalid password or corrupted keystore".to_string())?;
    let validated = validate_wallet_payload(payload, file.version)?;

    if validated.migrated_from_legacy {
        let serialized = Zeroizing::new(
            serde_json::to_vec(&validated.payload)
                .map_err(|e| format!("wallet migration serialize error: {e}"))?,
        );
        let (ciphertext, nonce) = aead_encrypt(&key, &serialized)?;
        let migrated_file = KeystoreFile {
            version: IDENTITY_V2_KEYSTORE_VERSION,
            kdf: file.kdf.clone(),
            aead: AeadParams {
                algorithm: "aes-256-gcm".to_string(),
                nonce_b64: encode_b64(&nonce),
            },
            ciphertext_b64: encode_b64(&ciphertext),
        };
        write_keystore(app, &migrated_file).await?;
    }

    Ok(validated)
}

pub async fn has_wallet_impl<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<bool, String> {
    let path = keystore_path(app)?;
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let parsed = serde_json::from_slice::<KeystoreFile>(&bytes)
                .map_err(|e| format!("keystore parse error: {e}"))?;
            if !is_supported_keystore_version(parsed.version) {
                return Err("unsupported keystore version".to_string());
            }
            Ok(true)
        }
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
    let payload_json = Zeroizing::new(
        serde_json::to_vec(&payload).map_err(|e| format!("wallet payload serialize error: {e}"))?,
    );

    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let salt_vec = salt.to_vec();

    let key = Zeroizing::new(derive_key(password, &salt_vec)?);
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
        version: IDENTITY_V2_KEYSTORE_VERSION,
        kdf,
        aead,
        ciphertext_b64: encode_b64(&ciphertext),
    };

    write_keystore(app, &file).await?;
    Ok(payload.address.clone())
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
                return Err(
                    "wallet unlock is temporarily locked due to failed attempts".to_string()
                );
            }
        }
    }

    // Every explicit unlock decrypts and re-validates the persisted identity.
    // A cached session must never turn a wrong password into a successful unlock.
    let validated = load_validated_wallet_payload(app, password).await?;
    let payload = &validated.payload;
    let validator_id = payload
        .validator_id
        .clone()
        .ok_or_else(|| "wallet validator identity is missing".to_string())?;

    let unlocked = UnlockedWallet {
        address: payload.address.clone(),
        validator_id,
        identity_version: payload
            .identity_version
            .ok_or_else(|| "wallet identity version is missing".to_string())?,
        network_chain_id: payload
            .network_chain_id
            .ok_or_else(|| "wallet network chain ID is missing".to_string())?,
        dilithium3_public_key: validated.dilithium3_public_key,
        dilithium3_secret_key: validated.dilithium3_secret_key,
        mnemonic: payload.mnemonic.clone(),
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
    let validated = load_validated_wallet_payload(app, password).await?;
    Ok(validated.payload.mnemonic.clone())
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
        validator_id: u.validator_id.clone(),
        identity_version: u.identity_version,
        network_chain_id: u.network_chain_id,
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
    use pqcrypto_traits::sign::{PublicKey, SecretKey};
    use qanto_core::qanto_native_crypto::QANTO_PQ_PUBLIC_KEY_LENGTH;
    use std::ffi::OsString;
    use std::path::PathBuf;
    use tauri::test::mock_app;
    use zeroize::Zeroize;

    const TEST_MNEMONIC: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    const TEST_PASSWORD: &str = "Qanto#IdentityV2";
    const LEGACY_ED25519_DERIVE_INFO: &[u8] = b"qanto-wallet-ed25519-v1";
    static APP_DATA_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct AppDataDirGuard {
        previous: Option<OsString>,
        root: PathBuf,
    }

    impl Drop for AppDataDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
            if let Some(previous) = self.previous.take() {
                std::env::set_var("QANTO_APP_DATA_DIR", previous);
            } else {
                std::env::remove_var("QANTO_APP_DATA_DIR");
            }
        }
    }

    fn use_isolated_app_data_dir() -> AppDataDirGuard {
        let root = std::env::temp_dir().join(format!("qanto-wallet-qip001-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let previous = std::env::var_os("QANTO_APP_DATA_DIR");
        std::env::set_var("QANTO_APP_DATA_DIR", &root);
        AppDataDirGuard { previous, root }
    }

    fn downgrade_payload_to_legacy(payload: &mut WalletSecretPayload) -> String {
        let mnemonic = bip39::Mnemonic::parse(&payload.mnemonic).expect("legacy mnemonic");
        let seed = Zeroizing::new(mnemonic.to_seed(""));
        let ed_secret = Zeroizing::new(
            hkdf_expand_seed(seed.as_slice(), LEGACY_ED25519_DERIVE_INFO)
                .expect("legacy ed25519 seed"),
        );
        let ed_public = ed25519_dalek::SigningKey::from_bytes(&ed_secret)
            .verifying_key()
            .to_bytes();
        let legacy_address = hex::encode(ed_public);

        payload.ed25519_secret_key_b64 = Some(encode_b64(ed_secret.as_slice()));
        payload.ed25519_public_key_b64 = Some(encode_b64(&ed_public));
        payload.address = legacy_address.clone();
        payload.identity_version = None;
        payload.network_chain_id = None;
        payload.validator_id = None;
        payload.legacy_ed25519_address = None;
        legacy_address
    }

    #[test]
    fn deterministic_mnemonic_derivation_is_stable() {
        let first = build_wallet_payload(TEST_MNEMONIC.to_string()).expect("first payload");
        let second = build_wallet_payload(TEST_MNEMONIC.to_string()).expect("second payload");

        assert_eq!(first.mnemonic, second.mnemonic);
        assert_eq!(first.address, second.address);
        assert!(first.ed25519_secret_key_b64.is_none());
        assert!(first.ed25519_public_key_b64.is_none());
        assert_eq!(
            first.dilithium3_secret_key_b64,
            second.dilithium3_secret_key_b64
        );
        assert_eq!(
            first.dilithium3_public_key_b64,
            second.dilithium3_public_key_b64
        );
        assert_eq!(first.identity_version, Some(IDENTITY_VERSION_V2));
        assert_eq!(first.network_chain_id, Some(WALLET_NETWORK_CHAIN_ID));
        assert_eq!(first.validator_id, second.validator_id);
        assert!(first.legacy_ed25519_address.is_none());

        let public_key = decode_b64(&first.dilithium3_public_key_b64).expect("public key");
        validate_account_address_v2(&first.address, WALLET_NETWORK_CHAIN_ID, &public_key)
            .expect("address must be bound to the stored ML-DSA public key");
    }

    #[test]
    fn wallet_protocol_identity_defaults_match_golden_vectors() {
        let public_key = [7u8; QANTO_PQ_PUBLIC_KEY_LENGTH];

        assert_eq!(
            account_address_v2(WALLET_NETWORK_CHAIN_ID, &public_key).expect("account identity"),
            "d45c7ef8ce3931b25ca2ea68342551b642ca4f5e1ad64d29c8e8629ecb7d30c8"
        );
        assert_eq!(
            validator_id_v2(WALLET_NETWORK_CHAIN_ID, &public_key).expect("validator identity"),
            "4c42338df578b73b1969971a6b32a4de8031f203d22101c9b40a571a4ba8971d"
        );
    }

    #[test]
    fn legacy_payload_migrates_without_replacing_ml_dsa_key() {
        let mut payload = build_wallet_payload(TEST_MNEMONIC.to_string()).expect("payload");
        let original_public_key_b64 = payload.dilithium3_public_key_b64.clone();
        let legacy_address = downgrade_payload_to_legacy(&mut payload);

        let migrated = validate_wallet_payload(payload, LEGACY_KEYSTORE_VERSION)
            .expect("legacy payload should migrate");
        assert!(migrated.migrated_from_legacy);
        assert_eq!(
            migrated.payload.dilithium3_public_key_b64,
            original_public_key_b64
        );
        assert_eq!(
            migrated.payload.legacy_ed25519_address.as_deref(),
            Some(legacy_address.as_str())
        );
        assert_eq!(migrated.payload.identity_version, Some(IDENTITY_VERSION_V2));
        assert_eq!(
            migrated.payload.network_chain_id,
            Some(WALLET_NETWORK_CHAIN_ID)
        );
        validate_account_address_v2(
            &migrated.payload.address,
            WALLET_NETWORK_CHAIN_ID,
            &migrated.dilithium3_public_key,
        )
        .expect("migrated account identity");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn encrypted_legacy_wallet_migrates_and_cached_unlock_still_fails_closed() {
        let _env_lock = APP_DATA_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _data_dir = use_isolated_app_data_dir();
        let app = mock_app();
        let app_handle = app.handle();

        let created_address = create_wallet_impl(&app_handle, TEST_PASSWORD)
            .await
            .expect("create identity-v2 wallet");
        let created_file = read_keystore(&app_handle).await.expect("created keystore");
        assert_eq!(created_file.version, IDENTITY_V2_KEYSTORE_VERSION);

        let salt = decode_b64(&created_file.kdf.salt_b64).expect("KDF salt");
        let key = Zeroizing::new(derive_key(TEST_PASSWORD, &salt).expect("keystore key"));
        let decrypt_payload = |file: &KeystoreFile| -> WalletSecretPayload {
            let nonce_bytes = decode_b64(&file.aead.nonce_b64).expect("AEAD nonce");
            let nonce: [u8; 12] = nonce_bytes.try_into().expect("12-byte AEAD nonce");
            let ciphertext = decode_b64(&file.ciphertext_b64).expect("ciphertext");
            let plaintext = Zeroizing::new(
                aead_decrypt(&key, &nonce, &ciphertext).expect("decrypt test keystore"),
            );
            serde_json::from_slice(&plaintext).expect("wallet payload")
        };

        let mut legacy_payload = decrypt_payload(&created_file);
        assert!(legacy_payload.ed25519_secret_key_b64.is_none());
        assert!(legacy_payload.ed25519_public_key_b64.is_none());
        assert!(legacy_payload.legacy_ed25519_address.is_none());
        let original_ml_dsa_public_key = legacy_payload.dilithium3_public_key_b64.clone();
        downgrade_payload_to_legacy(&mut legacy_payload);

        let serialized_legacy =
            Zeroizing::new(serde_json::to_vec(&legacy_payload).expect("legacy payload JSON"));
        let (legacy_ciphertext, legacy_nonce) =
            aead_encrypt(&key, &serialized_legacy).expect("legacy payload encryption");
        let legacy_nonce_b64 = encode_b64(&legacy_nonce);
        let legacy_file = KeystoreFile {
            version: LEGACY_KEYSTORE_VERSION,
            kdf: created_file.kdf.clone(),
            aead: AeadParams {
                algorithm: "aes-256-gcm".to_string(),
                nonce_b64: legacy_nonce_b64.clone(),
            },
            ciphertext_b64: encode_b64(&legacy_ciphertext),
        };
        write_keystore(&app_handle, &legacy_file)
            .await
            .expect("write legacy keystore fixture");

        let session = WalletSessionState::default();
        let migrated_address = unlock_wallet_impl(&app_handle, &session, TEST_PASSWORD)
            .await
            .expect("unlock and migrate legacy wallet");
        assert_eq!(migrated_address, created_address);

        let wrong_password_error = unlock_wallet_impl(&app_handle, &session, "Wrong#Password123")
            .await
            .expect_err("cached session must not bypass password verification");
        assert!(wrong_password_error.contains("invalid password or corrupted keystore"));

        let migrated_file = read_keystore(&app_handle).await.expect("migrated keystore");
        assert_eq!(migrated_file.version, IDENTITY_V2_KEYSTORE_VERSION);
        assert_ne!(migrated_file.aead.nonce_b64, legacy_nonce_b64);
        let mut migrated_payload = decrypt_payload(&migrated_file);
        assert_eq!(
            migrated_payload.dilithium3_public_key_b64,
            original_ml_dsa_public_key
        );
        assert_eq!(migrated_payload.identity_version, Some(IDENTITY_VERSION_V2));
        assert_eq!(
            migrated_payload.network_chain_id,
            Some(WALLET_NETWORK_CHAIN_ID)
        );

        migrated_payload.address = "0".repeat(64);
        let serialized_tampered =
            Zeroizing::new(serde_json::to_vec(&migrated_payload).expect("tampered payload JSON"));
        let (tampered_ciphertext, tampered_nonce) =
            aead_encrypt(&key, &serialized_tampered).expect("tampered payload encryption");
        let tampered_file = KeystoreFile {
            version: IDENTITY_V2_KEYSTORE_VERSION,
            kdf: migrated_file.kdf,
            aead: AeadParams {
                algorithm: "aes-256-gcm".to_string(),
                nonce_b64: encode_b64(&tampered_nonce),
            },
            ciphertext_b64: encode_b64(&tampered_ciphertext),
        };
        write_keystore(&app_handle, &tampered_file)
            .await
            .expect("write tampered keystore");

        let tamper_error = unlock_wallet_impl(&app_handle, &session, TEST_PASSWORD)
            .await
            .expect_err("cached session must not bypass identity validation");
        assert!(tamper_error.contains("account address/key mismatch"));
    }

    #[test]
    fn identity_v2_validation_rejects_address_and_key_tampering() {
        let mut address_tampered =
            build_wallet_payload(TEST_MNEMONIC.to_string()).expect("address payload");
        address_tampered.address = "0".repeat(64);
        let address_error = validate_wallet_payload(address_tampered, IDENTITY_V2_KEYSTORE_VERSION)
            .err()
            .expect("tampered address must fail");
        assert!(address_error.contains("account address/key mismatch"));

        let mut key_tampered =
            build_wallet_payload(TEST_MNEMONIC.to_string()).expect("key payload");
        let mut public_key =
            decode_b64(&key_tampered.dilithium3_public_key_b64).expect("public key");
        public_key[0] ^= 1;
        key_tampered.dilithium3_public_key_b64 = encode_b64(&public_key);
        let key_error = validate_wallet_payload(key_tampered, IDENTITY_V2_KEYSTORE_VERSION)
            .err()
            .expect("tampered public key must fail");
        assert!(key_error.contains("public/private key mismatch"));

        let mut secret_tampered =
            build_wallet_payload(TEST_MNEMONIC.to_string()).expect("secret payload");
        let mut secret_key =
            decode_b64(&secret_tampered.dilithium3_secret_key_b64).expect("secret key");
        secret_key[0] ^= 1;
        secret_tampered.dilithium3_secret_key_b64 = encode_b64(&secret_key);
        secret_key.zeroize();
        let secret_error = validate_wallet_payload(secret_tampered, IDENTITY_V2_KEYSTORE_VERSION)
            .err()
            .expect("tampered private key must fail");
        assert!(
            secret_error.contains("secret key")
                || secret_error.contains("public/private key mismatch")
        );

        let mut metadata_tampered =
            build_wallet_payload(TEST_MNEMONIC.to_string()).expect("metadata payload");
        metadata_tampered.network_chain_id = Some(WALLET_NETWORK_CHAIN_ID + 1);
        let metadata_error =
            validate_wallet_payload(metadata_tampered, IDENTITY_V2_KEYSTORE_VERSION)
                .err()
                .expect("tampered chain ID must fail");
        assert!(metadata_error.contains("network chain ID mismatch"));
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
        assert_ne!(
            first.dilithium3_secret_key_b64,
            second.dilithium3_secret_key_b64
        );
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
