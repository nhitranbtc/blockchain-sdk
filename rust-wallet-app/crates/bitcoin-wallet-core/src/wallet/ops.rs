//! Wallet operations (Task 54d / Issue #64).
//!
//! High-level `create_wallet` + `show_wallet` library functions backing
//! `btc wallet create` / `btc wallet show` CLI subcommands. Lives in
//! `bitcoin-wallet-core` (not `btc`) so the library is testable
//! independent of the CLI binary.
//!
//! Functions take an explicit `base: &Path` so unit tests use
//! `tempfile::tempdir()`. Production callers resolve `base` via
//! `wallet::store::data_dir()` at the call site.

use std::path::Path;

use bdk_wallet::bitcoin::{Address, Network};
use bdk_wallet::KeychainKind;
use serde::Serialize;

use crate::chain::esplora::EsploraClient;
use crate::crypto::aad::Aad;
use crate::crypto::mnemonic_cipher::{decrypt_mnemonic, encrypt_mnemonic, MnemonicCipherBlob};
use crate::error::{Error, Result};
use crate::keys::{Mnemonic, Secret};
use crate::wallet::id::WalletId;
use crate::wallet::store::{read_wallet_at, write_wallet_at};
use crate::wallet::Wallet;

/// Result of `show_wallet` — addresses + confirmed balance (in satoshis).
///
/// `#[non_exhaustive]` allows future field additions without breaking
/// destructuring callers (serde consumers tolerate extra fields by default).
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct WalletInfo {
    /// Wallet's first 5 external-chain receive addresses.
    pub receive_addresses: Vec<Address>,
    /// Wallet's first 5 internal-chain (change) addresses.
    pub change_addresses: Vec<Address>,
    /// Confirmed balance in satoshis. `0` for a wallet with no confirmed UTXOs.
    pub balance_sat: u64,
}

/// Supported BIP-39 word counts per F34. Validated before `Mnemonic::generate`.
pub const SUPPORTED_WORD_COUNTS: &[usize] = &[12, 15, 18, 21, 24];

/// Generate a new random mnemonic + persist the encrypted wallet blob.
/// Returns `(WalletId, Secret<String>)` — CLI routes the phrase to STDERR
/// with banner (per L28); library returns it so the CLI controls the
/// print destination.
pub fn create_wallet(
    base: &Path,
    network: Network,
    words: usize,
    password: &Secret<Vec<u8>>,
) -> Result<(WalletId, Secret<String>)> {
    if !SUPPORTED_WORD_COUNTS.contains(&words) {
        return Err(Error::InvalidMnemonic(format!(
            "unsupported BIP-39 word count: {words} (supported: {SUPPORTED_WORD_COUNTS:?})"
        )));
    }
    let mnemonic = Mnemonic::generate(words)?;
    let phrase = mnemonic.to_phrase();
    let id = WalletId::new();
    let aad = Aad::network(network);
    let blob = encrypt_mnemonic(&phrase, password.expose().as_slice(), aad)?;
    write_wallet_at(base, network, id, blob.as_bytes())?;
    Ok((id, phrase))
}

/// Import an existing BIP-39 mnemonic phrase + persist the encrypted
/// wallet blob. Returns the new `WalletId`; the phrase is not echoed
/// back (caller already has it). No mnemonic generation.
///
/// **Story 2 / Issue #99.** Validates word count + checksum via
/// `Mnemonic::from_phrase`; binds encryption AAD to the network
/// discriminant (per ADR 0001) so cross-network decryption fails.
///
/// BIP-39 passphrase (`--passphrase`) is NOT persisted — it is
/// derivation-time only. Pass it again at `show_wallet` time if you
/// need to derive from a passphrase-protected mnemonic. The encrypted
/// blob stores the mnemonic phrase alone.
pub fn import_wallet(
    base: &Path,
    network: Network,
    phrase: &str,
    password: &Secret<Vec<u8>>,
) -> Result<WalletId> {
    // Validate word count + BIP-39 checksum. `from_phrase` rejects
    // both unsupported word counts and invalid checksums with
    // `Error::InvalidMnemonic` (mapped via `map_bip39_error`).
    let _mnemonic = Mnemonic::from_phrase(phrase)?;
    let id = WalletId::new();
    let aad = Aad::network(network);
    // Wrap phrase in Secret<String> to satisfy `encrypt_mnemonic`'s
    // signature; the caller already has the phrase in plaintext (it
    // came from `--mnemonic` CLI flag), so this is defense-in-depth,
    // not a new exposure surface.
    let phrase_secret = Secret::new(phrase.to_string());
    let blob = encrypt_mnemonic(&phrase_secret, password.expose().as_slice(), aad)?;
    write_wallet_at(base, network, id, blob.as_bytes())?;
    Ok(id)
}

