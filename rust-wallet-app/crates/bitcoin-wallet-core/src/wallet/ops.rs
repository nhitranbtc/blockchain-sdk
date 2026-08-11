//! Wallet operations (Task 54d / Issue #64).
//!
//! High-level `create_wallet` + `show_wallet` library functions backing
//! the `btc wallet create` / `btc wallet show` CLI subcommands. Lives in
//! `bitcoin-wallet-core` (not `btc`) so the library is testable
//! independent of the CLI binary and can be reused by future consumers
//! (library-first design).
//!
//! **Layering:**
//!
//! ```text
//! btc CLI (main.rs)
//!   └─> wallet::ops::create_wallet / show_wallet  (this file)
//!         ├─> wallet::store::write_wallet_at / read_wallet_at
//!         ├─> crypto::mnemonic_cipher::encrypt_mnemonic / decrypt_mnemonic
//!         ├─> crypto::aad::Aad::network
//!         └─> wallet::Wallet (sync + balance)
//! ```
//!
//! **Testability:** functions take an explicit `base: &Path` so unit
//! tests can use `tempfile::tempdir()`. The CLI resolves `base` via
//! [`crate::wallet::store::data_dir`] at the call site.

use std::path::Path;

use bdk_wallet::bitcoin::{Address, Network};
use serde::Serialize;

use crate::chain::esplora::EsploraClient;
use crate::crypto::aad::Aad;
use crate::crypto::mnemonic_cipher::{decrypt_mnemonic, encrypt_mnemonic, MnemonicCipherBlob};
use crate::error::{Error, Result};
use crate::keys::{Mnemonic, Secret};
use crate::wallet::id::WalletId;
use crate::wallet::store::{read_wallet_at, write_wallet_at};
use crate::wallet::Wallet;
use bdk_wallet::KeychainKind;

/// Result of `show_wallet` — addresses + confirmed balance (in satoshis).
///
/// Serialized to JSON by the CLI for machine consumption (per Story
/// #13's "playaround-able" goal). Field names use snake_case to match
/// the rest of the CLI's JSON output.
#[derive(Debug, Clone, Serialize)]
pub struct WalletInfo {
    /// Wallet's first 5 external-chain receive addresses (BIP-44
    /// external chain = 0).
    pub receive_addresses: Vec<Address>,
    /// Wallet's first 5 internal-chain (change) addresses.
    pub change_addresses: Vec<Address>,
    /// Confirmed balance in satoshis. `0` for a wallet with no
    /// confirmed UTXOs.
    pub balance_sat: u64,
}

/// Supported BIP-39 word counts per F34. Mirrored here so the CLI
/// validates before calling [`Mnemonic::generate`].
pub const SUPPORTED_WORD_COUNTS: &[usize] = &[12, 15, 18, 21, 24];

/// Generate a new random mnemonic + persist the encrypted wallet blob to
/// `base`. Returns `(WalletId, Secret<String>)` — the phrase is wrapped
/// in `Secret<String>` (zeroize-on-drop, F47) so the CLI can print it to
/// STDERR (with banner) and then drop it; the on-disk form is the
/// `MnemonicCipherBlob` bytes via `store::write_wallet_at`.
///
/// **L28 honesty** (ADR §Failure modes): the CLI is responsible for
/// routing the mnemonic to STDERR (with banner) — never to STDOUT.
/// This library function deliberately returns the mnemonic to the caller
/// so the CLI controls the print destination; the library does not
/// perform any IO on stdout/stderr itself.
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
    // 1. Generate mnemonic.
    let mnemonic = Mnemonic::generate(words)?;
    let phrase = mnemonic.to_phrase();
    // 2. Generate wallet ID.
    let id = WalletId::new();
    // 3. Encrypt mnemonic with AAD = network discriminant (closes N5).
    let aad = Aad::network(network);
    let blob = encrypt_mnemonic(&phrase, password.expose().as_slice(), aad)?;
    // 4. Write atomically with 0o600 files + 0o700 dirs.
    write_wallet_at(base, network, id, blob.as_bytes())?;
    // 5. Return (id, phrase) — caller routes phrase to STDERR with banner.
    Ok((id, phrase))
}

