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
//! **Plaintext format inside the encrypted blob:** a `#[serde(untagged)]`
//! `PlaintextRecord` enum with one variant per secret shape. A
//! `{"phrase": "..."}` blob (any v0.1 version) decodes as `Mnemonic`;
//! a `{"hex": "..."}` blob decodes as `PrivateKey`. The "phrase-AND-hex"
//! state is unrepresentable — both variants carry `#[serde(deny_unknown_fields)]`,
//! so a JSON object with both keys fails to decode rather than silently
//! dropping one. An older `{"private_key_hex": "..."}` blob still decodes as
//! `PrivateKey` via the `alias` on the `hex` field.
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

/// What kind of secret a [`UnlockedWallet`] holds.
///
/// `is_private_key() -> bool` is the bool accessor — callers that already
/// branch on `kind` should match directly; the bool helper exists for
/// JSON output that wants a single field rather than a discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletKind {
    /// Derived from a BIP-39 phrase; every keypair comes from `derive_keypair`.
    Mnemonic,
    /// Imported as a raw 32-byte secp256k1 scalar; no derivation path applies.
    PrivateKey,
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
///
/// `#[non_exhaustive]` so future fields (created_at, last_unlocked_at,
/// derivation_path, …) can be added without breaking downstream code that
/// pattern-matches or constructs it. Construction stays possible from inside
/// the crate via explicit field init.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WalletSummary {
    /// Storage handle.
    pub id: WalletId,
    /// Operator-supplied label, if the record carries one.
    pub name: Option<String>,
    /// Network tag (`mainnet` / `shasta` / `nile` / `local`) the wallet was
    /// created for, if recorded.
    pub network: Option<String>,
    /// What kind of secret backs this wallet.
    pub kind: WalletKind,
}

impl WalletSummary {
    /// `true` for an imported raw-key wallet, `false` for a mnemonic-derived one.
    pub fn is_private_key(&self) -> bool {
        matches!(self.kind, WalletKind::PrivateKey)
    }
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

