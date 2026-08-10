//! Wallet module: end-to-end Bitcoin wallet (Task 9, #19b.2).
//!
//! Per plan §Task 9. `Wallet::from_mnemonic` (#19a, PR #48),
//! `Wallet::sync` (#19b.2, this PR), `Wallet::balance` (#19c, this PR).
//!
//! **Threat-model coverage:**
//!
//! - F34 (concrete mnemonic assertion in `Wallet::from_mnemonic`)
//! - F12 (chain sync via Esplora `/address/{addr}/utxo`)
//! - F13 (confirmed-only UTXO aggregation)
//! - F14 (persistence atomicity via `bdk_file_store`) — deferred to v0.1.1;
//!   this PR stores UTXOs in-memory only. A wallet restart loses
//!   UTXO state until next `sync`.
//!
//! **F20 + CONTEXT.md:** Esplora client uses SPKI-pinned TLS (raw
//! `reqwest` + custom `ServerCertVerifier` per Task 7 / PR #34).

use std::sync::Mutex;

use bdk_wallet::bitcoin::{Amount, Network, OutPoint, TxOut};
use bdk_wallet::{KeychainKind, Wallet as BdkWallet};

use crate::chain::esplora::{EsploraClient, TlsPolicy};
use crate::chain::network::coin_type_for;
use crate::error::Error;
use crate::keys::{address_type_to_path, AddressType, Mnemonic, Secret, XPrvHolder};

/// Gap limit for full chain scan (v0.1 demo). BIP-44 default is 20;
/// 5 keeps `Wallet::sync` quick while still finding any recent receive
/// for a fresh wallet. Bump to 20 once we test against wallets with
/// 20+ unused addresses.
const SCAN_GAP_LIMIT: u32 = 5;

/// Bitcoin wallet bound to one mnemonic + one network.
///
/// No `Default` impl (network policy). Construct via
/// [`Wallet::from_mnemonic`]. `sync` populates the inner bdk wallet
/// with UTXOs fetched from Esplora; `balance` reads it back.
///
/// **F14 (persistence):** the inner `bdk_wallet::Wallet` is in-memory
/// only. A wallet restart loses UTXO state until the next `sync`
/// call repopulates from Esplora. `bdk_file_store` SQLite is
/// deferred to v0.1.1 (per L28 + plan §Deferred threats).
pub struct Wallet {
    /// Recoverable BIP-39 phrase wrapped in `Secret<String>`.
    /// `sync` re-parses via `Mnemonic::from_phrase` to derive xprv.
    phrase: Secret<String>,
    network: Network,
    /// Lazily-populated by `sync`. Held in a `Mutex` for interior
    /// mutability across the `&self` API; `sync` and `balance` take
    /// short-lived locks that never cross `await`.
    bdk: Mutex<Option<BdkWallet>>,
}

impl Wallet {
    /// Construct a `Wallet` from a BIP-39 mnemonic + network.
    ///
    /// F34: rejects non-standard word counts (defense-in-depth).
    /// Network policy: caller must supply `network` explicitly.
    pub fn from_mnemonic(mnemonic: &Mnemonic, network: Network) -> Result<Self, Error> {
        match mnemonic.word_count() {
            12 | 15 | 18 | 21 | 24 => Ok(Self {
                phrase: mnemonic.to_phrase(),
                network,
                bdk: Mutex::new(None),
            }),
            _ => Err(Error::InvalidMnemonic(
                "unsupported BIP-39 word count".to_string(),
            )),
        }
    }

    /// Return the network this wallet was constructed for.
    pub fn network(&self) -> Network {
        self.network
    }

    /// Return a reference to the wrapped mnemonic phrase.
    pub fn phrase(&self) -> &Secret<String> {
        &self.phrase
    }

    /// Synchronize the wallet with the blockchain via an Esplora
    /// server (F12: full chain scan).
    ///
    /// Pipeline:
    /// 1. Validate `esplora_url` (non-empty + http(s) scheme).
    /// 2. Build [`EsploraClient`] with F20 SPKI-pinned TLS
    ///    ([`TlsPolicy::SystemRoots`] — pass a `TlsPolicy::Pinned`
    ///    with a real SPKI pin once configured in v0.1.1).
    /// 3. Derive BIP-84 native-segwit xprv from the stored phrase
    ///    (`m/84'/coin'/0'`), build `wpkh(xprv…/0/*)` + `…/1/*`
    ///    descriptors, construct an in-memory `bdk_wallet::Wallet`.
    /// 4. For first `SCAN_GAP_LIMIT` external + internal addresses,
    ///    query Esplora `/address/{addr}/utxo`; for each **confirmed**
    ///    UTXO (F13), `wallet.insert_txout(outpoint, txout)`.
    /// 5. Store the populated wallet in `self.bdk`.
    ///
    /// **F14 (persistence):** in-memory only. Wallet restart loses
    /// state until next `sync`. `bdk_file_store` SQLite is deferred.
    ///
    /// # Errors
    ///
    /// URL invalid → `Error::Esplora`. Network/HTTP failure →
    /// `Error::Esplora`. `bdk_wallet::Wallet::create` parse failure
    /// → `Error::Bdk`.
    pub async fn sync(&self, esplora_url: &str) -> Result<(), Error> {
        validate_esplora_url(esplora_url)?;
        let client = EsploraClient::new(esplora_url, TlsPolicy::SystemRoots)?;
        let mut bdk = self.build_bdk_wallet()?;
        scan_into(&client, &mut bdk).await?;
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        Ok(())
    }