/// Load an existing wallet from `base`, decrypt with `password`, sync
/// against `esplora`, return addresses + confirmed balance.
///
/// **Failure modes** (ADR §Failure modes):
///
/// - Wallet not found → constant-time-padded (N8) → returns the
///   indistinguishable `Error::WalletStore("wallet not accessible ...")`
///   message (N2).
/// - Symlink at blob path → `Error::WalletStore("... is a symlink ...")`
///   (separate message; not a confidentiality concern — the symlink is
///   already a refused read).
/// - AEAD verification fails (wrong password, wrong network, or
///   corrupt blob) → indistinguishable `Error::WalletStore(...)`.
/// - Esplora unreachable → `Error::Esplora` (T4 availability DoS;
///   propagates).
pub async fn show_wallet(
    base: &Path,
    network: Network,
    id: WalletId,
    password: &Secret<Vec<u8>>,
    esplora: &EsploraClient,
) -> Result<WalletInfo> {
    // Read blob (with symlink defense). Map NOT-FOUND to padded path.
    let blob_bytes = match read_wallet_at(base, network, id) {
        Ok(b) => b,
        Err(Error::Io(_)) => {
            // Missing file: pad timing to match wrong-password path.
            // L12 review HIGH #3 (code-reviewer) + security HIGH #3:
            // pad here AND on the try_from failure arm below, so all 4
            // collapse paths (missing/wrong-pw/wrong-AAD/corrupt-blob)
            // take the same wall-clock.
            crate::wallet::store::constant_time_padding();
            return Err(Error::WalletStore(
                crate::wallet::store::WALLET_NOT_ACCESSIBLE.into(),
            ));
        }
        Err(e) => return Err(e),
    };
    // Parse + decrypt. Try_from failure (truncated/oversized blob) also
    // gets the timing pad to match the wrong-password path.
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
    // Reconstruct the wallet from the mnemonic.
    let mnemonic = Mnemonic::from_phrase(phrase_secret.expose())?;
    let wallet = Wallet::from_mnemonic(&mnemonic, network)?;
    // Sync + read addresses + balance.
    wallet.sync(esplora).await?;
    let balance = wallet.balance(esplora).await?;
    // Derive the first 5 external + 5 internal addresses for display.
    // (Gap limit 5 matches SCAN_GAP_LIMIT in wallet/mod.rs; the wallet
    // has already scanned these during sync, so peek_address is cheap.)
    let receive_addresses = wallet.peek_addresses(KeychainKind::External, 5);
    let change_addresses = wallet.peek_addresses(KeychainKind::Internal, 5);
    Ok(WalletInfo {
        receive_addresses,
        change_addresses,
        balance_sat: balance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::esplora::TlsPolicy;
    use std::os::unix::fs::PermissionsExt;

    fn password() -> Secret<Vec<u8>> {
        Secret::new(b"test-password-not-a-bip39-phrase".to_vec())
    }

    /// Unit test: create_wallet generates a mnemonic, persists the
    /// blob, returns (id, phrase) tuple.
    #[test]
    fn create_wallet_persists_blob_and_returns_id_mnemonic() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path();
        let (id, phrase) = create_wallet(base, Network::Testnet, 12, &password()).expect("create");
        assert_eq!(phrase.expose().split_whitespace().count(), 12);
        let blob_path = crate::wallet::store::wallet_path_at(base, Network::Testnet, id);
        assert!(blob_path.exists(), "blob must exist on disk");
        let mode = std::fs::metadata(&blob_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    /// Unit test: create_wallet rejects unsupported word counts.
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

    /// Unit test: create_wallet accepts all supported word counts.
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

    /// Unit test: missing wallet surfaces the indistinguishable
    /// WALLET_NOT_ACCESSIBLE message (N2 closure witness).
    /// Uses a dummy EsploraClient — sync is never reached because the
    /// blob doesn't exist (constant-time padding → WALLET_NOT_ACCESSIBLE).
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
            ))
            .expect_err("missing wallet must reject");
        assert!(matches!(err, Error::WalletStore(_)));
        assert!(
            err.to_string().contains("wallet not accessible"),
            "must use the indistinguishable message; got: {err}"
        );
    }
}
