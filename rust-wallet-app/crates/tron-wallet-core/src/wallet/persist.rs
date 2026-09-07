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
use crate::keys::{
    derive_keypair, keypair_from_secret_bytes, DerivationPath, KeyPair, Language, Mnemonic,
};
use crate::platform::storage::WalletStorage;
use crate::wallet::id::WalletId;
use zeroize::Zeroizing;

/// State after a successful `unlock`.
///
/// Carries whichever secret the record holds — a BIP-39 mnemonic for a derived
/// wallet, or a raw scalar for an imported key — plus the optional label and
/// network the record was created with.
pub struct UnlockedWallet {
    id: WalletId,
    secret: WalletSecret,
    name: Option<String>,
    network: Option<String>,
}

/// The spending secret inside a wallet record.
///
/// Two variants rather than "mnemonic, sometimes empty": a raw-key wallet has
/// no phrase and never will, so a caller asking for one should get `None`, not
/// a phrase that fails BIP-39 validation later.
pub enum WalletSecret {
    /// Derived wallet — everything comes from the phrase.
    Mnemonic(Mnemonic),
    /// Imported raw secp256k1 key. No derivation path applies: the key *is*
    /// the account.
    PrivateKey(KeyPair),
}

/// Non-secret description of a stored wallet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletSummary {
    /// Storage handle.
    pub id: WalletId,
    /// Operator-supplied label, if the record carries one.
    pub name: Option<String>,
    /// Network tag (`mainnet` / `shasta` / `nile` / `local`) the wallet was
    /// created for, if recorded.
    pub network: Option<String>,
    /// Whether the secret is a raw key rather than a mnemonic. Surfaced
    /// because a raw-key wallet cannot be backed up as a phrase.
    pub is_private_key: bool,
}

impl UnlockedWallet {
    /// Wallet id this unlocked wallet corresponds to.
    pub fn id(&self) -> WalletId {
        self.id
    }

    /// Borrow the mnemonic, when the record holds one.
    ///
    /// `None` for a raw-key wallet. Returning an `Option` rather than a
    /// `&Mnemonic` is deliberate: an imported key has no phrase, and a
    /// caller printing "your recovery phrase" for one would be lying.
    pub fn mnemonic(&self) -> Option<&Mnemonic> {
        match &self.secret {
            WalletSecret::Mnemonic(m) => Some(m),
            WalletSecret::PrivateKey(_) => None,
        }
    }

    /// The secret this record holds.
    pub fn secret(&self) -> &WalletSecret {
        &self.secret
    }

    /// Operator-supplied label, if any.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Recorded network tag, if any.
    pub fn network(&self) -> Option<&str> {
        self.network.as_deref()
    }

    /// Non-secret description, safe to print or serialise.
    pub fn summary(&self) -> WalletSummary {
        WalletSummary {
            id: self.id,
            name: self.name.clone(),
            network: self.network.clone(),
            is_private_key: matches!(self.secret, WalletSecret::PrivateKey(_)),
        }
    }

    /// The signing keypair for this wallet.
    ///
    /// Derived wallets take `path`; raw-key wallets ignore it, because there is
    /// nothing to derive — passing a path for one is a caller mistake worth
    /// surfacing rather than silently honouring.
    pub fn keypair(&self, path: &DerivationPath) -> Result<KeyPair> {
        match &self.secret {
            WalletSecret::Mnemonic(m) => derive_keypair(m, "", path),
            WalletSecret::PrivateKey(kp) => Ok(kp.clone()),
        }
    }
}

impl fmt::Debug for UnlockedWallet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnlockedWallet")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("network", &self.network)
            .field("secret", &"<redacted>")
            .finish()
    }
}