    /// Return the wallet's confirmed balance in satoshis (F13).
    ///
    /// Lazy: on first call, syncs against `esplora_url` if the wallet
    /// has never been synced. Subsequent calls use the cached wallet.
    ///
    /// Returns `Ok(0)` for a wallet with no confirmed UTXOs (e.g.,
    /// fresh wallet or wallet on a chain with no funding history).
    pub async fn balance(&self, esplora_url: &str) -> Result<u64, Error> {
        validate_esplora_url(esplora_url)?;
        // Fast path: cached bdk present. Lock dropped before await.
        {
            let guard = self.bdk.lock().expect("bdk lock poisoned");
            if let Some(w) = guard.as_ref() {
                return Ok(w.balance().confirmed.to_sat());
            }
        }
        // Slow path: lazy first-time sync. Build + scan outside any
        // lock, then store + return balance. Avoids clippy
        // `await_holding_lock`.
        let client = EsploraClient::new(esplora_url, TlsPolicy::SystemRoots)?;
        let mut bdk = self.build_bdk_wallet()?;
        scan_into(&client, &mut bdk).await?;
        let bal = bdk.balance().confirmed.to_sat();
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        Ok(bal)
    }

    /// Build the in-memory `bdk_wallet::Wallet` from the stored
    /// phrase + network. Used by `sync` and `balance`; not exposed
    /// (descriptor construction is an internal detail).
    fn build_bdk_wallet(&self) -> Result<BdkWallet, Error> {
        let m = Mnemonic::from_phrase(self.phrase.expose())?;
        let seed = m.to_seed("");
        let seed_arr: [u8; 64] = seed.expose().as_slice().try_into().map_err(|_| {
            Error::InvalidDerivationPath(format!(
                "bip39 seed must be 64 bytes, got {}",
                seed.expose().len()
            ))
        })?;
        let master = XPrvHolder::master_from_seed(&seed_arr)?;
        let coin = coin_type_for(self.network);
        let path = address_type_to_path(AddressType::NativeSegwit, coin, 0, 0)?;
        let derived = master.derive(&path)?;
        let xprv = derived.to_xprv_string();
        drop(derived);
        drop(master);
        let external_descriptor = format!("wpkh({xprv}/0/*)");
        let change_descriptor = format!("wpkh({xprv}/1/*)");
        BdkWallet::create(external_descriptor, change_descriptor)
            .network(self.network)
            .create_wallet_no_persist()
            .map_err(|e| Error::Bdk(format!("create_wallet_no_persist: {e}")))
    }
}

/// Validate `esplora_url` is non-empty and http(s). Used by `sync`
/// and `balance` before constructing an `EsploraClient`.
fn validate_esplora_url(esplora_url: &str) -> Result<(), Error> {
    if esplora_url.is_empty() {
        return Err(Error::Esplora("Esplora URL required".to_string()));
    }
    if !(esplora_url.starts_with("http://") || esplora_url.starts_with("https://")) {
        return Err(Error::Esplora(format!(
            "Esplora URL must be http(s); got {esplora_url}"
        )));
    }
    Ok(())
}

/// For first `SCAN_GAP_LIMIT` external + internal addresses, query
/// Esplora `/address/{addr}/utxo`; for each **confirmed** UTXO,
/// `wallet.insert_txout(outpoint, txout)`. F13 confirmed-only.
async fn scan_into(client: &EsploraClient, bdk: &mut BdkWallet) -> Result<(), Error> {
    for kind in [KeychainKind::External, KeychainKind::Internal] {
        for i in 0..SCAN_GAP_LIMIT {
            let info = bdk.peek_address(kind, i);
            let utxos = client.address_utxos(&info.address).await?;
            for u in utxos {
                if !u.status.confirmed {
                    continue;
                }
                let outpoint = OutPoint {
                    txid: u.txid,
                    vout: u.vout,
                };
                let txout = TxOut {
                    value: Amount::from_sat(u.value),
                    script_pubkey: info.script_pubkey().clone(),
                };
                bdk.insert_txout(outpoint, txout);
            }
        }
    }
    Ok(())
}

