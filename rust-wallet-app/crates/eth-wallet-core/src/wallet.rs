//! WalletManager — encrypted-mnemonic wallet store for the v0.2 `eth` CLI.
//!
//! Per Issue #301 Task 2 acceptance:
//!   * Each wallet = encrypted mnemonic (Argon2id + AES-256-GCM, F5/F6
//!     mirror) + UUID `wallet_id` + user-facing `--name`. Per #297 B1
//!     (named wallets only, UUID is internal) and B2 (Argon2id from day 1).
//!   * Persistence at `<base_dir>/wallets/<network>/<wallet_id>.enc`
//!     (JSON blob).
//!   * Per-call unlock: `unlock(wallet_id, password) -> Zeroizing<Mnemonic>`
//!     re-derives the in-memory key from the encrypted blob.
//!
//! Task 4 will replace the local `WalletError` with the 17-variant Error
//! enum; the public method signatures are forward-compatible.
//!
//! Task 4 will replace the local `WalletError` with the 17-variant Error
//! enum; the public method signatures are forward-compatible.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use alloy_primitives::Address;
use alloy_signer_local::{MnemonicBuilder, PrivateKeySigner};
use bip39::{Language, Mnemonic};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::crypto::{self, CryptoError, KEY_LEN, NONCE_LEN, SALT_LEN};
use crate::mnemonic;

/// Default XDG layout (operator-side). Overridable via `open_at` (tests).
const PROJECT_QUALIFIER: &str = "btc";
const PROJECT_ORG: &str = "nhitran";
const PROJECT_APP: &str = "eth-wallet-core";

/// Logical network for a wallet (Task 10 CLI will pass --network).
/// Default = Sepolia for v0.2 (cross-cutting testnet default per #291).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Mainnet,
    Sepolia,
    Anvil,
}

impl Network {
    pub fn default_v0_2() -> Self {
        Network::Sepolia
    }

    fn as_dir_name(&self) -> &'static str {
        match self {
            Network::Mainnet => "mainnet",
            Network::Sepolia => "sepolia",
            Network::Anvil => "anvil",
        }
    }
}

/// Persisted encrypted blob on disk. Salt + nonce + ciphertext in a JSON file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedBlob {
    pub v: u32,      // schema version for forward-compat
    pub kdf: String, // "argon2id" (only one supported today)
    pub salt_hex: String,
    pub nonce_hex: String,
    pub ciphertext: Vec<u8>,
}

impl EncryptedBlob {
    fn new(salt: [u8; SALT_LEN], nonce: [u8; NONCE_LEN], ciphertext: Vec<u8>) -> Self {
        Self {
            v: 1,
            kdf: "argon2id".to_string(),
            salt_hex: hex::encode(salt),
            nonce_hex: hex::encode(nonce),
            ciphertext,
        }
    }
}

/// In-memory cached metadata for a wallet. Does NOT include the mnemonic
/// (mnemonic stays in `Zeroizing<Mnemonic>` after `unlock`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletMeta {
    pub wallet_id: Uuid,
    pub name: String,
    pub network: Network,
    pub address: Address,
    pub created_at_secs: u64,
}

/// Summary returned by `list_wallets`.
#[derive(Debug, Clone, Serialize)]
pub struct WalletInfo {
    pub wallet_id: Uuid,
    pub name: String,
    pub network: Network,
    pub address: Address,
    pub derivation_path: String,
}

/// Result of `create_wallet` — caller learns the wallet_id + first address.
#[derive(Debug, Clone)]
pub struct WalletCreated {
    pub wallet_id: Uuid,
    pub name: String,
    pub network: Network,
    pub address: Address,
}

/// WalletManager errors. Task 4 widens to the 17-variant crate-wide enum;
/// these variants are the minimum WalletManager needs today.
#[derive(Debug, Error)]
pub enum WalletError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("crypto: {0}")]
    Crypto(#[from] CryptoError),
    #[error("mnemonic: {0}")]
    Mnemonic(String),
    #[error("private key: {0}")]
    PrivateKey(String),
    #[error("path: {0}")]
    Path(String),
    #[error("wallet '{name}' already exists on {network:?}")]
    AlreadyExists { name: String, network: Network },
    #[error("wallet {wallet_id} not found")]
    NotFound { wallet_id: Uuid },
    #[error("corrupt wallet file: {reason}")]
    Corrupt { reason: String },
}

pub type Result<T> = std::result::Result<T, WalletError>;

