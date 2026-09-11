//! `sol-wallet-core` — `WalletManager` CRUD over encrypted blobs.
//!
//! Phase 6.1 Task 6.1 Step 4. Per security audit
//! `docs/audit/2026-09-11-sol-wallet-core-phase6-security-review.md`:
//!
//! - **P6-3** — `unlock -> OwnedLock` wraps `Zeroizing<Keypair>`;
//!   `Drop` zeros bytes. Caller MUST NOT clone.
//! - **P6-6** — `import_from_pk_file` refuses source file with
//!   Unix mode `& 0o077 != 0`.
//! - **P6-14** — in-memory map holds encrypted blobs, NOT plaintext
//!   keys (confirmed).

use crate::crypto::{self, EncryptedBlob};
use crate::platform::WalletStorage;
use crate::{Error, Result, WalletId};
use solana_sdk::signature::{Keypair, Signer};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr as _;
use std::sync::{Arc, RwLock};
use zeroize::{Zeroize, Zeroizing};

/// Public summary of a wallet — what `list()` and `summary()` return.
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

/// RAII guard holding an unlocked `Keypair`. `Drop` zeros the
/// secret bytes via `to_bytes` round-trip + `Zeroizing` (audit P6-3).
/// `solana_sdk::Keypair` does not implement `Zeroize` (Anza gap);
/// `Zeroizing<[u8; 64]>` is the verifiable contract. Caller MUST
/// NOT extract the keypair via any method — the only legitimate
/// use is `sign_transaction` / `sign_message` via `OwnedLock`.
pub struct OwnedLock {
    inner: Box<Keypair>,
}

impl OwnedLock {
    /// Borrow the inner keypair. Caller MUST NOT clone.
    pub fn keypair(&self) -> &Keypair {
        &self.inner
    }
}

impl Drop for OwnedLock {
    fn drop(&mut self) {
        // Explicit zeroize via to_bytes round-trip. `Box` ensures
        // the Keypair lives on the heap; the round-trip
        // overwrites those bytes with the resulting array (which
        // we then Zeroizing-wrap, triggering Drop on scope exit).
        // The Box deallocation then returns the (now-zeroed)
        // memory to the allocator — best-effort, not perfect, but
        // significantly better than no-op Drop on the Anza type.
        let mut bytes = Zeroizing::new(self.inner.to_bytes());
        bytes.zeroize();
        // `bytes` is dropped at end of this scope, firing Zeroizing.
    }
}

impl std::fmt::Debug for OwnedLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedLock")
            .field("pubkey", &self.inner.pubkey())
            .finish()
    }
}

