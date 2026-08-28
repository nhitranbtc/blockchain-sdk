//! WalletManager — encrypted-mnemonic wallet store for the v0.2 `eth` CLI.
//!
//! Per Issue #301 Task 2 acceptance:
//!   * Each wallet = encrypted mnemonic (Argon2id + AES-256-GCM, F5/F6
//!     mirror) + UUID `wallet_id` + user-facing `--name`. Per #297 B1
//!     (named wallets only, UUID is internal) and B2 (Argon2id from day 1).
//!   * Persistence at `<base_dir>/wallets/<network>/<wallet_id>.enc`
//!     (JSON blob) PLUS `<wallet_id>.meta.json` (WalletMeta, plaintext
//!     JSON for fast list/show without decrypting the blob).
//!   * Per-call unlock: `unlock(wallet_id, password) -> Zeroizing<Mnemonic>`
//!     re-derives the in-memory key from the encrypted blob.
//!   * Private-key blobs: `unlock_signer(wallet_id, password)` returns a
//!     `PrivateKeySigner` (handles both mnemonic-derived and pk-imported
//!     wallets). `unlock()` returns `WalletError::Corrupt` for pk blobs
//!     per the existing test contract (wallet_manager.rs:113-119).

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
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
use crate::network::{EthereumChain, Network};

/// Default XDG layout (operator-side). Overridable via `open_at` (tests).
const PROJECT_QUALIFIER: &str = "btc";
const PROJECT_ORG: &str = "nhitran";
const PROJECT_APP: &str = "eth-wallet-core";

/// Companion file extension for plaintext `WalletMeta` next to each `.enc`.
const META_EXT: &str = "meta.json";

// NOTE: `Network` (the two-level chain-family enum) lives in `crate::network`
// — moved there in Phase 0 of the polygon-wallet-core plan
// (`docs/superpowers/plans/2026-08-27-polygon-wallet-core.md`). The previous
// ETH-only flat enum (Mainnet/Sepolia/Anvil) is now
// `Network::Ethereum(EthereumChain::Mainnet)` etc. — see wallet.rs test
// assertions for the migration. All wallet.rs struct fields still type as
// `Network` because the family-level type composes via inner enum.

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

/// Plaintext metadata persisted alongside the encrypted blob. Used by
/// `list_wallets` and `show` so the CLI can render wallet identity without
/// unlocking. Safe to be plaintext — contains wallet_id (UUID), user-chosen
/// name, network, first-receive address, derivation path, creation time.
/// Never contains mnemonic, private key, or password material.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletMeta {
    pub wallet_id: Uuid,
    pub name: String,
    pub network: Network,
    pub address: Address,
    pub derivation_path: String,
    pub created_at_secs: u64,
}