    /// What kind of secret backs this wallet.
    pub fn kind(&self) -> WalletKind {
        match &self.secret {
            WalletSecret::Mnemonic(_) => WalletKind::Mnemonic,
            WalletSecret::PrivateKey(_) => WalletKind::PrivateKey,
        }
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
            kind: self.kind(),
        }
    }

    /// The signing keypair for this wallet.
    ///
    /// **Mnemonic wallets:** `path` is honoured — a fresh derivation is
    /// performed each call, and the returned `KeyPair` is owned by the
    /// caller (with its secret inside `Zeroizing`).
    ///
    /// **Raw-key wallets:** this method errors out. A raw-key wallet has no
    /// derivation path — the secret scalar IS the account — so asking for
    /// "the keypair at path" is a category mistake. Use [`Self::raw_keypair`]
    /// to borrow the imported keypair directly.
    pub fn keypair(&self, path: &DerivationPath) -> Result<KeyPair> {
        match &self.secret {
            WalletSecret::Mnemonic(m) => derive_keypair(m, "", path),
            WalletSecret::PrivateKey(_) => Err(Error::Derivation(
                "raw-key wallets have no derivation path; use raw_keypair() instead".into(),
            )),
        }
    }

    /// Borrow the imported keypair for a raw-key wallet.
    ///
    /// **Raw-key wallets:** returns `&KeyPair` (zeroizing on drop). The
    /// returned reference is tied to `&self`; it does not extend the
    /// `KeyPair`'s lifetime, just exposes it.
    ///
    /// **Mnemonic wallets:** this method errors out. Use [`Self::keypair`]
    /// to derive a keypair at a path.
    ///
    /// `sign_prepared(&Zeroizing<[u8; SECRET_KEY_LEN]>)` callers can pass
    /// `raw_keypair().secret_bytes()` straight through; the borrow lasts
    /// as long as the `UnlockedWallet` does, which is longer than any
    /// single signing call.
    pub fn raw_keypair(&self) -> Result<&KeyPair> {
        match &self.secret {
            WalletSecret::PrivateKey(kp) => Ok(kp),
            WalletSecret::Mnemonic(_) => Err(Error::Derivation(
                "mnemonic wallets have no raw keypair to borrow; use keypair(path) instead".into(),
            )),
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
/// Two variants, not "phrase + optional hex": the "phrase-AND-hex-present"
/// state would be ambiguous (which one is the spending key?). Newtype
/// variants wrapping inner `#[serde(deny_unknown_fields)]` structs make
/// that ambiguity unrepresentable on the wire — `{"phrase":"x","hex":"y"}`
/// fails to decode because both inner types reject unknown fields, rather
/// than silently dropping one. Serde `untagged` then tries the next variant
/// after each failure, and the overall parse errors.
///
/// Backward compatibility:
/// - `{"phrase":"..."}` (any v0.1 version) → `Mnemonic`
/// - `{"private_key_hex":"..."}` (older camelCase) → `PrivateKey` via the
///   `alias` on the new `hex` field
/// - `{"hex":"..."}` → `PrivateKey`
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum PlaintextRecord {
    /// BIP-39 phrase plus optional label and network tag.
    Mnemonic(MnemonicRecord),
    /// Raw 32-byte secp256k1 scalar as hex, plus optional label and network tag.
    PrivateKey(PrivateKeyRecord),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MnemonicRecord {
    phrase: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    network: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrivateKeyRecord {
    /// Accepts both the new field name and the older `private_key_hex`
    /// (the field name before v0.1 added `name`/`network`).
    #[serde(alias = "private_key_hex")]
    hex: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    network: Option<String>,
}

impl PlaintextRecord {
    fn from_mnemonic(m: &Mnemonic, name: Option<&str>, network: Option<&str>) -> Self {
        Self::Mnemonic(MnemonicRecord {
            phrase: m.phrase().to_owned(),
            name: name.map(str::to_owned),
            network: network.map(str::to_owned),
        })
    }

    fn from_private_key_hex(hex_key: &str, name: Option<&str>, network: Option<&str>) -> Self {
        Self::PrivateKey(PrivateKeyRecord {
            hex: hex_key.to_owned(),
            name: name.map(str::to_owned),
            network: network.map(str::to_owned),
        })
    }

    fn name(&self) -> Option<&str> {
        match self {
            Self::Mnemonic(r) => r.name.as_deref(),
            Self::PrivateKey(r) => r.name.as_deref(),
        }
    }

    fn network(&self) -> Option<&str> {
        match self {
            Self::Mnemonic(r) => r.network.as_deref(),
            Self::PrivateKey(r) => r.network.as_deref(),
        }
    }

    /// Rebuild the secret, re-validating it in the process.
    fn to_secret(&self, language: Language) -> Result<WalletSecret> {
        match self {
            Self::Mnemonic(MnemonicRecord { phrase, .. }) => Ok(WalletSecret::Mnemonic(
                Mnemonic::from_phrase(phrase, language)?,
            )),
            Self::PrivateKey(PrivateKeyRecord { hex, .. }) => {
                let bytes = Zeroizing::new(hex::decode(hex).map_err(|e| {
                    Error::Config(format!("wallet record private key is not hex: {e}"))
                })?);
                Ok(WalletSecret::PrivateKey(keypair_from_secret_bytes(&bytes)?))
            }
        }
    }
}

/// Encrypted-wallet persistence layer. Takes any `WalletStorage` impl
/// at construction; the same instance is usable across many wallet
// create/unlock cycles.
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
        let record = PlaintextRecord::from_mnemonic(mnemonic, name, network);
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

        let record = PlaintextRecord::from_private_key_hex(&normalised, name, network);
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
            name: record.name().map(str::to_owned),
            network: record.network().map(str::to_owned),
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
        let owned = trimmed.to_owned();
        match &mut record {
            PlaintextRecord::Mnemonic(r) => r.name = Some(owned),
            PlaintextRecord::PrivateKey(r) => r.name = Some(owned),
        }

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
            name: record.name().map(str::to_owned),
            network: record.network().map(str::to_owned),
            kind: match &record {
                PlaintextRecord::Mnemonic(_) => WalletKind::Mnemonic,
                PlaintextRecord::PrivateKey(_) => WalletKind::PrivateKey,
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A legacy phrase-only blob (the v0.1 wire format) must round-trip
    /// through the new untagged enum without any code knowing the field
    /// names changed. Decoded as `Mnemonic(MnemonicRecord { phrase, name: None, network: None })`.
    #[test]
    fn a_legacy_phrase_only_blob_decodes_as_mnemonic() {
        let legacy = r#"{"phrase":"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"}"#;
        let decoded: PlaintextRecord = serde_json::from_str(legacy).expect("legacy blob decodes");
        match decoded {
            PlaintextRecord::Mnemonic(MnemonicRecord {
                phrase,
                name,
                network,
            }) => {
                assert!(phrase.starts_with("abandon"));
                assert!(name.is_none());
                assert!(network.is_none());
            }
            PlaintextRecord::PrivateKey(_) => panic!("expected Mnemonic variant"),
        }
    }

    /// A blob carrying both a phrase and a hex key is a corrupt record.
    /// The new enum must reject it rather than silently picking one.
    #[test]
    fn a_wallet_record_cannot_carry_both_phrase_and_hex() {
        let both = r#"{"phrase":"x","hex":"00"}"#;
        assert!(
            serde_json::from_str::<PlaintextRecord>(both).is_err(),
            "phrase+hex must fail to decode (deny_unknown_fields on both inner structs)"
        );
    }

    /// A legacy camelCase `private_key_hex` blob decodes as `PrivateKey`
    /// via the alias on the new `hex` field.
    #[test]
    fn a_legacy_private_key_hex_alias_still_decodes() {
        let legacy = r#"{"private_key_hex":"00"}"#;
        let decoded: PlaintextRecord =
            serde_json::from_str(legacy).expect("legacy private_key_hex alias decodes");
        match decoded {
            PlaintextRecord::PrivateKey(PrivateKeyRecord { hex, .. }) => assert_eq!(hex, "00"),
            PlaintextRecord::Mnemonic(_) => panic!("expected PrivateKey variant"),
        }
    }

    /// Mnemonic variant round-trips through serde with no `hex` field appearing
    /// in the output.
    #[test]
    fn a_mnemonic_record_round_trips() {
        let rec = PlaintextRecord::from_mnemonic(
            &Mnemonic::from_phrase(
                "abandon abandon abandon abandon abandon abandon \
                 abandon abandon abandon abandon abandon about",
                Language::English,
            )
            .expect("valid phrase"),
            Some("cold"),
            Some("nile"),
        );
        let s = serde_json::to_string(&rec).expect("serialize");
        assert!(
            s.contains("\"phrase\""),
            "mnemonic record contains phrase field: {s}"
        );
        assert!(
            !s.contains("\"hex\""),
            "mnemonic record must not leak a hex field: {s}"
        );

        let back: PlaintextRecord = serde_json::from_str(&s).expect("round-trip");
        assert!(matches!(back, PlaintextRecord::Mnemonic(_)));
        assert_eq!(back.name(), Some("cold"));
        assert_eq!(back.network(), Some("nile"));
    }

    /// PrivateKey variant round-trips through serde with no `phrase` field.
    #[test]
    fn a_raw_key_record_round_trips() {
        let rec = PlaintextRecord::from_private_key_hex(
            "e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35",
            Some("paper"),
            Some("mainnet"),
        );
        let s = serde_json::to_string(&rec).expect("serialize");
        assert!(
            s.contains("\"hex\""),
            "private-key record contains hex field: {s}"
        );
        assert!(
            !s.contains("\"phrase\""),
            "private-key record must not leak a phrase field: {s}"
        );

        let back: PlaintextRecord = serde_json::from_str(&s).expect("round-trip");
        assert!(matches!(back, PlaintextRecord::PrivateKey(_)));
        assert_eq!(back.name(), Some("paper"));
        assert_eq!(back.network(), Some("mainnet"));
    }

    /// `WalletSummary::is_private_key` mirrors `kind` for the bool accessor
    /// that JSON callers rely on.
    #[test]
    fn wallet_summary_kind_reflects_secret() {
        let mnemonic_summary = WalletSummary {
            id: WalletId::new(),
            name: None,
            network: None,
            kind: WalletKind::Mnemonic,
        };
        assert!(!mnemonic_summary.is_private_key());
        assert_eq!(mnemonic_summary.kind, WalletKind::Mnemonic);

        let raw_summary = WalletSummary {
            id: WalletId::new(),
            name: None,
            network: None,
            kind: WalletKind::PrivateKey,
        };
        assert!(raw_summary.is_private_key());
        assert_eq!(raw_summary.kind, WalletKind::PrivateKey);
    }
}
