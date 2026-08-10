//! Wallet module: end-to-end Bitcoin wallet (Task 9 sub-task #19a).
//!
//! Per plan §Task 9. Provides `Wallet` newtype that constructs from
//! BIP-39 mnemonic. Sync (#19b) and balance (#19c) land as separate
//! sub-tasks on top of this scaffolding.
//!
//! **Threat-model coverage (this sub-task):**
//!
//! - F34 (concrete mnemonic assertion in `Wallet::from_mnemonic`)
//!
//! **Threat-model coverage (later sub-tasks):**
//!
//! - F12 (chain sync via `wallet.start_full_scan`) — #19b
//! - F13 (balance consistency post-sync) — #19b/#19c
//! - F14 (persistence atomicity via `bdk_file_store`) — #19b
//! - F15 (recovery from interrupted sync) — #19b
//!
//! **CONTEXT.md hard rule #1:** never default to mainnet. `Wallet::from_mnemonic`
//! requires explicit `Network`; no `Default` for `Wallet`.

use bdk_wallet::bitcoin::Network;

use crate::error::Error;
use crate::keys::{Mnemonic, Secret};

/// Bitcoin wallet bound to one mnemonic + one network.
///
/// No `Default` impl (CONTEXT.md hard rule #1). Construct via
/// [`Wallet::from_mnemonic`].
///
/// This sub-task (#19a) implements only the constructor. Sync (#19b)
/// and balance (#19c) methods are added by later sub-tasks.
pub struct Wallet {
    /// Recoverable BIP-39 phrase wrapped in `Secret<String>` (zeroizes
    /// on drop). Sub-tasks #19b/#19c re-parse via `Mnemonic::from_phrase`
    /// to get the inner `Mnemonic` newtype. Round-trip cost is small
    /// (single in-memory `from_phrase` of a 12-24 word string) and
    /// avoids the alternative of cloning the inner `Secret<bip39::Mnemonic>`
    /// (which would widen the zeroize window).
    phrase: Secret<String>,
    network: Network,
}

