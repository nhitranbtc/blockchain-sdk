//! `sol-wallet-core` — `WalletManager` CRUD over encrypted blobs.
//!
//! Phase 6.1 Task 6.1 Step 4 + L13 step 10 review fixes (Sept 11):
//!
//! - **P6-3** — `OwnedLock` wraps `crate::wallet::Wallet` (a newtype
//!   over `solana_sdk::Keypair`). Signing methods on `OwnedLock`
//!   delegate to `Wallet::sign_message` / `sign_transaction` so the
//!   inner Keypair never escapes. The Anza-Zeroize gap is documented
//!   on `OwnedLock::Drop`.
//! - **P6-6** — `import_from_pk_file` refuses Unix source file with
//!   `mode & 0o077 != 0` OR that resolve to symlinks (`symlink_metadata`).
//! - **P6-14** — in-memory map holds encrypted blobs, NOT plaintext keys.
//! - **Phantom-equivalence fix (L13 review Sept 11)** —
//!   `create_with_mnemonic` / `import_from_phrase` route through
//!   `Wallet::from_mnemonic_at` so the SLIP-0010 derivation chain
//!   matches Phantom.

use crate::crypto::{self, EncryptedBlob};
use crate::platform::WalletStorage;
use crate::wallet::Wallet;
use crate::{Error, Result, WalletId};
use solana_sdk::signature::Signature;
use solana_sdk::transaction::VersionedTransaction;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr as _;
use std::sync::{Arc, RwLock};
use zeroize::{Zeroize, Zeroizing};

/// Public summary of a wallet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletSummary {
    /// Stable wallet identifier.
    pub id: WalletId,
    /// User-chosen name.
    pub name: String,
    /// On-chain public key (safe to expose).
    pub pubkey: solana_sdk::pubkey::Pubkey,
    /// Creation timestamp (Unix seconds).
    pub created_at_unix: u64,
}

/// Per-wallet record stored in `WalletStorage` and the in-memory map.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WalletRecord {
    id: WalletId,
    name: String,
    pubkey_bs58: String,
    created_at_unix: u64,
    blob: EncryptedBlob,
}

/// RAII guard holding an unlocked `Wallet`.
///
/// # No `&Keypair` accessor (L13 step 10 review Sept 11)
///
/// `Wallet` is a newtype over `solana_sdk::Keypair`, which provides a
/// public `insecure_clone()` method. Exposing `&Keypair` (via
/// `&Wallet` → `&Keypair`) would let callers clone the inner keypair
/// and bypass RAII. Instead, this guard exposes only `sign_message`
/// and `sign_transaction_message`, never the inner keypair.
///
/// # Anza-Zeroize gap (L13 review Sept 11)
///
/// `solana_sdk::Keypair` does NOT implement `Zeroize`. `Drop` below
/// performs a `to_bytes()` round-trip + `Zeroizing<[u8; 64]>` zeroize
/// on a *copy*. The `Box<Keypair>`'s heap allocation returns to the
/// allocator with the original secret bytes intact — documented
/// residual limitation requiring an upstream `Zeroize` impl on
/// `solana_keypair::Keypair` for a hard guarantee.
pub struct OwnedLock {
    inner: Box<Wallet>,
}

impl OwnedLock {
    /// Sign arbitrary bytes via `Signer::sign_message`.
    pub fn sign_message(&self, msg: &[u8]) -> Signature {
        self.inner.sign_message(msg)
    }

    /// Sign a `VersionedTransaction`. Inserts the Ed25519 signature
    /// at the wallet's pubkey position in the message's static
    /// account keys.
    pub fn sign_transaction(&self, tx: VersionedTransaction) -> Result<VersionedTransaction> {
        self.inner.sign_transaction(tx)
    }

    /// Borrow the inner Wallet. **CALLER MUST NOT clone the
    /// underlying Keypair** via `wallet.into_inner()` (not even
    /// `pub(crate)` exposure — kept private). The only legitimate
    /// use of `&self` is to read the pubkey or pass to a function
    /// that takes `&dyn Signer` in the same scope.
    pub fn wallet(&self) -> &Wallet {
        &self.inner
    }
}

impl Drop for OwnedLock {
    fn drop(&mut self) {
        // Best-effort round-trip zeroize. `Wallet::inner_bytes`
        // returns `Zeroizing<[u8; 64]>` which triggers Drop on its
        // own scope exit — the stack/heap copy is zeroed. The
        // `Box<Keypair>`'s heap allocation is opaque to us and
        // returns to the allocator with the original bytes intact
        // (Anza gap, documented on struct).
        let mut bytes = self.inner.inner_bytes();
        bytes.zeroize();
    }
}

