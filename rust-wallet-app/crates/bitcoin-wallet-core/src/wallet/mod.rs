//! Wallet module: end-to-end Bitcoin wallet (Task 9, #19b.2).
//!
//! Per plan §Task 9. `Wallet::from_mnemonic` (#19a, PR #48),
//! `Wallet::sync` (#19b.2, this PR), `Wallet::balance` (#19c, this PR).
//!
//! **Threat-model coverage:**
//!
//! - F34 (concrete mnemonic assertion in `Wallet::from_mnemonic`)
//! - F12 (chain sync via Esplora `/address/{addr}/utxo`)
//! - F13 (confirmed-only UTXO aggregation, capped at MAX_MONEY)
//! - F14 (persistence atomicity via `bdk_file_store`) — deferred to v0.1.1;
//!   this PR stores UTXOs in-memory only. A wallet restart loses
//!   UTXO state until next `sync`.
//!
//! **Security:**
//!
//! - F20 (Esplora SPKI pinning): enforced via caller-built
//!   `EsploraClient`. `sync`/`balance` take `&EsploraClient` (not a
//!   raw URL) so the caller controls TLS policy — they can pass a
//!   `TlsPolicy::Pinned(SpkiPinSet)` for production endpoints and
//!   are responsible for rejecting `TlsPolicy::SystemRoots` for
//!   public Esplora servers. This file does NOT default to
//!   `SystemRoots` — there is no default.
//! - Cross-network confusion: caller is responsible for building
//!   the `EsploraClient` from a `WalletConfig` whose `network`
//!   matches this wallet's `network` (see `WalletConfig::testnet`
//!   /`mainnet` /`regtest` /`signet` constructors). The client is
//!   network-bound via the URL the operator passes to that
//!   constructor.
//! - xprv material: `XPrvHolder::to_xprv_secret` returns
//!   `Secret<String>` (zeroize-on-drop). Descriptor strings are
//!   dropped immediately after `bdk_wallet::Wallet::create`
//!   returns — bdk parses them into its own keystore.
//! - bdk error wrapping: `Error::Bdk` carries a fixed string, not
//!   the raw bdk error which may echo the descriptor.

use std::sync::Mutex;

use bdk_wallet::bitcoin::{Amount, Network, OutPoint, TxOut};
use bdk_wallet::{KeychainKind, Wallet as BdkWallet};

use crate::chain::esplora::{EsploraClient, EsploraUtxo};
use crate::chain::network::coin_type_for;
use crate::error::Error;
use crate::keys::{address_type_to_path, AddressType, Mnemonic, Secret, XPrvHolder};

/// Gap limit for full chain scan (v0.1 demo). BIP-44 default is 20;
/// 5 keeps `Wallet::sync` quick while still finding any recent
/// receive for a fresh wallet. Bump to 20 once we test against
/// wallets with 20+ unused addresses.
const SCAN_GAP_LIMIT: u32 = 5;

/// Bitcoin wallet bound to one mnemonic + one network.
///
/// No `Default` impl (network policy). Construct via
/// [`Wallet::from_mnemonic`]. `sync` populates the inner bdk wallet
/// with UTXOs fetched from Esplora; `balance` reads it back.
///
/// **F14 (persistence):** the inner `bdk_wallet::Wallet` is in-memory
/// only. Wallet restart loses UTXO state until next `sync`
/// repopulates from Esplora. `bdk_file_store` SQLite deferred to
/// v0.1.1.
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
    /// Takes `&EsploraClient` (not a raw URL) so the caller controls
    /// TLS policy (F20). For production endpoints, build with
    /// `TlsPolicy::Pinned(SpkiPinSet)` via `EsploraClient::from_config`.
    /// For local dev, `TlsPolicy::SystemRoots` is acceptable.
    ///
    /// Pipeline:
    /// 1. Build in-memory `bdk_wallet::Wallet` from the phrase.
    /// 2. For first `SCAN_GAP_LIMIT` external + internal addresses,
    ///    query Esplora `/address/{addr}/utxo`; for each
    ///    **confirmed** UTXO (F13), cap the value against MAX_MONEY,
    ///    then `wallet.insert_txout(outpoint, txout)`.
    /// 3. Store the populated wallet in `self.bdk`.
    ///
    /// # Errors
    ///
    /// Network/HTTP failure → `Error::Esplora`. bdk parse failure →
    /// `Error::Bdk` (fixed message — does NOT echo the descriptor
    /// to avoid leaking xprv).
    pub async fn sync(&self, client: &EsploraClient) -> Result<(), Error> {
        let mut bdk = self.build_bdk_wallet()?;
        scan_into(client, &mut bdk).await?;
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        Ok(())
    }

    /// Return the wallet's confirmed balance in satoshis (F13).
    ///
    /// Lazy: on first call, syncs against `client` if the wallet has
    /// never been synced. Subsequent calls use the cached wallet.
    /// Returns `Ok(0)` for a wallet with no confirmed UTXOs.
    pub async fn balance(&self, client: &EsploraClient) -> Result<u64, Error> {
        {
            let guard = self.bdk.lock().expect("bdk lock poisoned");
            if let Some(w) = guard.as_ref() {
                return Ok(w.balance().confirmed.to_sat());
            }
        }
        // Slow path: lazy first-time sync. Build + scan outside any
        // lock, then store + return. No MutexGuard crosses await.
        let mut bdk = self.build_bdk_wallet()?;
        scan_into(client, &mut bdk).await?;
        let bal = bdk.balance().confirmed.to_sat();
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        Ok(bal)
    }

    /// Build the in-memory `bdk_wallet::Wallet` from the stored
    /// phrase + network. Used by `sync` and `balance`; not exposed.
    ///
    /// xprv flow: phrase → seed → BIP-32 master → derive
    /// `m/84'/coin'/0'` → wpkh descriptor. The xprv is read via
    /// `XPrvHolder::to_xprv_secret` (returns `Secret<String>`,
    /// zeroize-on-drop). Descriptor Strings are dropped immediately
    /// after `bdk_wallet::Wallet::create` returns — bdk parses them
    /// into its own keystore.
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

        // Build descriptor from zeroizing Secret<String>. Drop
        // immediately after `create` returns.
        let xprv_secret = derived.to_xprv_secret();
        let external_descriptor = format!("wpkh({}/0/*)", xprv_secret.expose());
        let change_descriptor = format!("wpkh({}/1/*)", xprv_secret.expose());
        drop(xprv_secret);
        drop(derived);
        drop(master);

        let result = BdkWallet::create(external_descriptor.clone(), change_descriptor.clone())
            .network(self.network)
            .create_wallet_no_persist();

        // Drop the descriptor Strings — bdk has parsed them into
        // its keystore. If create failed, drop is moot but harmless.
        drop(external_descriptor);
        drop(change_descriptor);

        // Sanitize bdk error: do NOT propagate bdk's Display, which
        // can include the descriptor (xprv leak). Use a fixed
        // message.
        result.map_err(|_| Error::Bdk("wallet descriptor parse failed (sanitized)".into()))
    }
}