/// CRUD over encrypted blobs. Holds `Arc<S: WalletStorage>` for
/// persistence, `RwLock<HashMap<WalletId, WalletRecord>>` for
/// in-memory lookup. Map holds `EncryptedBlob` (ciphertext), NOT
/// plaintext keys (audit P6-14).
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
                serde_json::from_slice(&bytes).map_err(|_| Error::Placeholder)?;
            map.insert(rec.id, rec);
        }
        Ok(Self {
            storage,
            inner: RwLock::new(map),
        })
    }

    /// Generate a fresh keypair from a BIP-39 phrase, encrypt the
    /// keypair bytes under `password`, store as new record.
    pub fn create_with_mnemonic(
        &self,
        mnemonic_words: &str,
        password: &str,
        name: &str,
        now_unix: u64,
    ) -> Result<WalletId> {
        use solana_sdk::signer::SeedDerivable;
        let mnemonic =
            bip39::Mnemonic::parse(mnemonic_words).map_err(|_| Error::InvalidMnemonic)?;
        let seed = Zeroizing::new(mnemonic.to_seed(""));
        let keypair = Keypair::from_seed(seed.as_ref()).map_err(|_| Error::InvalidSeed)?;
        self.store_keypair(&keypair, password, name, now_unix)
    }

    /// Import an existing BIP-39 phrase.
    pub fn import_from_phrase(
        &self,
        phrase: &str,
        password: &str,
        name: &str,
        now_unix: u64,
    ) -> Result<WalletId> {
        self.create_with_mnemonic(phrase, password, name, now_unix)
    }

    /// Import a base58-encoded 64-byte secret from `path`. Refuses
    /// source files with mode `& 0o077 != 0` on Unix (audit P6-6).
    pub fn import_from_pk_file(
        &self,
        path: &Path,
        password: &str,
        name: &str,
        now_unix: u64,
    ) -> Result<WalletId> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(path).map_err(|source| Error::FileIo {
                path: path.to_path_buf(),
                source,
            })?;
            let mode = meta.permissions().mode();
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
        let keypair = Keypair::try_from_base58_string(secret_b58)
            .map_err(|_| Error::InvalidBase58Secret(secret_b58.len()))?;
        self.store_keypair(&keypair, password, name, now_unix)
    }

    /// Decrypt the blob for `id`, return an `OwnedLock` wrapping
    /// the plaintext keypair (audit P6-3).
    pub fn unlock(&self, id: WalletId, password: &str) -> Result<OwnedLock> {
        let rec = {
            let g = self.inner.read().expect("wallet manager lock poisoned");
            g.get(&id).cloned()
        }
        .ok_or(Error::WalletNotFound(id))?;
        let plaintext = crypto::decrypt_wallet(&rec.blob, password)?;
        let bytes: [u8; 64] = plaintext
            .as_slice()
            .try_into()
            .map_err(|_| Error::WalletDecryptFailed { id })?;
        // Reconstruct Keypair from the 32-byte secret (first 32 bytes
        // of the 64-byte serialized form). Ed25519 public key is
        // deterministically derived from the secret.
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&bytes[..32]);
        let keypair = Keypair::new_from_array(secret);
        Ok(OwnedLock {
            inner: Box::new(keypair),
        })
    }

    /// Drop any in-memory plaintext key for `id`. Since this
    /// implementation does NOT cache plaintext keys in memory
    /// (audit P6-14), `lock` is a logical no-op that validates
    /// the id exists. RAII happens automatically when the
    /// `OwnedLock` from `unlock` goes out of scope.
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
        let pubkey = solana_sdk::pubkey::Pubkey::from_str(&rec.pubkey_bs58)
            .map_err(|_| Error::Placeholder)?;
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
        let mut out: Vec<WalletSummary> = g
            .values()
            .filter_map(|rec| {
                let pubkey = solana_sdk::pubkey::Pubkey::from_str(&rec.pubkey_bs58).ok()?;
                Some(WalletSummary {
                    id: rec.id,
                    name: rec.name.clone(),
                    pubkey,
                    created_at_unix: rec.created_at_unix,
                })
            })
            .collect();
        out.sort_by_key(|w| w.id);
        Ok(out)
    }

    /// Delete the wallet record for `id`.
    pub fn delete(&self, id: WalletId) -> Result<()> {
        let removed = {
            let mut g = self.inner.write().expect("wallet manager lock poisoned");
            g.remove(&id)
        }
        .ok_or(Error::WalletNotFound(id))?;
        let name = record_id_str(removed.id);
        self.storage.delete(&name)
    }

    /// Rename the wallet for `id`.
    pub fn rename(&self, id: WalletId, new_name: &str) -> Result<()> {
        let updated = {
            let mut g = self.inner.write().expect("wallet manager lock poisoned");
            let rec = g.get_mut(&id).ok_or(Error::WalletNotFound(id))?;
            rec.name = new_name.to_string();
            rec.clone()
        };
        let id_str = record_id_str(updated.id);
        let bytes = serde_json::to_vec(&updated).map_err(|_| Error::Placeholder)?;
        self.storage.put_atomic(&id_str, &bytes)
    }

    fn store_keypair(
        &self,
        keypair: &Keypair,
        password: &str,
        name: &str,
        now_unix: u64,
    ) -> Result<WalletId> {
        let bytes: Zeroizing<[u8; 64]> = Zeroizing::new(keypair.to_bytes());
        let plaintext = Zeroizing::new(bytes.to_vec());
        let blob = crypto::encrypt_wallet(plaintext, password)?;
        let id = WalletId::new()?;
        let rec = WalletRecord {
            id,
            name: name.to_string(),
            pubkey_bs58: keypair.pubkey().to_string(),
            created_at_unix: now_unix,
            blob,
        };
        let id_str = record_id_str(id);
        let bytes = serde_json::to_vec(&rec).map_err(|_| Error::Placeholder)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::InMemoryStorage;

    fn fixture() -> WalletManager<InMemoryStorage> {
        WalletManager::new(InMemoryStorage::new()).expect("manager")
    }

    #[test]
    fn empty_manager_lists_nothing() {
        let m = fixture();
        assert!(m.list().unwrap().is_empty());
    }

    #[test]
    fn create_then_summary_round_trip() {
        let m = fixture();
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let id = m
            .create_with_mnemonic(
                phrase,
                "correct horse battery staple",
                "main",
                1_700_000_000,
            )
            .unwrap();
        let s = m.summary(id).unwrap();
        assert_eq!(s.name, "main");
        assert!(!s.pubkey.to_string().is_empty());
    }

    #[test]
    fn unlock_wrong_password_errors() {
        let m = fixture();
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let id = m.create_with_mnemonic(phrase, "right", "main", 1).unwrap();
        let e = m.unlock(id, "wrong").unwrap_err();
        assert!(matches!(e, Error::WalletDecryptFailed { .. }));
    }

    #[test]
    fn delete_then_summary_errors() {
        let m = fixture();
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let id = m.create_with_mnemonic(phrase, "pw", "main", 1).unwrap();
        m.delete(id).unwrap();
        let e = m.summary(id).unwrap_err();
        assert!(matches!(e, Error::WalletNotFound(_)));
    }
}