/// WalletManager — thread-safe encrypted-wallet store on local disk.
pub struct WalletManager {
    base_dir: PathBuf,
    wallets: RwLock<HashMap<Uuid, EncryptedBlob>>,
}

impl WalletManager {
    /// Open (or create) the default wallet store at
    /// `$XDG_DATA_HOME/eth-wallet-core/wallets/` (Linux) / platform-equivalent.
    pub fn open() -> Result<Self> {
        let dirs = ProjectDirs::from(PROJECT_QUALIFIER, PROJECT_ORG, PROJECT_APP)
            .ok_or_else(|| WalletError::Path("no project dir".to_string()))?;
        let base_dir = dirs.data_dir().join("wallets");
        Self::open_at(base_dir)
    }

    /// Open a wallet store at an explicit `base_dir` (used by tests).
    pub fn open_at(base_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_dir)?;
        let mut wallets: HashMap<Uuid, EncryptedBlob> = HashMap::new();
        // Scan existing on-disk wallets into the in-memory cache.
        scan_disk_into(&base_dir, &mut wallets)?;
        Ok(Self {
            base_dir,
            wallets: RwLock::new(wallets),
        })
    }

    /// Generate a fresh 12-word mnemonic, encrypt under `password`, persist
    /// to disk at `<base_dir>/<network>/<wallet_id>.enc`. Returns the
    /// wallet_id + first receive address (m/44'/60'/0'/0/0 per Q3).
    pub fn create_wallet(&self, name: &str, password: &[u8]) -> Result<WalletCreated> {
        let network = Network::default_v0_2();
        self.create_wallet_for_network(name, password, network)
    }

    /// Network-aware create_wallet (Task 10 CLI surfaces this).
    pub fn create_wallet_for_network(
        &self,
        name: &str,
        password: &[u8],
        network: Network,
    ) -> Result<WalletCreated> {
        if password.is_empty() {
            return Err(WalletError::Crypto(CryptoError::Argon2(
                "password must be non-empty".to_string(),
            )));
        }

        // 1. Generate fresh mnemonic (F47 zeroize treatment handled inside
        //    mnemonic::generate_12_word()).
        let phrase = mnemonic::generate_12_word();

        // 2. Derive first receive address for the metadata record.
        let address = self.address_of(&phrase, 0);

        // 3. Encrypt the phrase as bytes.
        let plaintext = phrase.to_string();
        let plaintext_bytes = plaintext.as_bytes();

        let salt = crypto::random_salt();
        let nonce = crypto::random_nonce();
        let key = crypto::derive_key(password, &salt)?;
        let key_arr: [u8; KEY_LEN] = key.as_slice()[..KEY_LEN]
            .try_into()
            .expect("KEY_LEN == 32 <= derived key length");

        let ciphertext = crypto::encrypt(&key_arr, &nonce, plaintext_bytes)?;
        let blob = EncryptedBlob::new(salt, nonce, ciphertext);

        // 4. Allocate UUID + persist atomically.
        let wallet_id = Uuid::new_v4();
        let network_dir = self.base_dir.join(network.as_dir_name());
        fs::create_dir_all(&network_dir)?;
        let path = wallet_path(&network_dir, wallet_id);

        write_atomic(&path, &serde_json::to_vec(&blob)?)?;
        self.wallets
            .write()
            .map_err(|_| WalletError::Path("wallet store poisoned".into()))?
            .insert(wallet_id, blob);

        Ok(WalletCreated {
            wallet_id,
            name: name.to_string(),
            network,
            address,
        })
    }

    /// Import an existing BIP-39 mnemonic (12/15/18/21/24 words).
    pub fn import_wallet(
        &self,
        name: &str,
        phrase: &str,
        password: &[u8],
    ) -> Result<WalletCreated> {
        let network = Network::default_v0_2();
        if password.is_empty() {
            return Err(WalletError::Crypto(CryptoError::Argon2(
                "password must be non-empty".to_string(),
            )));
        }
        let mnemonic_parsed = Mnemonic::parse_in(Language::English, phrase)
            .map_err(|e| WalletError::Mnemonic(format!("parse: {e}")))?;
        let address = mnemonic::derive_address(&mnemonic_parsed, 0);

        let plaintext_bytes = phrase.as_bytes();
        let salt = crypto::random_salt();
        let nonce = crypto::random_nonce();
        let key = crypto::derive_key(password, &salt)?;
        let key_arr: [u8; KEY_LEN] = key.as_slice()[..KEY_LEN].try_into().expect("KEY_LEN");
        let ciphertext = crypto::encrypt(&key_arr, &nonce, plaintext_bytes)?;
        let blob = EncryptedBlob::new(salt, nonce, ciphertext);

        let wallet_id = Uuid::new_v4();
        let network_dir = self.base_dir.join(network.as_dir_name());
        fs::create_dir_all(&network_dir)?;
        let path = wallet_path(&network_dir, wallet_id);
        write_atomic(&path, &serde_json::to_vec(&blob)?)?;
        self.wallets
            .write()
            .map_err(|_| WalletError::Path("wallet store poisoned".into()))?
            .insert(wallet_id, blob);

        Ok(WalletCreated {
            wallet_id,
            name: name.to_string(),
            network,
            address,
        })
    }

    /// Import a raw secp256k1 private key (32-byte hex). Per #297 G4 spec.
    pub fn import_private_key(
        &self,
        name: &str,
        private_key_hex: &str,
        password: &[u8],
    ) -> Result<WalletCreated> {
        let network = Network::default_v0_2();
        if password.is_empty() {
            return Err(WalletError::Crypto(CryptoError::Argon2(
                "password must be non-empty".to_string(),
            )));
        }
        let key_bytes = hex::decode(private_key_hex.trim_start_matches("0x"))
            .map_err(|e| WalletError::PrivateKey(format!("hex: {e}")))?;
        let signer = PrivateKeySigner::from_slice(&key_bytes)
            .map_err(|e| WalletError::PrivateKey(format!("from_slice: {e}")))?;

        // Store hex of private key bytes (cannot reverse to mnemonic).
        let plaintext_bytes = format!("0x{}", hex::encode(key_bytes.as_slice()));

        let salt = crypto::random_salt();
        let nonce = crypto::random_nonce();
        let key = crypto::derive_key(password, &salt)?;
        let key_arr: [u8; KEY_LEN] = key.as_slice()[..KEY_LEN].try_into().expect("KEY_LEN");
        let ciphertext = crypto::encrypt(&key_arr, &nonce, plaintext_bytes.as_bytes())?;
        let blob = EncryptedBlob::new(salt, nonce, ciphertext);

        let wallet_id = Uuid::new_v4();
        let address = signer.address();
        let network_dir = self.base_dir.join(network.as_dir_name());
        fs::create_dir_all(&network_dir)?;
        let path = wallet_path(&network_dir, wallet_id);
        write_atomic(&path, &serde_json::to_vec(&blob)?)?;
        self.wallets
            .write()
            .map_err(|_| WalletError::Path("store poisoned".into()))?
            .insert(wallet_id, blob);

        drop(signer);

        Ok(WalletCreated {
            wallet_id,
            name: name.to_string(),
            network,
            address,
        })
    }

    /// List all wallets under the base_dir (does NOT decrypt).
    pub fn list_wallets(&self) -> Result<Vec<WalletInfo>> {
        let wallets = self
            .wallets
            .read()
            .map_err(|_| WalletError::Path("store poisoned".into()))?;
        let mut out: Vec<WalletInfo> = Vec::with_capacity(wallets.len());
        for (wallet_id, _blob) in wallets.iter() {
            out.push(WalletInfo {
                wallet_id: *wallet_id,
                name: format!("wallet-{}", &wallet_id.to_string()[..8]),
                network: Network::Sepolia,
                address: Address::ZERO,
                derivation_path: "m/44'/60'/0'/0/0".to_string(),
            });
        }
        Ok(out)
    }

    /// Delete a wallet by ID. Removes the on-disk file + in-memory entry.
    pub fn delete_wallet(&self, wallet_id: Uuid) -> Result<()> {
        let mut wallets = self
            .wallets
            .write()
            .map_err(|_| WalletError::Path("store poisoned".into()))?;

        let path = match (
            locate_wallet(&self.base_dir, wallet_id),
            wallets.remove(&wallet_id),
        ) {
            (Some(p), Some(_)) => p,
            (None, _) => return Err(WalletError::NotFound { wallet_id }),
            (Some(_), None) => {
                return Err(WalletError::Corrupt {
                    reason: "blob missing from cache".into(),
                });
            }
        };
        fs::remove_file(&path)?;
        Ok(())
    }

    /// Unlock the wallet's mnemonic by deriving the AES key from `password`
    /// and decrypting the persisted blob. Returns a `Zeroizing<Mnemonic>`
    /// that auto-wipes on drop (F47).
    pub fn unlock(&self, wallet_id: Uuid, password: &[u8]) -> Result<Zeroizing<Mnemonic>> {
        let wallets = self
            .wallets
            .read()
            .map_err(|_| WalletError::Path("store poisoned".into()))?;
        let blob = wallets
            .get(&wallet_id)
            .ok_or(WalletError::NotFound { wallet_id })?;

        let salt = parse_blob_salt(blob)?;
        let nonce = parse_blob_nonce(blob)?;
        let ciphertext = blob.ciphertext.as_slice();
        let key = crypto::derive_key(password, &salt)?;
        let key_arr: [u8; KEY_LEN] = key.as_slice()[..KEY_LEN].try_into().expect("KEY_LEN");
        let plaintext = crypto::decrypt(&key_arr, &nonce, ciphertext)?;

        let s = std::str::from_utf8(&plaintext).map_err(|e| WalletError::Corrupt {
            reason: format!("utf8: {e}"),
        })?;
        if s.starts_with("0x") {
            // Private-key import — surface as a Mnemonic-shaped error for
            // Task 2; Task 10 will use the signer directly via a separate
            // `unlock_signer(wallet_id, password)` path.
            Err(WalletError::Corrupt {
                reason: "private-key wallet: use unlock_signer in Task 10 to get the signer".into(),
            })
        } else {
            let parsed =
                Mnemonic::parse_in(Language::English, s).map_err(|e| WalletError::Corrupt {
                    reason: format!("mnemonic parse: {e}"),
                })?;
            Ok(Zeroizing::new(parsed))
        }
    }

    /// Derive the first-receive address for `phrase` at index 0. Helper
    /// for the create / import paths.
    fn address_of(&self, phrase: &Mnemonic, index: u32) -> Address {
        let s = phrase.to_string();
        MnemonicBuilder::english()
            .phrase(s.as_str())
            .index(index)
            .expect("valid account index")
            .build()
            .expect("mnemonic build")
            .address()
    }
}