/// Load existing wallet from `base`, decrypt with `password`, sync
/// against `esplora`, return addresses + confirmed balance.
///
/// All 4 collapse paths (missing/wrong-pw/wrong-network-AAD/corrupt-blob)
/// take the same wall-clock via `constant_time_padding()` per branch,
/// and surface the indistinguishable `WALLET_NOT_ACCESSIBLE` message.
pub async fn show_wallet(
    base: &Path,
    network: Network,
    id: WalletId,
    password: &Secret<Vec<u8>>,
    esplora: &EsploraClient,
    db_path: Option<&Path>,
) -> Result<WalletInfo> {
    let blob_bytes = match read_wallet_at(base, network, id) {
        Ok(b) => b,
        // L12 review HIGH #1: collapse ALL existence-revealing errors
        // (file-not-found + symlink-at-blob-path + oversize-blob) to the
        // indistinguishable WALLET_NOT_ACCESSIBLE message + pad timing.
        // Non-existence-revealing IO errors (perms, disk full) propagate
        // so the operator can diagnose the real problem.
        Err(Error::Io(_)) | Err(Error::WalletStore(_)) => {
            crate::wallet::store::constant_time_padding();
            return Err(Error::WalletStore(
                crate::wallet::store::WALLET_NOT_ACCESSIBLE.into(),
            ));
        }
        Err(e) => return Err(e),
    };
    let blob = match MnemonicCipherBlob::try_from(blob_bytes.as_slice()) {
        Ok(b) => b,
        Err(_) => {
            crate::wallet::store::constant_time_padding();
            return Err(Error::WalletStore(
                crate::wallet::store::WALLET_NOT_ACCESSIBLE.into(),
            ));
        }
    };
    let aad = Aad::network(network);
    let phrase_secret = decrypt_mnemonic(&blob, password.expose().as_slice(), aad)
        .map_err(|_| Error::WalletStore(crate::wallet::store::WALLET_NOT_ACCESSIBLE.into()))?;
    let mnemonic = Mnemonic::from_phrase(phrase_secret.expose())?;
    let wallet = Wallet::from_mnemonic(&mnemonic, network, db_path.map(|p| p.to_path_buf()))?;
    wallet.sync(esplora).await?;
    let balance = wallet.balance(esplora).await?;
    let receive_addresses = wallet
        .peek_addresses(KeychainKind::External, 5)
        .map_err(|_| Error::WalletStore("cannot peek wallet addresses".into()))?;
    let change_addresses = wallet
        .peek_addresses(KeychainKind::Internal, 5)
        .map_err(|_| Error::WalletStore("cannot peek wallet addresses".into()))?;

    // Story 12 / Issue #130 PR3-CLI: if `db_path` is set, write the
    // wallet's `ChangeSet` to the bdk_file_store so subsequent
    // `btc wallet show --db-path <path>` invocations can reload
    // without re-syncing from Esplora.
    if db_path.is_some() {
        wallet
            .persist()
            .map_err(|e| Error::WalletStore(format!("Wallet::persist failed: {e}")))?;
    }

    Ok(WalletInfo {
        receive_addresses,
        change_addresses,
        balance_sat: balance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    use crate::chain::esplora::TlsPolicy;

    fn password() -> Secret<Vec<u8>> {
        Secret::new(b"test-password-not-a-bip39-phrase".to_vec())
    }

    #[test]
    fn create_wallet_persists_blob_and_returns_id_phrase() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path();
        let (id, phrase) = create_wallet(base, Network::Testnet, 12, &password()).expect("create");
        assert_eq!(phrase.expose().split_whitespace().count(), 12);
        let blob_path = crate::wallet::store::wallet_path_at(base, Network::Testnet, id);
        assert!(blob_path.exists(), "blob must exist on disk");
        let mode = std::fs::metadata(&blob_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn create_wallet_rejects_unsupported_word_count() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path();
        for bad in [0usize, 1, 7, 13, 14, 22, 23, 25, 100] {
            let err =
                create_wallet(base, Network::Testnet, bad, &password()).expect_err("must reject");
            assert!(matches!(err, Error::InvalidMnemonic(_)), "word count {bad}");
        }
    }

    #[test]
    fn create_wallet_accepts_supported_word_counts() {
        for words in [12usize, 15, 18, 21, 24] {
            let tmp = tempfile::tempdir().expect("tmpdir");
            let base = tmp.path();
            let (id, phrase) =
                create_wallet(base, Network::Testnet, words, &password()).expect("create");
            assert_eq!(phrase.expose().split_whitespace().count(), words);
            assert!(crate::wallet::store::wallet_path_at(base, Network::Testnet, id).exists());
        }
    }

    // -- import_wallet tests (Issue #99 / Story 2) -----------------------

    /// BIP-39 standard test vector (12-word, valid checksum).
    const IMPORT_PHRASE_12: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    /// BIP-39 standard test vector (24-word, valid checksum).
    const IMPORT_PHRASE_24: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";

    #[test]
    fn import_wallet_accepts_valid_12_word_phrase() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path();
        let id = import_wallet(base, Network::Testnet, IMPORT_PHRASE_12, &password())
            .expect("import must succeed for valid 12-word phrase");
        let blob_path = crate::wallet::store::wallet_path_at(base, Network::Testnet, id);
        assert!(
            blob_path.exists(),
            "imported wallet blob must exist on disk"
        );
        let mode = std::fs::metadata(&blob_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "imported blob must be 0o600");
    }

    #[test]
    fn import_wallet_accepts_valid_24_word_phrase() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path();
        let id = import_wallet(base, Network::Bitcoin, IMPORT_PHRASE_24, &password())
            .expect("import must succeed for valid 24-word phrase");
        assert!(crate::wallet::store::wallet_path_at(base, Network::Bitcoin, id).exists());
    }

    #[test]
    fn import_wallet_rejects_invalid_checksum() {
        // Same 12 words but with last word changed — checksum broken.
        let bad = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path();
        let err = import_wallet(base, Network::Testnet, bad, &password())
            .expect_err("must reject invalid checksum");
        assert!(
            matches!(err, Error::InvalidMnemonic(_)),
            "expected InvalidMnemonic, got {err:?}"
        );
    }

    #[test]
    fn import_wallet_rejects_unsupported_word_count() {
        // 13 words is not a BIP-39 supported length.
        let bad = "abandon ".repeat(13);
        let bad = bad.trim_end();
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path();
        let err = import_wallet(base, Network::Testnet, bad, &password())
            .expect_err("must reject unsupported word count");
        assert!(matches!(err, Error::InvalidMnemonic(_)), "got {err:?}");
    }

    #[test]
    fn import_wallet_persists_distinct_ids_for_same_phrase() {
        // Each import gets a fresh WalletId (UUID). Two imports of the
        // same phrase persist two distinct blobs.
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path();
        let id_a =
            import_wallet(base, Network::Testnet, IMPORT_PHRASE_12, &password()).expect("import A");
        let id_b =
            import_wallet(base, Network::Testnet, IMPORT_PHRASE_12, &password()).expect("import B");
        assert_ne!(id_a, id_b);
        assert!(crate::wallet::store::wallet_path_at(base, Network::Testnet, id_a).exists());
        assert!(crate::wallet::store::wallet_path_at(base, Network::Testnet, id_b).exists());
    }

    /// Witness: missing-wallet surfaces the indistinguishable
    /// `WALLET_NOT_ACCESSIBLE` message (N2 closure).
    #[test]
    fn missing_wallet_message_is_indistinguishable() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path();
        let id = WalletId::new();
        let dummy_url =
            crate::chain::esplora_url::EsploraUrl::new("https://blockstream.info/testnet/api")
                .expect("url");
        let esplora = EsploraClient::new(dummy_url, TlsPolicy::SystemRoots).expect("client");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let err = rt
            .block_on(show_wallet(
                base,
                Network::Testnet,
                id,
                &password(),
                &esplora,
                None,
            ))
            .expect_err("missing wallet must reject");
        assert!(matches!(err, Error::WalletStore(_)));
        assert!(err.to_string().contains("wallet not accessible"));
    }
}