impl std::fmt::Debug for OwnedLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedLock")
            .field("pubkey", &self.inner.public_key())
            .finish()
    }
}

/// CRUD over encrypted blobs.
pub struct WalletManager<S: WalletStorage> {
    storage: Arc<S>,
    inner: RwLock<HashMap<WalletId, WalletRecord>>,
}

impl<S: WalletStorage + 'static> WalletManager<S> {
    /// Construct a manager. Loads all persisted records on startup.
    pub fn new(storage: S) -> Result<Self> {
        let storage = Arc::new(storage);
        let mut map = HashMap::new();
        for id_str in storage.list_ids()? {
            let bytes = storage.get(&id_str)?;
            let rec: WalletRecord =
                serde_json::from_slice(&bytes).map_err(|source| Error::RecordCorrupt {
                    context: "load wallet record",
                    message: source.to_string(),
                })?;
            map.insert(rec.id, rec);
        }
        Ok(Self {
            storage,
            inner: RwLock::new(map),
        })
    }

    /// Generate a fresh keypair from a BIP-39 phrase using the
    /// Phantom SLIP-0010 path (`m/44'/501'/{account}'/0'/0'`).
    pub fn create_with_mnemonic(
        &self,
        mnemonic_words: &str,
        password: &str,
        name: &str,
        account: u32,
        address_index: u32,
        now_unix: u64,
    ) -> Result<WalletId> {
        let wallet = Wallet::from_mnemonic_at(mnemonic_words, account, address_index)?;
        self.store_wallet(&wallet, password, name, now_unix)
    }

    /// Alias for `create_with_mnemonic`.
    pub fn import_from_phrase(
        &self,
        phrase: &str,
        password: &str,
        name: &str,
        account: u32,
        address_index: u32,
        now_unix: u64,
    ) -> Result<WalletId> {
        self.create_with_mnemonic(phrase, password, name, account, address_index, now_unix)
    }

    /// Import a base58 64-byte secret. Refuses Unix source file with
    /// `mode & 0o077 != 0` OR that resolves to a symlink (L13 review
    /// Sept 11 — `metadata` follows symlinks, defeating mode check).
    pub fn import_from_pk_file(
        &self,
        path: &Path,
        password: &str,
        name: &str,
        now_unix: u64,
    ) -> Result<WalletId> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let meta = std::fs::symlink_metadata(path).map_err(|source| Error::FileIo {
                path: path.to_path_buf(),
                source,
            })?;
            if meta.file_type().is_symlink() {
                return Err(Error::InsecureSourceFile {
                    path: path.to_path_buf(),
                    mode: MetadataExt::mode(&meta),
                });
            }
            let mode = MetadataExt::mode(&meta);
            if mode & 0o077 != 0 {
                return Err(Error::InsecureSourceFile {
                    path: path.to_path_buf(),
                    mode,
                });
            }
        }
        let secret_b58 = std::fs::read_to_string(path).map_err(|source| Error::FileIo {
            path: path.to_path_buf(),
            source,
        })?;
        let secret_b58 = secret_b58.trim();
        let decoded =
            bs58::decode(secret_b58)
                .into_vec()
                .map_err(|_| Error::InvalidBase58Secret {
                    got: secret_b58.len(),
                    expected: 64,
                })?;
        if decoded.len() != 64 {
            return Err(Error::InvalidBase58Secret {
                got: decoded.len(),
                expected: 64,
            });
        }
        let mut arr = Zeroizing::new([0u8; 64]);
        arr.as_mut_slice().copy_from_slice(&decoded);
        let wallet = Wallet::from_bytes(&arr);
        self.store_wallet(&wallet, password, name, now_unix)
    }

    /// Decrypt the blob for `id`, return an `OwnedLock`.
    pub fn unlock(&self, id: WalletId, password: &str) -> Result<OwnedLock> {
        let rec = {
            let g = self.inner.read().expect("wallet manager lock poisoned");
            g.get(&id).cloned()
        }
        .ok_or(Error::WalletNotFound(id))?;
        let plaintext = crypto::decrypt_wallet(&rec.blob, password)?;
        let mut bytes = plaintext;
        // Reconstruct Wallet from 64-byte serialization (32 secret
        // + 32 pubkey). Ed25519 public key is deterministically
        // derived from the secret, so the inner pubkey byte slice
        // is redundant but we keep the standard 64-byte wire format.
        let arr: [u8; 64] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| Error::WalletDecryptFailed { id })?;
        bytes.zeroize();
        let wallet = Wallet::from_bytes(&arr);
        Ok(OwnedLock {
            inner: Box::new(wallet),
        })
    }

    /// Logical no-op — validates id exists. RAII fires when the
    /// `OwnedLock` from `unlock` drops.
    pub fn lock(&self, id: WalletId) -> Result<()> {
        let g = self.inner.read().expect("wallet manager lock poisoned");
        if !g.contains_key(&id) {
            return Err(Error::WalletNotFound(id));
        }
        Ok(())
    }

    /// Return the public summary for `id`.
    pub fn summary(&self, id: WalletId) -> Result<WalletSummary> {
        let rec = {
            let g = self.inner.read().expect("wallet manager lock poisoned");
            g.get(&id).cloned()
        }
        .ok_or(Error::WalletNotFound(id))?;
        let pubkey = solana_sdk::pubkey::Pubkey::from_str(&rec.pubkey_bs58).map_err(|source| {
            Error::RecordCorrupt {
                context: "parse stored pubkey_bs58",
                message: source.to_string(),
            }
        })?;
        Ok(WalletSummary {
            id: rec.id,
            name: rec.name,
            pubkey,
            created_at_unix: rec.created_at_unix,
        })
    }

    /// List all wallet summaries (sorted by id).
    pub fn list(&self) -> Result<Vec<WalletSummary>> {
        let g = self.inner.read().expect("wallet manager lock poisoned");
        let mut out: Vec<WalletSummary> = Vec::with_capacity(g.len());
        for rec in g.values() {
            let pubkey =
                solana_sdk::pubkey::Pubkey::from_str(&rec.pubkey_bs58).map_err(|source| {
                    Error::RecordCorrupt {
                        context: "parse stored pubkey_bs58 during list",
                        message: source.to_string(),
                    }
                })?;
            out.push(WalletSummary {
                id: rec.id,
                name: rec.name.clone(),
                pubkey,
                created_at_unix: rec.created_at_unix,
            });
        }
        out.sort_by_key(|w| w.id);
        Ok(out)
    }

    /// Delete the wallet record for `id`. Storage-first ordering
    /// (L13 review Sept 11).
    pub fn delete(&self, id: WalletId) -> Result<()> {
        let rec = {
            let g = self.inner.read().expect("wallet manager lock poisoned");
            g.get(&id).cloned()
        }
        .ok_or(Error::WalletNotFound(id))?;
        let id_str = record_id_str(rec.id);
        self.storage.delete(&id_str)?;
        let mut g = self.inner.write().expect("wallet manager lock poisoned");
        g.remove(&id);
        Ok(())
    }

    /// Rename the wallet for `id`. Storage-first ordering.
    pub fn rename(&self, id: WalletId, new_name: &str) -> Result<()> {
        let updated = {
            let g = self.inner.read().expect("wallet manager lock poisoned");
            let rec = g.get(&id).ok_or(Error::WalletNotFound(id))?;
            let mut updated = rec.clone();
            updated.name = new_name.to_string();
            updated
        };
        let id_str = record_id_str(updated.id);
        let bytes = serde_json::to_vec(&updated).map_err(|source| Error::RecordCorrupt {
            context: "serialize updated WalletRecord for rename",
            message: source.to_string(),
        })?;
        self.storage.put_atomic(&id_str, &bytes)?;
        let mut g = self.inner.write().expect("wallet manager lock poisoned");
        if let Some(rec) = g.get_mut(&id) {
            rec.name = new_name.to_string();
        }
        Ok(())
    }

    fn store_wallet(
        &self,
        wallet: &Wallet,
        password: &str,
        name: &str,
        now_unix: u64,
    ) -> Result<WalletId> {
        let bytes = wallet.inner_bytes();
        let plaintext = Zeroizing::new(bytes.to_vec());
        let blob = crypto::encrypt_wallet(plaintext, password)?;
        let id = WalletId::new()?;
        let rec = WalletRecord {
            id,
            name: name.to_string(),
            pubkey_bs58: wallet.public_key().to_string(),
            created_at_unix: now_unix,
            blob,
        };
        let id_str = record_id_str(id);
        let bytes = serde_json::to_vec(&rec).map_err(|source| Error::RecordCorrupt {
            context: "serialize WalletRecord for store",
            message: source.to_string(),
        })?;
        self.storage.put_atomic(&id_str, &bytes)?;
        let mut g = self.inner.write().expect("wallet manager lock poisoned");
        g.insert(id, rec);
        Ok(id)
    }
}

fn record_id_str(id: WalletId) -> String {
    serde_json::to_string(&id)
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_else(|_| id.to_string())
}