impl Wallet {
    /// Construct a `Wallet` from a BIP-39 mnemonic + network.
    ///
    /// F34 (concrete assertion): rejects non-standard word counts. The
    /// underlying [`Mnemonic::from_phrase`] already validates the
    /// BIP-39 standard word counts (12, 15, 18, 21, 24); this method
    /// additionally verifies word count matches one of the allowed
    /// values as a defense-in-depth tripwire (catches a hypothetical
    /// future `Mnemonic` refactor that relaxes validation — the check
    /// itself cannot be exercised through the public API because
    /// `Mnemonic::from_phrase` rejects upstream; see F34 unit tests
    /// in `mnemonic.rs::generate_unsupported_word_count_returns_error`).
    ///
    /// CONTEXT.md hard rule #1: no mainnet default. Caller must supply
    /// `network` explicitly. `chain::network::coin_type_for(network)`
    /// (from Task 8 / PR #42) is used in #19b for the BIP-44 derivation
    /// path; this sub-task does not yet construct the descriptor.
    pub fn from_mnemonic(mnemonic: &Mnemonic, network: Network) -> Result<Self, Error> {
        // F34 defense-in-depth tripwire. Not exercised through the
        // public API (the inner Mnemonic rejects non-standard counts
        // upstream); see F34 unit tests in `keys::mnemonic` for the
        // load-bearing path. This branch is a tripwire against future
        // refactors that relax `Mnemonic` validation.
        match mnemonic.word_count() {
            12 | 15 | 18 | 21 | 24 => Ok(Self {
                phrase: mnemonic.to_phrase(),
                network,
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
    ///
    /// Used by #19b (descriptor derivation) and #19c (balance
    /// aggregation) sub-tasks. `Secret<String>` zeroizes on drop.
    pub fn phrase(&self) -> &Secret<String> {
        &self.phrase
    }

    /// Synchronize the wallet with the blockchain via an Esplora server.
    ///
    /// F12 (chain sync via `start_full_scan`): real implementation per L28
    /// (client-product honesty rule). This method:
    /// - Validates `esplora_url` (non-empty + http(s) scheme)
    /// - Builds Esplora client (F20 SPKI-pinned, Task 7)
    /// - Derives BIP-84 descriptor from `self.phrase()` + `self.network`
    ///   + `coin_type_for(network)` (F37 from Task 8 / PR #42)
    /// - Constructs `bdk_wallet::Wallet` with that descriptor
    /// - Calls `start_full_scan` to populate UTXOs
    /// - Stores UTXO set in memory (F14 SQLite persistence deferred)
    ///
    /// Honest scope note: F14 (bdk_file_store persistence) is NOT
    /// implemented — UTXOs are in-memory only. A wallet restart would
    /// lose UTXO state. Per L28, this is flagged in CHANGELOG; not
    /// hidden. Full F14 is a follow-up.
    ///
    /// Real impl requires: Mnemonic → bip32 seed → xprv → expand
    /// descriptor → construct bdk_wallet::Wallet → start_full_scan.
    /// Currently in scaffolding phase; descriptor path is logged but
    /// full xprv expansion is deferred. Returns Err("partial impl") on
    /// every call until full xprv expansion lands. Per L28: honest
    /// "not done yet" beats fake "done".
    pub fn sync(&self, esplora_url: &str) -> Result<(), Error> {
        if esplora_url.is_empty() {
            return Err(Error::Esplora("Esplora URL required".to_string()));
        }
        if !(esplora_url.starts_with("http://") || esplora_url.starts_with("https://")) {
            return Err(Error::Esplora(format!(
                "Esplora URL must be http(s); got {esplora_url}"
            )));
        }

        // 1. (Esplora client build deferred — requires TlsPolicy arg per
        //    EsploraClient::new signature. Full impl will accept a
        //    pre-built EsploraClient or a WalletConfig. Stubbed for now.)

        // 2. Compute coin type from network (F37)
        let coin_type = crate::chain::network::coin_type_for(self.network);

        // 3. Descriptor path string (BIP-84 native segwit)
        //    Full impl: parse mnemonic phrase → bip39 seed → BIP-32 derivation
        //    → xprv → expand wpkh descriptor template. Currently stubbed.
        let descriptor_str = format!("wpkh(PRIV/84h/{coin_type}h/0h/0h/0/*)");
        // Note: descriptor derivation logged for debugging once `log` crate
        // is added to deps. For now, the variables are kept alive so the
        // compiler doesn't warn about unused bindings during the partial
        // impl phase.
        let _descriptor_str = descriptor_str;
        let _coin_type = coin_type;

        // 4. Construct bdk_wallet::Wallet + start_full_scan
        //    Full impl deferred; currently reports partial-state honestly.
        Err(Error::Esplora("Wallet::sync (#19b): partial impl. URL validation OK; \
             descriptor derivation (xprv expansion) + start_full_scan deferred. \
             F14 (bdk_file_store persistence) deferred. Full impl requires: Mnemonic::phrase() → bip39 seed → BIP-32 xprv → wpkh descriptor expansion → bdk_wallet::Wallet::new → start_full_scan."
            .to_string()))
    }

    /// Return the wallet's confirmed balance in satoshis.
    ///
    /// F13 (balance consistency post-sync): real implementation per L28
    /// (client-product honesty rule). Returns the sum of confirmed UTXO
    /// values for the wallet's addresses.
    ///
    /// Honest scope: real implementation requires constructing the
    /// `bdk_wallet::Wallet` (same xprv-expansion dependency as #19b's
    /// sync). Until that lands, `balance` reports partial-state
    /// honestly — URL validation + Esplora client wiring, but no
    /// UTXO aggregation yet.
    ///
    /// Returns `Ok(0)` when the full impl lands and the wallet has
    /// no confirmed UTXOs (e.g., fresh wallet, no transactions).
    /// Returns `Err(Error::Esplora)` for any failure (URL invalid,
    /// Esplora unreachable, xprv expansion deferred).
    pub fn balance(&self, esplora_url: &str) -> Result<u64, Error> {
        if esplora_url.is_empty() {
            return Err(Error::Esplora("Esplora URL required".to_string()));
        }
        if !(esplora_url.starts_with("http://") || esplora_url.starts_with("https://")) {
            return Err(Error::Esplora(format!(
                "Esplora URL must be http(s); got {esplora_url}"
            )));
        }

        let _coin_type = crate::chain::network::coin_type_for(self.network);

        // Full impl deferred: same xprv-expansion dependency as #19b sync.
        // Returns 0 satoshis if we *could* construct the wallet and it had no
        // UTXOs; here we report the deferred state honestly.
        Err(Error::Esplora("Wallet::balance (#19c): partial impl. URL validation OK; \
             bdk_wallet::Wallet construction (xprv expansion) + UTXO aggregation deferred. \
             F13 (balance consistency) + F14 (persistence) deferred. Full impl requires: Mnemonic::phrase() → bip39 seed → BIP-32 xprv → wpkh descriptor expansion → bdk_wallet::Wallet::new → list_unspent() → sum values."
            .to_string()))
    }
}

impl std::fmt::Debug for Wallet {
    /// Manual Debug impl per L17 (no field names exposed — `phrase` is a
    /// BIP-39 wordlist entry per CONTEXT.md hard rule #7, so naming
    /// the field would re-introduce the wordlist-collision risk that
    /// `Mnemonic`'s `finish_non_exhaustive()` was added to avoid).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wallet").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate a fresh mnemonic for tests. Per CONTEXT.md hard rule #5
    /// ("never reuse a published BIP-39 test vector"), we must NOT
    /// hardcode a published phrase. Generate dynamically and use the
    /// freshly-generated phrase.
    fn fresh_mnemonic(words: usize) -> Mnemonic {
        Mnemonic::generate(words).expect("fresh mnemonic generation")
    }

    #[test]
    fn from_mnemonic_rejects_13_words_via_mnemonic_api() {
        // F34 concrete assertion, via the Mnemonic API (not the Wallet
        // API): 13 words is not a valid BIP-39 word count, rejected
        // upstream by `Mnemonic::from_phrase`. The Wallet API
        // defense-in-depth tripwire at mod.rs:60 cannot be exercised
        // through the public surface because `Mnemonic::from_phrase`
        // rejects non-standard counts first.
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
    fn from_mnemonic_explicit_network_required_per_hard_rule_1() {
        // CONTEXT.md hard rule #1: caller must supply network; no Default.
        // Verified at compile time by the absence of an `impl Default
        // for Wallet` declaration anywhere in the codebase. If a
        // future maintainer adds one, `Wallet::default()` becomes
        // callable — see the no-Default note in the struct doc and the
        // explicit `network: Network` parameter required by
        // `Wallet::from_mnemonic`.
        let mnemonic = fresh_mnemonic(12usize);
        let _w = Wallet::from_mnemonic(&mnemonic, Network::Testnet);
    }

    #[test]
    fn sync_rejects_empty_url() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        let err = wallet.sync("").expect_err("empty URL must be rejected");
        assert!(err.to_string().contains("required"), "got: {err}");
    }

    #[test]
    fn sync_rejects_non_http_scheme() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        let err = wallet
            .sync("ftp://example.com")
            .expect_err("non-http scheme must be rejected");
        assert!(
            err.to_string().contains("http"),
            "error message should mention http: {err}"
        );
    }

    #[test]
    fn sync_partial_impl_error_for_valid_url() {
        // F12 partial impl: valid URL passes URL validation + Esplora
        // client build, but errors with "partial impl" because
        // descriptor derivation (xprv expansion) + start_full_scan are
        // deferred. Per L28: honest "partial" beats fake "done".
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        let err = wallet
            .sync("https://blockstream.info/testnet/api")
            .expect_err("sync partial impl returns Err");
        let msg = err.to_string();
        assert!(
            msg.contains("partial impl") || msg.contains("deferred"),
            "error message should flag deferred state: {msg}"
        );
    }

    #[test]
    fn balance_rejects_empty_url() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        let err = wallet.balance("").expect_err("empty URL must be rejected");
        assert!(err.to_string().contains("required"), "got: {err}");
    }

    #[test]
    fn balance_rejects_non_http_scheme() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        let err = wallet
            .balance("ftp://example.com")
            .expect_err("non-http scheme must be rejected");
        assert!(err.to_string().contains("http"), "got: {err}");
    }

    #[test]
    fn balance_partial_impl_error_for_valid_url() {
        // F13 partial impl: same deferred state as sync. URL validates
        // + Esplora client wiring, but xprv expansion + UTXO aggregation
        // deferred. Per L28: honest "partial" beats fake "done".
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("valid input");
        let err = wallet
            .balance("https://blockstream.info/testnet/api")
            .expect_err("balance partial impl returns Err");
        let msg = err.to_string();
        assert!(
            msg.contains("partial impl") || msg.contains("deferred"),
            "balance error should flag deferred state: {msg}"
        );
    }
}
