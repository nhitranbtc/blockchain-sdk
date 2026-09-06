//! Phase 5 — `WalletManager`: encrypted wallet persistence (Task 4.7).
//!
//! Storage flow per `WalletManager::create`:
//!
//! ```text
//! Mnemonic::phrase() → bytes
//!   → crypto::encrypt(bytes, passphrase)          [Argon2id + AES-256-GCM]
//!   → WalletStorage::put_atomic(id, blob)         [PAL-backed: filesystem / Keychain / EncryptedFile]
//! ```
//!
//! `WalletManager::unlock` reverses it:
//!
//! ```text
//! WalletStorage::get(id) → blob
//!   → crypto::decrypt(blob, passphrase)           [Zeroizing<Vec<u8>> plaintext]
//!   → Mnemonic::from_phrase(&str, lang)           [validated against BIP-39 word list]
//!   → UnlockedWallet { mnemonic: Zeroizing<...> }
//! ```
//!
//! **Plaintext format inside the encrypted blob:** JSON-encoded
//! `PlaintextRecord { phrase: String }`. A v0.1 blob is exactly one
//! phrase; later versions will add a `version` + `network` +
//! `derivation_path` header. Single-phrase-per-wallet is fine because
//! the CLI enforces "one wallet per logical account".
//!
//! **Zeroizing wrap:** the `Mnemonic` returned by `unlock` is rebuilt
//! from the decrypted phrase bytes and lives inside `bip39::Mnemonic`,
//! whose own storage is `Zeroizing` per Phase 1 finding. Its `Drop`
//! zeroes those bytes.
//!
//! **Storage abstraction:** `WalletManager` owns a `&dyn WalletStorage`;
//! the PAL impl is injected by the binary that wires it up (desktop:
//! `FileWalletStorage`; mobile: `KeychainWalletStorage` /
//! `EncryptedFileWalletStorage`; tests: `InMemoryStorage`).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::crypto::{self, EncryptedWallet};
use crate::error::{Error, Result};
use crate::keys::{Language, Mnemonic};
use crate::platform::storage::WalletStorage;
use crate::wallet::id::WalletId;

/// State after a successful `unlock`. Holds the mnemonic inside
/// `bip39::Mnemonic` (zeroize-on-drop, `Debug` redacts the phrase).
///
/// Future `UnlockedWallet` may also carry the resolved derivation
/// path + xpub; for v0.1 the mnemonic is the only secret the caller
/// needs to derive everything else.
pub struct UnlockedWallet {
    id: WalletId,
    mnemonic: Mnemonic,
}

impl UnlockedWallet {
    /// Wallet id this unlocked wallet corresponds to.
    pub fn id(&self) -> WalletId {
        self.id
    }

    /// Borrow the mnemonic (zeroize-on-drop, `Debug` redacts the phrase).
    pub fn mnemonic(&self) -> &Mnemonic {
        &self.mnemonic
    }
}

impl fmt::Debug for UnlockedWallet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnlockedWallet")
            .field("id", &self.id)
            .field("mnemonic", &"<redacted>")
            .finish()
    }
}

/// Plaintext record encrypted at rest. v0.1 = just the phrase.
///
/// Future versions may add `version: u8` + `network` + `derivation_path`
/// fields. Adding them requires an on-disk version byte; v0.1 ships
/// phrase-only and reserves the right to migrate (per design note in
/// plan §Phase 5 Task 4.7).
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlaintextRecord {
    phrase: String,
}

impl PlaintextRecord {
    fn from_mnemonic(m: &Mnemonic) -> Self {
        Self {
            phrase: m.phrase().to_owned(),
        }
    }

    fn to_mnemonic(&self, language: Language) -> Result<Mnemonic> {
        Mnemonic::from_phrase(&self.phrase, language)
    }
}

/// Encrypted-wallet persistence layer. Takes any `WalletStorage` impl
/// at construction; the same instance is usable across many wallet
/// create/unlock cycles.
pub struct WalletManager<'a> {
    storage: &'a dyn WalletStorage,
}

impl<'a> WalletManager<'a> {
    /// Construct a manager that persists through `storage`. The
    /// manager does not clone the storage reference — the lifetime
    /// ties it to the backend (desktop keeps the file dir alive;
    /// mobile keeps the FFI handle; tests keep the `Arc<Mutex<...>>`).
    pub fn new(storage: &'a dyn WalletStorage) -> Self {
        Self { storage }
    }

    /// Serialize `mnemonic` + encrypt with `passphrase` + persist via
    /// `WalletStorage::put_atomic(id, blob)`.
    ///
    /// Returns the freshly minted `WalletId`. Two `create` calls in
    /// the same nanosecond yield distinct `WalletId`s (UUID v4 from
    /// the OS CSPRNG).
    pub fn create(&self, mnemonic: &Mnemonic, passphrase: &str) -> Result<WalletId> {
        let id = WalletId::new();
        let record = PlaintextRecord::from_mnemonic(mnemonic);

        let plaintext = serde_json::to_vec(&record)
            .map_err(|e| Error::Config(format!("serialize wallet record: {e}")))?;

        let blob = crypto::encrypt(&plaintext, passphrase.as_bytes())?;

        self.storage.put_atomic(&id, blob.as_bytes())?;
        Ok(id)
    }

    /// Read the encrypted blob, decrypt, deserialize, rebuild
    /// `Mnemonic` (which validates the BIP-39 checksum again).
    ///
    /// **Errors:**
    /// - `Error::Encryption` from `crypto::decrypt` on wrong passphrase
    ///   / corrupted blob (same error path — no password oracle)
    /// - `Error::Config` if the decrypted bytes aren't a valid
    ///   `PlaintextRecord` JSON or the phrase is BIP-39-invalid
    /// - `Error::Wallet` if `id` is not present in the storage backend
    /// - backend-specific error from `WalletStorage::get`
    pub fn unlock(&self, id: WalletId, passphrase: &str) -> Result<UnlockedWallet> {
        let raw_blob = self
            .storage
            .get(&id)?
            .ok_or_else(|| Error::Wallet(format!("wallet not found: {}", id.to_hex())))?;

        let blob = EncryptedWallet::from_blob(raw_blob)?;
        let plaintext = crypto::decrypt(&blob, passphrase.as_bytes())?;

        let record: PlaintextRecord = serde_json::from_slice(&plaintext)
            .map_err(|e| Error::Config(format!("deserialize wallet record: {e}")))?;

        let mnemonic = record.to_mnemonic(Language::English)?;
        Ok(UnlockedWallet { id, mnemonic })
    }

    /// List every wallet id the backend currently holds. Backends
    /// decide ordering (filesystem: sorted by filename; in-memory:
    /// insertion order; Keychain/EncryptedFile: backend-defined).
    pub fn list(&self) -> Result<Vec<WalletId>> {
        self.storage.list()
    }

    /// Remove a wallet. Idempotent — `WalletStorage::delete`
    /// does not error on missing id (per the trait contract).
    pub fn delete(&self, id: WalletId) -> Result<()> {
        self.storage.delete(&id)
    }
}