/// Summary returned by `list_wallets`. Mirrors `WalletMeta` so the CLI
/// doesn't need to deserialize `WalletMeta` directly.
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
    #[error("wallet '{name}' not found on {network:?}")]
    NotFoundByName { name: String, network: Network },
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

    /// Open a wallet store at an explicit `base_dir` (used by tests + CLI
    /// when `ETH_DATA_DIR` is set).
    pub fn open_at(base_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_dir)?;
        // Tighten the directory to mode 0o700 so other local users cannot
        // enumerate wallet UUIDs + names. Defense-in-depth per #337
        // security-audit H-1. Skipped on non-unix targets (Windows will
        // need ACL handling when the platform surface grows).
        #[cfg(unix)]
        {
            let perms = std::fs::Permissions::from_mode(0o700);
            std::fs::set_permissions(&base_dir, perms)?;
        }
        let mut wallets: HashMap<Uuid, EncryptedBlob> = HashMap::new();
        // Scan existing on-disk wallets into the in-memory cache.
        scan_disk_into(&base_dir, &mut wallets)?;
        Ok(Self {
            base_dir,
            wallets: RwLock::new(wallets),
        })
    }

    /// Returns true if a wallet named `name` already exists on `network`.
    /// Used by `create_wallet_for_network` / `import_wallet_for_network` /
    /// `import_private_key` to enforce the (name, network) uniqueness
    /// invariant documented on `WalletError::AlreadyExists`. Without
    /// this check, two wallets with the same name could coexist and the
    /// second one becomes dead state (lookup_by_name resolves to the
    /// first only). Added in #337 type-design CRITICAL fix.
    pub fn name_exists_on_network(&self, name: &str, network: Network) -> bool {
        self.lookup_by_name(name, network).is_ok()
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
        if self.name_exists_on_network(name, network) {
            return Err(WalletError::AlreadyExists {
                name: name.to_string(),
                network,
            });
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
        let enc_path = wallet_path(&network_dir, wallet_id);
        let meta_path = meta_path(&network_dir, wallet_id);

        write_atomic(&enc_path, &serde_json::to_vec(&blob)?)?;
        let meta = WalletMeta {
            wallet_id,
            name: name.to_string(),
            network,
            address,
            derivation_path: "m/44'/60'/0'/0/0".to_string(),
            created_at_secs: now_secs(),
        };
        write_atomic(&meta_path, &serde_json::to_vec(&meta)?)?;

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

    /// Import an existing BIP-39 mnemonic (12/15/18/21/24 words). Uses
    /// the default network (Sepolia). Prefer `import_wallet_for_network`
    /// from CLI code so the `--network` flag is honored.
    pub fn import_wallet(
        &self,
        name: &str,
        phrase: &str,
        password: &[u8],
    ) -> Result<WalletCreated> {
        self.import_wallet_for_network(name, phrase, password, Network::default_v0_2())
    }

    /// Network-aware import (CLI uses this; the bare `import_wallet` is the
    /// legacy alias preserved for back-compat with existing tests).
    pub fn import_wallet_for_network(
        &self,
        name: &str,
        phrase: &str,
        password: &[u8],
        network: Network,
    ) -> Result<WalletCreated> {
        if password.is_empty() {
            return Err(WalletError::Crypto(CryptoError::Argon2(
                "password must be non-empty".to_string(),
            )));
        }
        if self.name_exists_on_network(name, network) {
            return Err(WalletError::AlreadyExists {
                name: name.to_string(),
                network,
            });
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
        let enc_path = wallet_path(&network_dir, wallet_id);
        let meta_path = meta_path(&network_dir, wallet_id);
        write_atomic(&enc_path, &serde_json::to_vec(&blob)?)?;
        let meta = WalletMeta {
            wallet_id,
            name: name.to_string(),
            network,
            address,
            derivation_path: "m/44'/60'/0'/0/0".to_string(),
            created_at_secs: now_secs(),
        };
        write_atomic(&meta_path, &serde_json::to_vec(&meta)?)?;

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
        if self.name_exists_on_network(name, network) {
            return Err(WalletError::AlreadyExists {
                name: name.to_string(),
                network,
            });
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
        let enc_path = wallet_path(&network_dir, wallet_id);
        let meta_path = meta_path(&network_dir, wallet_id);
        write_atomic(&enc_path, &serde_json::to_vec(&blob)?)?;
        let meta = WalletMeta {
            wallet_id,
            name: name.to_string(),
            network,
            address,
            derivation_path: "m/44'/60'/0'/0/0".to_string(),
            created_at_secs: now_secs(),
        };
        write_atomic(&meta_path, &serde_json::to_vec(&meta)?)?;

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

    /// List all wallets under the base_dir. Reads `<wallet_id>.meta.json`
    /// files for plaintext metadata so the CLI can render identity without
    /// unlocking. Falls back to placeholder metadata for legacy wallets
    /// created before meta.json persistence shipped.
    pub fn list_wallets(&self) -> Result<Vec<WalletInfo>> {
        let mut out: Vec<WalletInfo> = Vec::new();
        for network in [
            Network::Ethereum(EthereumChain::Mainnet),
            Network::Ethereum(EthereumChain::Sepolia),
            Network::Ethereum(EthereumChain::Anvil),
        ] {
            let network_dir = self.base_dir.join(network.as_dir_name());
            let entries = match fs::read_dir(&network_dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let p = entry.path();
                // Path::extension returns only the last component — for
                // `xxx.meta.json` it returns "json", not "meta.json". Match
                // the full filename suffix instead.
                let is_meta = p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|name| name.ends_with(&format!(".{META_EXT}")));
                if !is_meta {
                    continue;
                }
                let meta_bytes = match fs::read(&p) {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                let meta: WalletMeta = match serde_json::from_slice(&meta_bytes) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                out.push(WalletInfo {
                    wallet_id: meta.wallet_id,
                    name: meta.name,
                    network: meta.network,
                    address: meta.address,
                    derivation_path: meta.derivation_path,
                });
            }
        }
        Ok(out)
    }

    /// Resolve a wallet_id by `name` + `network`. Returns `NotFoundByName`
    /// if no matching wallet exists.
    pub fn lookup_by_name(&self, name: &str, network: Network) -> Result<Uuid> {
        let network_dir = self.base_dir.join(network.as_dir_name());
        let entries = match fs::read_dir(&network_dir) {
            Ok(e) => e,
            Err(_) => {
                return Err(WalletError::NotFoundByName {
                    name: name.to_string(),
                    network,
                });
            }
        };
        for entry in entries.flatten() {
            let p = entry.path();
            let is_meta = p
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|name| name.ends_with(&format!(".{META_EXT}")));
            if !is_meta {
                continue;
            }
            let meta_bytes = match fs::read(&p) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let meta: WalletMeta = match serde_json::from_slice(&meta_bytes) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.name == name {
                return Ok(meta.wallet_id);
            }
        }
        Err(WalletError::NotFoundByName {
            name: name.to_string(),
            network,
        })
    }

    /// Delete a wallet by ID. Removes the on-disk .enc + .meta.json files
    /// + in-memory cache entry. If the wallet was created before
    ///   meta.json persistence shipped, only .enc is removed (no
    ///   .meta.json to find).
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

        // Also remove the companion meta.json if present (best-effort —
        // legacy wallets pre-meta.json may not have one).
        let meta = path.with_extension(META_EXT);
        if meta.exists() {
            let _ = fs::remove_file(&meta);
        }
        Ok(())
    }

    /// Unlock the wallet's mnemonic by deriving the AES key from `password`
    /// and decrypting the persisted blob. Returns a `Zeroizing<Mnemonic>`
    /// that auto-wipes on drop (F47).
    ///
    /// **Returns `WalletError::Corrupt` for private-key wallets.** CLI code
    /// that needs to sign transactions for pk-imported wallets should call
    /// `unlock_signer(wallet_id, password)` instead.
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
        // Wrap the AES-GCM key in Zeroizing so the stack-resident copy
        // is overwritten on drop — defense-in-depth for the decryption
        // key itself (mirrors `unlock_signer`).
        let mut key_arr = Zeroizing::new([0u8; KEY_LEN]);
        key_arr.copy_from_slice(&key.as_slice()[..KEY_LEN]);
        let plaintext = crypto::decrypt(&key_arr, &nonce, ciphertext)?;

        let s = std::str::from_utf8(&plaintext).map_err(|e| WalletError::Corrupt {
            reason: format!("utf8: {e}"),
        })?;
        if s.starts_with("0x") {
            // Private-key import — surface as a Mnemonic-shaped error per
            // the wallet_manager.rs:113-119 contract. Callers needing the
            // signer should call `unlock_signer` instead.
            Err(WalletError::Corrupt {
                reason: "private-key wallet: use unlock_signer to get the signer".into(),
            })
        } else {
            let parsed =
                Mnemonic::parse_in(Language::English, s).map_err(|e| WalletError::Corrupt {
                    reason: format!("mnemonic parse: {e}"),
                })?;
            Ok(Zeroizing::new(parsed))
        }
    }

    /// Unlock a wallet and return a `PrivateKeySigner` ready for
    /// `sign_native_eth_tx` / `sign_erc20_tx_bytes` (Task 3). Handles
    /// both mnemonic-derived and private-key-imported wallets:
    ///
    /// - Mnemonic blob → re-derive signer at m/44'/60'/0'/0/0, extract
    ///   the 32-byte secret scalar.
    /// - Private-key blob → parse hex bytes into a 32-byte secret scalar.
    /// - Wrong password → `WalletError::Crypto` (AES-GCM auth tag).
    ///
    /// Returns `Zeroizing<[u8; 32]>` so the unlocked secret scalar is
    /// overwritten on drop. alloy's `PrivateKeySigner` (= `LocalSigner`)
    /// does not implement `Zeroize` (the bound `Zeroizing<T>` requires for
    /// its `Drop` impl), and the alloy newtype doesn't satisfy
    /// `DefaultIsZeroes` (the marker that auto-impls `Zeroize` for
    /// all-zero `Default` types), so we wrap the raw 32-byte secret
    /// scalar instead — mirrors the Bitcoin sibling pattern at
    /// `bitcoin-wallet-core/src/keys/signer.rs:46` (`Secret<Vec<u8>>`).
    /// Callers construct a `PrivateKeySigner::from_slice(&bytes)` at use
    /// site, scoping the signer's lifetime to the smallest possible block.
    ///
    /// **Known limitation (acknowledged, follow-up):** alloy's `LocalSigner`
    /// and k256's `SigningKey` may hold ephemeral non-zeroized copies of
    /// the secret scalar for the lifetime of the signing call (k256's
    /// `SigningKey` derives `ZeroizeOnDrop` per Cargo.lock pin, but its
    /// internal `FieldElement` drop behavior is implementation-defined).
    /// Defense-in-depth only — primary defense is shorter signer scope +
    /// future OS-keyring integration.
    pub fn unlock_signer(&self, wallet_id: Uuid, password: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
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
        // Wrap the AES-GCM key in Zeroizing so the stack-resident copy
        // (and the heap copy after Drop) is overwritten — defense-in-depth
        // for the decryption key itself.
        let mut key_arr = Zeroizing::new([0u8; KEY_LEN]);
        key_arr.copy_from_slice(&key.as_slice()[..KEY_LEN]);
        let plaintext = crypto::decrypt(&key_arr, &nonce, ciphertext)?;

        Self::decode_signer_bytes(&plaintext)
    }

    /// Decode an unlocked plaintext blob into a 32-byte secret scalar,
    /// wrapped in `Zeroizing`. Pure helper (no filesystem or manager
    /// state) so tests can exercise the length-check + UTF-8 branches
    /// directly without encryption fixtures.
    fn decode_signer_bytes(plaintext: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
        let s = std::str::from_utf8(plaintext).map_err(|e| WalletError::Corrupt {
            reason: format!("utf8: {e}"),
        })?;
        if let Some(hex_str) = s.strip_prefix("0x") {
            // Private-key blob — parse hex bytes into a 32-byte secret.
            // Wrap the decoded vector in Zeroizing so the intermediate
            // heap allocation gets overwritten on drop (before the
            // Vec<u8> allocator reuse window).
            let key_bytes: Zeroizing<Vec<u8>> = Zeroizing::new(
                hex::decode(hex_str).map_err(|e| WalletError::PrivateKey(format!("hex: {e}")))?,
            );
            if key_bytes.len() != 32 {
                return Err(WalletError::PrivateKey(format!(
                    "expected 32 bytes, got {}",
                    key_bytes.len()
                )));
            }
            let mut secret = [0u8; 32];
            secret.copy_from_slice(&key_bytes);
            Ok(Zeroizing::new(secret))
        } else {
            // Mnemonic blob — derive signer at m/44'/60'/0'/0/0, then
            // extract the 32-byte secret scalar. Wrap `parsed` (heap
            // entropy) in Zeroizing so it zeroes on drop — defense-in-
            // depth parity with `unlock()`. bip39 2.2 enables the
            // `zeroize` feature workspace-wide (`Cargo.toml:44`), so
            // `Mnemonic` impls `Zeroize`+`ZeroizeOnDrop`. The
            // `phrase: String` intermediate (bip39 has no `&str` accessor
            // without `to_string()`) is sub-millisecond and explicitly
            // documented as out-of-scope per PR body.
            let parsed =
                Mnemonic::parse_in(Language::English, s).map_err(|e| WalletError::Corrupt {
                    reason: format!("mnemonic parse: {e}"),
                })?;
            let parsed = Zeroizing::new(parsed);
            let phrase = parsed.to_string();
            let signer = MnemonicBuilder::english()
                .phrase(phrase.as_str())
                .index(0)
                .expect("valid index")
                .build()
                .expect("mnemonic build");
            // `to_bytes()` returns B256; `.0` is the inner FixedBytes<32>,
            // which coerces to [u8; 32] via the `From` impl.
            let secret: [u8; 32] = signer.to_bytes().0;
            Ok(Zeroizing::new(secret))
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

fn meta_path(network_dir: &Path, wallet_id: Uuid) -> PathBuf {
    network_dir.join(format!("{wallet_id}.{META_EXT}"))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("enc.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        // mode 0o600 — owner read/write only. Per #337 security-audit H-1
        // the default umask would leave encrypted blobs world-readable.
        #[cfg(unix)]
        {
            let perms = std::fs::Permissions::from_mode(0o600);
            f.set_permissions(perms)?;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::PolygonChain;
    use tempfile::tempdir;

    fn password() -> Vec<u8> {
        b"correct horse battery staple".to_vec()
    }

    #[test]
    fn network_parse_accepts_known_aliases() {
        assert_eq!(
            Network::parse_cli("mainnet").unwrap(),
            Network::Ethereum(EthereumChain::Mainnet)
        );
        assert_eq!(
            Network::parse_cli("Sepolia").unwrap(),
            Network::Ethereum(EthereumChain::Sepolia)
        );
        assert_eq!(
            Network::parse_cli("anvil").unwrap(),
            Network::Ethereum(EthereumChain::Anvil)
        );
        assert_eq!(
            Network::parse_cli("dev").unwrap(),
            Network::Ethereum(EthereumChain::Anvil)
        );
        assert_eq!(
            Network::parse_cli("31337").unwrap(),
            Network::Ethereum(EthereumChain::Anvil)
        );
        // Phase 0 family-level: polygon now parses to Polygon::Mainnet.
        assert_eq!(
            Network::parse_cli("polygon").unwrap(),
            Network::Polygon(PolygonChain::Mainnet)
        );
        // ETH-only parser (used by `eth` CLI) still rejects polygon.
        assert!(EthereumChain::parse_cli("polygon").is_err());
    }

    #[test]
    fn list_wallets_returns_real_name_after_meta_write() {
        let tmp = tempdir().unwrap();
        let mgr = WalletManager::open_at(tmp.path().to_path_buf()).unwrap();
        mgr.create_wallet("alpha", &password()).unwrap();
        mgr.import_wallet(
            "beta-import",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            &password(),
        )
        .unwrap();

        let listed = mgr.list_wallets().unwrap();
        assert_eq!(listed.len(), 2);
        let names: Vec<&str> = listed.iter().map(|w| w.name.as_str()).collect();
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta-import"));
        // No more placeholder `wallet-<uuid8>`.
        for w in &listed {
            assert!(
                !w.name.starts_with("wallet-"),
                "name leaked placeholder: {}",
                w.name
            );
            assert_ne!(w.address, Address::ZERO, "address leaked ZERO");
            assert_eq!(w.network, Network::Ethereum(EthereumChain::Sepolia));
            assert_eq!(w.derivation_path, "m/44'/60'/0'/0/0");
        }
    }

    #[test]
    fn lookup_by_name_resolves_wallet_id() {
        let tmp = tempdir().unwrap();
        let mgr = WalletManager::open_at(tmp.path().to_path_buf()).unwrap();
        let created = mgr.create_wallet("findme", &password()).unwrap();

        let resolved = mgr
            .lookup_by_name("findme", Network::Ethereum(EthereumChain::Sepolia))
            .unwrap();
        assert_eq!(resolved, created.wallet_id);
        assert!(mgr
            .lookup_by_name("nope", Network::Ethereum(EthereumChain::Sepolia))
            .is_err());
    }

    #[test]
    fn unlock_signer_works_for_mnemonic_wallet() {
        let tmp = tempdir().unwrap();
        let mgr = WalletManager::open_at(tmp.path().to_path_buf()).unwrap();
        let w = mgr.create_wallet("signer-test", &password()).unwrap();

        let secret = mgr.unlock_signer(w.wallet_id, &password()).unwrap();
        let signer =
            PrivateKeySigner::from_slice(secret.as_ref()).expect("unlock_signer returns 32 bytes");
        assert_eq!(signer.address(), w.address);
    }

    #[test]
    fn unlock_signer_works_for_private_key_wallet() {
        let tmp = tempdir().unwrap();
        let mgr = WalletManager::open_at(tmp.path().to_path_buf()).unwrap();
        let pk = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let w = mgr
            .import_private_key("pk-signer-test", pk, &password())
            .unwrap();

        let secret = mgr.unlock_signer(w.wallet_id, &password()).unwrap();
        let expected: Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
            .parse()
            .unwrap();
        let signer =
            PrivateKeySigner::from_slice(secret.as_ref()).expect("unlock_signer returns 32 bytes");
        assert_eq!(signer.address(), expected);
    }

    #[test]
    fn unlock_signer_returns_zeroizing_wrapper() {
        // RED for H-2 (Issue #350): assert `unlock_signer()` returns
        // `Zeroizing<[u8; 32]>` raw secret bytes so the unlocked key
        // material is overwritten on drop. alloy's `PrivateKeySigner` (=
        // `LocalSigner`) does NOT satisfy `DefaultIsZeroes` (required by
        // `Zeroizing<T>`), so we wrap the raw 32-byte secret scalar
        // instead — mirrors Bitcoin sibling pattern at
        // `bitcoin-wallet-core/src/keys/signer.rs:46` (`Secret<Vec<u8>>`).
        // Callers construct `PrivateKeySigner::from_slice(&bytes)` at use
        // site, scoping signer lifetime to the smallest possible block.
        let tmp = tempdir().unwrap();
        let mgr = WalletManager::open_at(tmp.path().to_path_buf()).unwrap();
        let w = mgr
            .create_wallet("zeroizing-wrapper-test", &password())
            .unwrap();

        // Type assertion: must bind to `Zeroizing<[u8; 32]>`.
        let secret: Zeroizing<[u8; 32]> = mgr.unlock_signer(w.wallet_id, &password()).unwrap();
        // Round-trip: construct PrivateKeySigner at use site; assert the
        // address matches the wallet's stored address (proves bytes are
        // the real secret key, not garbage).
        let signer = PrivateKeySigner::from_slice(secret.as_ref())
            .expect("unlock_signer must yield a valid 32-byte secret");
        assert_eq!(signer.address(), w.address);
    }

    #[test]
    fn delete_wallet_removes_both_enc_and_meta() {
        let tmp = tempdir().unwrap();
        let mgr = WalletManager::open_at(tmp.path().to_path_buf()).unwrap();
        let w = mgr.create_wallet("del-meta-test", &password()).unwrap();

        let enc = tmp
            .path()
            .join("sepolia")
            .join(format!("{}.enc", w.wallet_id));
        let meta = tmp
            .path()
            .join("sepolia")
            .join(format!("{}.meta.json", w.wallet_id));
        assert!(enc.exists());
        assert!(meta.exists(), "meta.json must exist alongside .enc");

        mgr.delete_wallet(w.wallet_id).unwrap();
        assert!(!enc.exists(), ".enc must be removed");
        assert!(!meta.exists(), ".meta.json must be removed");
    }

    #[test]
    fn import_wallet_for_network_persists_under_correct_dir() {
        let tmp = tempdir().unwrap();
        let mgr = WalletManager::open_at(tmp.path().to_path_buf()).unwrap();
        let created = mgr
            .import_wallet_for_network(
                "anvil-test",
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
                &password(),
                Network::Ethereum(EthereumChain::Anvil),
            )
            .unwrap();
        assert_eq!(created.network, Network::Ethereum(EthereumChain::Anvil));
        assert!(tmp
            .path()
            .join("anvil")
            .join(format!("{}.enc", created.wallet_id))
            .exists());
        assert!(tmp
            .path()
            .join("anvil")
            .join(format!("{}.meta.json", created.wallet_id))
            .exists());
    }

    #[test]
    fn decode_signer_bytes_rejects_wrong_length_hex() {
        // CRITICAL coverage gap closed (pr-test-analyzer #1): the explicit
        // length-check on the private-key path was the only thing
        // standing between a malformed/legacy `.enc` blob and
        // `copy_from_slice` panicking.
        // Odd-length hex (31 bytes) — must hit the length-check branch.
        let err = WalletManager::decode_signer_bytes(
            b"0xabababababababababababababababababababababababababababababab",
        )
        .expect_err("odd-length hex payload must fail length check");
        assert!(matches!(err, WalletError::PrivateKey(_)), "got: {err:?}");
        // Don't pin the byte count — assert the error shape instead.
        let msg = err.to_string();
        assert!(
            msg.starts_with("private key: expected 32 bytes, got "),
            "error msg should mention expected + actual length: {msg}"
        );
        assert!(
            !msg.ends_with("got 32"),
            "wrong-length payload must not claim 32 bytes: {msg}"
        );

        // Even-length-but-wrong hex (33 bytes) — must also fail.
        let err = WalletManager::decode_signer_bytes(
            b"0xababababababababababababababababababababababababababababababababab",
        )
        .expect_err("33-byte payload must fail length check");
        assert!(matches!(err, WalletError::PrivateKey(_)), "got: {err:?}");
    }

    #[test]
    fn decode_signer_bytes_accepts_valid_64char_hex() {
        // Companion to the wrong-length test: a well-formed 64-char hex
        // payload (32 bytes after decode) round-trips through to the
        // private-key Anvil account #0.
        let secret = WalletManager::decode_signer_bytes(
            b"0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        )
        .expect("valid 32-byte secret");
        let signer =
            PrivateKeySigner::from_slice(secret.as_ref()).expect("unlock_signer returns 32 bytes");
        let expected: Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
            .parse()
            .unwrap();
        assert_eq!(signer.address(), expected);
    }
}