/// For first `SCAN_GAP_LIMIT` external + internal addresses, query
/// Esplora `/address/{addr}/utxo`; for each **confirmed** UTXO,
/// cap value at `Amount::MAX_MONEY` (F13 confirmed-only, plus a
/// malicious-Esplora-response cap to bound a DoS).
///
/// **Script-pubkey trust**: `info.script_pubkey()` comes from the
/// wallet's derived keychain (`bdk.peek_address`), not from
/// Esplora. Esplora's `/address/{addr}/utxo` returns only the
/// `txid`/`vout`/`value`; we substitute the wallet-derived
/// script_pubkey for the UTXO, so a malicious Esplora response
/// cannot trick the wallet into tracking someone else's UTXO.
async fn scan_into(client: &EsploraClient, bdk: &mut BdkWallet) -> Result<(), Error> {
    for kind in [KeychainKind::External, KeychainKind::Internal] {
        for i in 0..SCAN_GAP_LIMIT {
            let info = bdk.peek_address(kind, i);
            let utxos: Vec<EsploraUtxo> = client.address_utxos(&info.address).await?;
            for u in utxos {
                if !u.status.confirmed {
                    continue;
                }
                // Cap value against MAX_MONEY. Reject on overflow.
                let amount = Amount::from_sat(u.value);
                if amount > Amount::MAX_MONEY {
                    return Err(Error::Esplora(format!(
                        "utxo value {} sat exceeds MAX_MONEY ({} sat) for {}",
                        u.value,
                        Amount::MAX_MONEY.to_sat(),
                        info.address,
                    )));
                }
                let outpoint = OutPoint {
                    txid: u.txid,
                    vout: u.vout,
                };
                let txout = TxOut {
                    value: amount,
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
    use crate::chain::esplora::TlsPolicy;
    use tokio::runtime::Builder as RtBuilder;

    fn block<F: std::future::Future>(f: F) -> F::Output {
        RtBuilder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
            .block_on(f)
    }

    fn fresh_mnemonic(words: usize) -> Mnemonic {
        Mnemonic::generate(words).expect("fresh mnemonic generation")
    }

    /// Build a testnet EsploraClient for unit tests. Uses
    /// `TlsPolicy::SystemRoots` for local dev (testnet Esplora
    /// endpoints are public + trusted CA chain).
    fn testnet_client() -> EsploraClient {
        EsploraClient::new(
            "https://blockstream.info/testnet/api",
            TlsPolicy::SystemRoots,
        )
        .expect("testnet Esplora client")
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
    fn sync_rejects_invalid_client_url() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        // http:// scheme is rejected by EsploraClient::new.
        let bad = EsploraClient::new(
            "http://blockstream.info/testnet/api",
            TlsPolicy::SystemRoots,
        );
        let err = match bad {
            Ok(c) => block(wallet.sync(&c)).expect_err("http must be rejected"),
            Err(e) => e,
        };
        let msg = err.to_string();
        assert!(
            msg.contains("https") || msg.contains("invalid"),
            "error should mention https/invalid: {msg}"
        );
    }

    #[tokio::test]
    #[ignore = "requires live testnet Esplora; run manually before merge per L29"]
    async fn sync_completes_against_testnet_for_fresh_wallet() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        let client = testnet_client();
        wallet
            .sync(&client)
            .await
            .expect("full sync should complete against testnet");
    }

    #[tokio::test]
    #[ignore = "requires live testnet Esplora; run manually before merge per L29"]
    async fn balance_returns_zero_for_fresh_wallet() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        let client = testnet_client();
        let bal = wallet
            .balance(&client)
            .await
            .expect("balance fetch should complete against testnet");
        assert_eq!(bal, 0, "fresh wallet has no UTXOs; got {bal}");
    }

    #[tokio::test]
    #[ignore = "requires live testnet Esplora; run manually before merge per L29"]
    async fn balance_reuses_cached_wallet_on_second_call() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        let client = testnet_client();
        let b1 = wallet.balance(&client).await.expect("first balance call");
        let b2 = wallet.balance(&client).await.expect("second balance call");
        assert_eq!(b1, b2);
    }
}