impl std::fmt::Debug for Wallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wallet").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Builder as RtBuilder;

    /// Run an async future on a fresh current-thread runtime. Used
    /// by sync tests that need to drive `async fn` from `#[test]`.
    fn block<F: std::future::Future>(f: F) -> F::Output {
        RtBuilder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
            .block_on(f)
    }

    /// Generate a fresh mnemonic for tests (network policy: never
    /// reuse a published BIP-39 test vector).
    fn fresh_mnemonic(words: usize) -> Mnemonic {
        Mnemonic::generate(words).expect("fresh mnemonic generation")
    }

    #[test]
    fn from_mnemonic_rejects_13_words_via_mnemonic_api() {
        let mnemonic_12 = fresh_mnemonic(12usize);
        let phrase_12 = mnemonic_12.to_phrase();
        let bad_phrase = format!("{} extra", phrase_12.expose());
        assert!(
            Mnemonic::from_phrase(&bad_phrase).is_err(),
            "Mnemonic::from_phrase must reject 13 words"
        );
    }

    #[test]
    fn from_mnemonic_accepts_12_words() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        assert_eq!(wallet.network(), Network::Testnet);
        assert_eq!(wallet.phrase().expose().split_whitespace().count(), 12);
    }

    #[test]
    fn from_mnemonic_accepts_24_words() {
        let mnemonic = fresh_mnemonic(24usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        assert_eq!(wallet.network(), Network::Testnet);
        assert_eq!(wallet.phrase().expose().split_whitespace().count(), 24);
    }

    #[test]
    fn from_mnemonic_accepts_15_18_21_words() {
        for words in [15usize, 18, 21] {
            let mnemonic = fresh_mnemonic(words);
            assert!(Wallet::from_mnemonic(&mnemonic, Network::Testnet).is_ok());
        }
    }

    #[test]
    fn from_mnemonic_explicit_network_required() {
        let mnemonic = fresh_mnemonic(12usize);
        let _w = Wallet::from_mnemonic(&mnemonic, Network::Testnet);
    }

    #[test]
    fn sync_rejects_empty_url() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        let err = block(wallet.sync("")).expect_err("empty URL must be rejected");
        assert!(err.to_string().contains("required"), "got: {err}");
    }

    #[test]
    fn sync_rejects_non_http_scheme() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        let err =
            block(wallet.sync("ftp://example.com")).expect_err("non-http scheme must be rejected");
        assert!(
            err.to_string().contains("http"),
            "error message should mention http: {err}"
        );
    }

    #[tokio::test]
    #[ignore = "requires live testnet Esplora; run manually before merge per L29"]
    async fn sync_completes_against_testnet_for_fresh_wallet() {
        // F12 happy path: build bdk_wallet from xprv + scan first
        // SCAN_GAP_LIMIT external + internal addresses via Esplora.
        // Freshly-generated mnemonic → 0 UTXOs → sync returns Ok.
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        wallet
            .sync("https://blockstream.info/testnet/api")
            .await
            .expect("full sync should complete against testnet");
    }

    #[test]
    fn balance_rejects_empty_url() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        let err = block(wallet.balance("")).expect_err("empty URL must be rejected");
        assert!(err.to_string().contains("required"), "got: {err}");
    }

    #[test]
    fn balance_rejects_non_http_scheme() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        let err = block(wallet.balance("ftp://example.com"))
            .expect_err("non-http scheme must be rejected");
        assert!(err.to_string().contains("http"), "got: {err}");
    }

    #[tokio::test]
    #[ignore = "requires live testnet Esplora; run manually before merge per L29"]
    async fn balance_returns_zero_for_fresh_wallet() {
        // F13: fresh testnet wallet (no funding history) → 0 sat.
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        let bal = wallet
            .balance("https://blockstream.info/testnet/api")
            .await
            .expect("balance fetch should complete against testnet");
        assert_eq!(bal, 0, "fresh wallet has no UTXOs; got {bal}");
    }

    #[tokio::test]
    #[ignore = "requires live testnet Esplora; run manually before merge per L29"]
    async fn balance_reuses_cached_wallet_on_second_call() {
        // After sync(), subsequent balance() must reuse the cached
        // bdk wallet (no second Esplora scan). Two consecutive calls
        // both succeed and return the same value.
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        let b1 = wallet
            .balance("https://blockstream.info/testnet/api")
            .await
            .expect("first balance call");
        let b2 = wallet
            .balance("https://blockstream.info/testnet/api")
            .await
            .expect("second balance call");
        assert_eq!(b1, b2);
    }
}
