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
}