fn wallet_path(network_dir: &Path, wallet_id: Uuid) -> PathBuf {
    network_dir.join(format!("{wallet_id}.enc"))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("enc.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

fn scan_disk_into(base_dir: &Path, wallets: &mut HashMap<Uuid, EncryptedBlob>) -> Result<()> {
    let entries = match fs::read_dir(base_dir) {
        Ok(e) => e,
        Err(_) => return Ok(()), // empty / new store
    };
    for entry in entries {
        let entry = entry?;
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let dir_name = match p.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if !matches!(dir_name, "mainnet" | "sepolia" | "anvil") {
            continue;
        }
        let wallet_files = match fs::read_dir(&p) {
            Ok(w) => w,
            Err(_) => continue,
        };
        for wallet_file in wallet_files {
            let wallet_file = wallet_file?;
            let wp = wallet_file.path();
            if wp.extension().and_then(|s| s.to_str()) != Some("enc") {
                continue;
            }
            let stem = match wp.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s,
                None => continue,
            };
            let wallet_id = match Uuid::parse_str(stem) {
                Ok(u) => u,
                Err(_) => continue,
            };
            let mut buf = Vec::new();
            fs::File::open(&wp)?.read_to_end(&mut buf)?;
            let blob: EncryptedBlob = match serde_json::from_slice(&buf) {
                Ok(b) => b,
                Err(_) => continue, // silent skip on corrupt
            };
            wallets.insert(wallet_id, blob);
        }
    }
    Ok(())
}

fn locate_wallet(base_dir: &Path, wallet_id: Uuid) -> Option<PathBuf> {
    let name = format!("{wallet_id}.enc");
    for network_dir in ["mainnet", "sepolia", "anvil"] {
        let cand = base_dir.join(network_dir).join(&name);
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

fn parse_blob_salt(blob: &EncryptedBlob) -> Result<[u8; SALT_LEN]> {
    let bytes = hex::decode(&blob.salt_hex).map_err(|e| WalletError::Corrupt {
        reason: format!("salt hex: {e}"),
    })?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| WalletError::Corrupt {
            reason: format!("salt length: expected {SALT_LEN}, got {}", bytes.len()),
        })
}

fn parse_blob_nonce(blob: &EncryptedBlob) -> Result<[u8; NONCE_LEN]> {
    let bytes = hex::decode(&blob.nonce_hex).map_err(|e| WalletError::Corrupt {
        reason: format!("nonce hex: {e}"),
    })?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| WalletError::Corrupt {
            reason: format!("nonce length: expected {NONCE_LEN}, got {}", bytes.len()),
        })
}

/// Best-effort timestamp helper for `created_at_secs` field.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