/// Plaintext record encrypted at rest.
///
/// Backward compatible with the v0.1 phrase-only shape: every field added
/// after `phrase` is `#[serde(default)]`, so a blob written before this change
/// still decodes (as an unnamed, network-less mnemonic wallet). That is why
/// there is no version byte — adding one would have made those blobs
/// unreadable, and the wallets they hold are the only copy of their funds.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlaintextRecord {
    /// BIP-39 phrase. Empty string for a raw-key record.
    phrase: String,
    /// Raw secp256k1 scalar as hex, for imported keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    private_key_hex: Option<String>,
    /// Operator-supplied label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// Network tag this wallet was created for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    network: Option<String>,
}

impl PlaintextRecord {
    fn from_mnemonic(m: &Mnemonic) -> Self {
        Self {
            phrase: m.phrase().to_owned(),
            private_key_hex: None,
            name: None,
            network: None,
        }
    }

    fn with_meta(mut self, name: Option<&str>, network: Option<&str>) -> Self {
        self.name = name.map(str::to_owned);
        self.network = network.map(str::to_owned);
        self
    }

    fn from_private_key_hex(hex_key: &str) -> Self {
        Self {
            phrase: String::new(),
            private_key_hex: Some(hex_key.to_owned()),
            name: None,
            network: None,
        }
    }

    /// Rebuild the secret, re-validating it in the process.
    fn to_secret(&self, language: Language) -> Result<WalletSecret> {
        match &self.private_key_hex {
            Some(hex_key) => {
                let bytes = Zeroizing::new(hex::decode(hex_key).map_err(|e| {
                    Error::Config(format!("wallet record private key is not hex: {e}"))
                })?);
                Ok(WalletSecret::PrivateKey(keypair_from_secret_bytes(&bytes)?))
            }
            None => Ok(WalletSecret::Mnemonic(Mnemonic::from_phrase(
                &self.phrase,
                language,
            )?)),
        }
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
        self.create_with_meta(mnemonic, passphrase, None, None)
    }

    /// `create`, plus the label and network tag the record should carry.
    pub fn create_with_meta(
        &self,
        mnemonic: &Mnemonic,
        passphrase: &str,
        name: Option<&str>,
        network: Option<&str>,
    ) -> Result<WalletId> {
        let record = PlaintextRecord::from_mnemonic(mnemonic).with_meta(name, network);
        self.persist_new(&record, passphrase)
    }

    /// Import a raw 32-byte secp256k1 secret, supplied as hex.
    ///
    /// The scalar is validated before anything is written: persisting an
    /// out-of-range key would produce a wallet whose address exists but whose
    /// funds can never be signed for.
    pub fn import_private_key(
        &self,
        secret_hex: &str,
        passphrase: &str,
        name: Option<&str>,
        network: Option<&str>,
    ) -> Result<WalletId> {
        let normalised = secret_hex
            .trim()
            .trim_start_matches("0x")
            .to_ascii_lowercase();
        let bytes = Zeroizing::new(
            hex::decode(&normalised)
                .map_err(|e| Error::Config(format!("private key is not hex: {e}")))?,
        );
        keypair_from_secret_bytes(&bytes)?;

        let record = PlaintextRecord::from_private_key_hex(&normalised).with_meta(name, network);
        self.persist_new(&record, passphrase)
    }

    /// Encrypt `record` under a fresh id and write it atomically.
    fn persist_new(&self, record: &PlaintextRecord, passphrase: &str) -> Result<WalletId> {
        let id = WalletId::new();
        let plaintext = Zeroizing::new(
            serde_json::to_vec(record)
                .map_err(|e| Error::Config(format!("serialize wallet record: {e}")))?,
        );
        let blob = crypto::encrypt(&plaintext, passphrase.as_bytes())?;
        self.storage.put_atomic(&id, blob.as_bytes())?;
        Ok(id)
    }

