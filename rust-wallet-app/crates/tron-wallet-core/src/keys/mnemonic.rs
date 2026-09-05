//! BIP-39 mnemonic wrapper (plan Task 1.1).

use anychain_kms::bip39::{Language, Mnemonic as KmsMnemonic, MnemonicType, Seed};
use core::fmt;
use zeroize::Zeroizing;

use crate::error::{Error, Result};

/// Length in bytes of a BIP-39 seed.
pub const SEED_LEN: usize = 64;

/// A validated BIP-39 mnemonic.
///
/// Wraps `anychain_kms::bip39::Mnemonic`, which already keeps the phrase and
/// entropy in `Zeroizing`. The wrapper exists to keep the upstream type out of
/// our public signatures and to redact the phrase from `Debug` output.
#[derive(Clone)]
pub struct Mnemonic(KmsMnemonic);

impl Mnemonic {
    /// Generates a fresh mnemonic from system randomness.
    ///
    /// Infallible: `word_count` is an enum, so there is no invalid length to
    /// reject. (The plan sketched `new(words: u8) -> Result<Self>`; taking the
    /// enum removes the failure case rather than hiding it behind a `Result`
    /// that never returns `Err`.)
    pub fn generate(word_count: MnemonicType, language: Language) -> Self {
        Self(KmsMnemonic::new(word_count, language))
    }

    /// Parses an existing phrase, checking the word list and BIP-39 checksum.
    pub fn from_phrase(phrase: &str, language: Language) -> Result<Self> {
        KmsMnemonic::from_phrase(phrase, language)
            .map(Self)
            .map_err(|e| Error::Mnemonic(e.to_string()))
    }

    /// The phrase itself.
    ///
    /// This is the wallet's root secret in plaintext. Callers are responsible
    /// for not logging or persisting it unencrypted; the crate exposes it only
    /// because backup and re-import flows need it.
    pub fn phrase(&self) -> &str {
        self.0.phrase()
    }

    /// The language this phrase was validated against.
    pub fn language(&self) -> Language {
        self.0.language()
    }

    /// Stretches the phrase into a BIP-39 seed with PBKDF2-HMAC-SHA512.
    ///
    /// `passphrase` is the BIP-39 "25th word". An empty string is the common
    /// case and is what most wallets import.
    pub fn to_seed(&self, passphrase: &str) -> Zeroizing<[u8; SEED_LEN]> {
        // `Seed` zeroes itself on drop; copy out and let it go immediately.
        let seed = Seed::new(&self.0, passphrase);

        let mut bytes = Zeroizing::new([0u8; SEED_LEN]);
        // PBKDF2 output is fixed at 64 bytes upstream. If that ever changes,
        // `copy_from_slice` panics on the length mismatch rather than
        // truncating, which is the behaviour we want — a short seed would
        // silently derive the wrong wallet.
        bytes.copy_from_slice(seed.as_bytes());

        bytes
    }
}

/// Redacts the phrase. A wallet's root secret must not reach a log line
/// through a stray `{:?}`.
///
/// The word count is redacted too. It is weak information, but it is also of
/// no use in a debug line: knowing a value is a mnemonic is the part that
/// matters, and the count only narrows an attacker's search.
impl fmt::Debug for Mnemonic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mnemonic")
            .field("phrase", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANONICAL: &str = "abandon abandon abandon abandon abandon abandon \
         abandon abandon abandon abandon abandon about";

    #[test]
    fn accepts_the_canonical_phrase() {
        let mnemonic = Mnemonic::from_phrase(CANONICAL, Language::English).expect("valid phrase");
        assert_eq!(mnemonic.phrase(), CANONICAL);
    }

    #[test]
    fn rejects_a_broken_checksum() {
        // Valid words, wrong checksum: twelve "abandon"s do not checksum.
        let phrase = "abandon abandon abandon abandon abandon abandon \
             abandon abandon abandon abandon abandon abandon";

        assert!(matches!(
            Mnemonic::from_phrase(phrase, Language::English),
            Err(Error::Mnemonic(_))
        ));
    }

    #[test]
    fn rejects_a_word_outside_the_list() {
        let phrase = "zzzz abandon abandon abandon abandon abandon \
             abandon abandon abandon abandon abandon about";

        assert!(Mnemonic::from_phrase(phrase, Language::English).is_err());
    }

    #[test]
    fn generates_the_requested_word_count() {
        for (word_count, expected) in [
            (MnemonicType::Words12, 12),
            (MnemonicType::Words15, 15),
            (MnemonicType::Words24, 24),
        ] {
            let mnemonic = Mnemonic::generate(word_count, Language::English);
            assert_eq!(mnemonic.phrase().split_whitespace().count(), expected);
        }
    }

    #[test]
    fn generated_phrases_differ() {
        let a = Mnemonic::generate(MnemonicType::Words12, Language::English);
        let b = Mnemonic::generate(MnemonicType::Words12, Language::English);

        assert_ne!(a.phrase(), b.phrase(), "entropy source must not be fixed");
    }

    #[test]
    fn seed_is_64_bytes_and_passphrase_sensitive() {
        let mnemonic = Mnemonic::from_phrase(CANONICAL, Language::English).expect("valid phrase");

        let plain = mnemonic.to_seed("");
        let salted = mnemonic.to_seed("TREZOR");

        assert_eq!(plain.len(), SEED_LEN);
        assert_ne!(*plain, *salted);
    }

    /// BIP-39 reference vector: all-zero entropy, passphrase "TREZOR".
    /// Source: <https://github.com/trezor/python-mnemonic/blob/master/vectors.json>
    #[test]
    fn seed_matches_the_bip39_reference_vector() {
        let mnemonic = Mnemonic::from_phrase(CANONICAL, Language::English).expect("valid phrase");

        assert_eq!(
            hex::encode(*mnemonic.to_seed("TREZOR")),
            "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e5349553\
             1f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04"
        );
    }

    #[test]
    fn debug_does_not_leak_the_phrase() {
        let mnemonic = Mnemonic::from_phrase(CANONICAL, Language::English).expect("valid phrase");
        let rendered = format!("{mnemonic:?}");

        assert!(!rendered.contains("abandon"), "leaked: {rendered}");
        assert!(rendered.contains("redacted"));
    }
}