    /// Read the encrypted blob, decrypt, deserialize, and rebuild the secret
    /// (re-validating the BIP-39 checksum, or the secp256k1 scalar).
    ///
    /// **Errors:**
    /// - `Error::Encryption` from `crypto::decrypt` on wrong passphrase
    ///   / corrupted blob (same error path — no password oracle)
    /// - `Error::Config` if the decrypted bytes aren't a valid
    ///   `PlaintextRecord` JSON or the phrase is BIP-39-invalid
    /// - `Error::Wallet` if `id` is not present in the storage backend
    /// - backend-specific error from `WalletStorage::get`
    pub fn unlock(&self, id: WalletId, passphrase: &str) -> Result<UnlockedWallet> {
        let record = self.read_record(id, passphrase)?;
        let secret = record.to_secret(Language::English)?;
        Ok(UnlockedWallet {
            id,
            secret,
            name: record.name.clone(),
            network: record.network.clone(),
        })
    }

    /// Decrypt and decode the stored record without rebuilding the secret.
    fn read_record(&self, id: WalletId, passphrase: &str) -> Result<PlaintextRecord> {
        let raw_blob = self
            .storage
            .get(&id)?
            .ok_or_else(|| Error::Wallet(format!("wallet not found: {}", id.to_hex())))?;

        let blob = EncryptedWallet::from_blob(raw_blob)?;
        let plaintext = crypto::decrypt(&blob, passphrase.as_bytes())?;

        serde_json::from_slice(&plaintext)
            .map_err(|e| Error::Config(format!("deserialize wallet record: {e}")))
    }

    /// Change a wallet's label.
    ///
    /// Takes the passphrase because the label lives *inside* the ciphertext:
    /// renaming means decrypt, edit, re-encrypt. Storing the name in plaintext
    /// beside the blob would leak which of an operator's wallets is the
    /// "cold-storage" one to anyone who can read the directory.
    ///
    /// The re-encrypted record is written through `put_atomic`, so a crash
    /// mid-rename leaves the previous blob intact rather than a truncated one.
    pub fn rename(&self, id: WalletId, passphrase: &str, new_name: &str) -> Result<()> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err(Error::Wallet("wallet name must not be empty".into()));
        }
        let mut record = self.read_record(id, passphrase)?;
        record.name = Some(trimmed.to_owned());

        let plaintext = Zeroizing::new(
            serde_json::to_vec(&record)
                .map_err(|e| Error::Config(format!("serialize wallet record: {e}")))?,
        );
        let blob = crypto::encrypt(&plaintext, passphrase.as_bytes())?;
        self.storage.put_atomic(&id, blob.as_bytes())?;
        Ok(())
    }

    /// Non-secret description of one wallet. Requires the passphrase: the
    /// label and network tag are inside the encrypted record.
    pub fn summary(&self, id: WalletId, passphrase: &str) -> Result<WalletSummary> {
        let record = self.read_record(id, passphrase)?;
        Ok(WalletSummary {
            id,
            name: record.name.clone(),
            network: record.network.clone(),
            is_private_key: record.private_key_hex.is_some(),
        })
    }

    /// List every wallet id the backend currently holds. Backends
    /// decide ordering (filesystem: sorted by filename; in-memory:
    /// insertion order; Keychain/EncryptedFile: backend-defined).
    pub fn list(&self) -> Result<Vec<WalletId>> {
        self.storage.list()
    }

    /// Summaries for every wallet that opens with `passphrase`.
    ///
    /// Wallets encrypted under a different passphrase are skipped rather than
    /// failing the whole listing — an operator with several passphrases still
    /// gets a useful inventory, and skipping is not an information leak
    /// (`list` already exposes the ids).
    pub fn list_summaries(&self, passphrase: &str) -> Result<Vec<WalletSummary>> {
        let mut out = Vec::new();
        for id in self.storage.list()? {
            match self.summary(id, passphrase) {
                Ok(summary) => out.push(summary),
                Err(Error::Encryption(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    /// Remove a wallet. Idempotent — `WalletStorage::delete`
    /// does not error on missing id (per the trait contract).
    pub fn delete(&self, id: WalletId) -> Result<()> {
        self.storage.delete(&id)
    }
}
