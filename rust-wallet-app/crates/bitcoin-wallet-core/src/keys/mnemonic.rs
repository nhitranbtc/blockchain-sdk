//! BIP-39 mnemonic (English wordlist).
//!
//! Wraps `bip39::Mnemonic` in [`Secret`] for zeroize-on-drop (per F47/F53,
//! mitigates threat-model U3 — `/proc/$pid/mem` exposure). Caller never
//! touches raw entropy or seed bytes via the public API.
//!
//! **Drift from plan §Task 3** (L9 v3 PR body — see PR for the full table):
//!
//! | Plan said | This implementation | Why |
//! |---|---|---|
//! | Exposes raw `bdk_wallet::keys::bip39::Mnemonic` | Newtype `Mnemonic(Secret<bip39::Mnemonic>)` | CONTEXT.md vocab: "mnemonic wrapped in `Secret<Mnemonic>`" |
//! | `to_seed → [u8; 64]` | `to_seed → Secret<Box<[u8; 64]>>` | `[u8; 64]` is `Copy` — `Secret<Copy T>` provides ~zero protection (caller dups with `*seed.expose()`). `Box` forces heap allocation. |
//! | `phrase() → String` (plaintext) | `to_phrase() → Secret<String>` | F47: plaintext mnemonic on heap survives `Drop`; U3/A1/A8 escape. |
//! | `parse` (auto-detects language) | `parse_in(Language::English, …)` | Plan promises English-only; `parse` would silently accept Japanese etc. if a future dep enables another wordlist. |
//! | Tests use `"abandon × 11 about"` | Tests use `generate().phrase()` round-trip | CONTEXT.md hard rule #5 — never reuse a published BIP-39 test vector |
//! | New Cargo.toml dep on `bitcoin-wallet-core` only | `bip39` declared in workspace `[workspace.dependencies]` (per crate convention) | Workspace dep centralization |
//!
//! **Defends against:** A1 (local read of memory dump), A8 (co-resident
//! process reads `/proc/$pid/mem`), U3 (memory leak), U6 (world-readable
//! file — mnemonic never persisted in plaintext at this layer).
//!
//! **Word count → entropy bits** (BIP-39 spec, enforced in
//! `is_valid_word_count`):
//!
//! | Words | Entropy bits | Checksum bits | Total bits |
//! |---|---|---|---|
//! | 12 | 128 | 4 | 132 |
//! | 15 | 160 | 5 | 165 |
//! | 18 | 192 | 6 | 198 |
//! | 21 | 224 | 7 | 231 |
//! | 24 | 256 | 8 | 264 |

use std::str::FromStr;

use zeroize::ZeroizeOnDrop;

use crate::error::{Error, Result};
use crate::keys::secret::Secret;

/// BIP-39 mnemonic phrase. Inner `Secret<>` zeroes memory on drop.
///
/// Construct via [`Mnemonic::generate`] (fresh entropy) or
/// [`Mnemonic::from_phrase`] (parse + checksum-validate English).
/// Derive seeds via [`Mnemonic::to_seed`]. Reconstruct phrases via
/// [`Mnemonic::to_phrase`].
#[derive(ZeroizeOnDrop)]
pub struct Mnemonic(Secret<bip39::Mnemonic>);

// `#[derive(ZeroizeOnDrop)]` on `Mnemonic` is a **marker trait assertion**,
// not the zeroizing mechanism. The actual zeroize happens in `Secret`'s own
// `Drop` impl (because `Secret<T: Zeroize>` derives `ZeroizeOnDrop` and
// `Secret::drop` calls `T::zeroize()` on the inner). The outer derive here
// only makes `Mnemonic: ZeroizeOnDrop` true at the type level — the
// `assert_zeroize_on_drop` compile-time witness test in the test module
// depends on it. Do NOT remove without also removing the test.

// Manual `Debug` impl: hides the secret inner in `{:?}` output (threat-model
// U3 mitigation — mnemonic must never appear in logs, panic messages, or
// error chains). `Secret<T>` doesn't derive `Debug` for the same reason.
//
// Uses `finish_non_exhaustive()` (renders as `Mnemonic { .. }`) instead of
// naming any field — field names like `"inner"` can collide with BIP-39
// wordlist words (e.g. "inner" is word 933 of the English wordlist), which
// makes a "no phrase word appears in Debug" assertion flaky.
// NOTE: `bip39::Mnemonic` itself impls `Display` and prints the full phrase
// (bip39 2.2.2 src/lib.rs:646). We deliberately do NOT impl `Display` on our
// wrapper — `format!("{}", m)` and `println!("{}", m)` would leak the
// phrase. This omission is load-bearing; do not "fix" it.
impl std::fmt::Debug for Mnemonic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mnemonic").finish_non_exhaustive()
    }
}

impl FromStr for Mnemonic {
    type Err = Error;

    /// Equivalent to [`Mnemonic::from_phrase`]. Provided so that
    /// `s.parse::<Mnemonic>()` works per Rust API guideline C-CONV-FROMSTR.
    fn from_str(s: &str) -> Result<Self> {
        Self::from_phrase(s)
    }
}

impl Mnemonic {
    /// Generate a fresh English mnemonic with `words` words.
    ///
    /// `words` must be one of 12, 15, 18, 21, 24 (BIP-39 standard word
    /// counts). Randomness comes from `bip39`'s `thread_rng()` wrapper
    /// (rand 0.8 → getrandom → OS RNG).
    ///
    /// # Errors
    /// - [`Error::InvalidMnemonic`] if `words` is not a supported word count.
    pub fn generate(words: usize) -> Result<Self> {
        if !is_valid_word_count(words) {
            return Err(Error::InvalidMnemonic(format!(
                "unsupported word count: {words}; expected one of 12, 15, 18, 21, 24"
            )));
        }
        let inner = bip39::Mnemonic::generate(words).map_err(map_bip39_error)?;
        Ok(Self(Secret::new(inner)))
    }

    /// Parse a whitespace-separated English mnemonic phrase.
    ///
    /// Validates that the word count is in {12, 15, 18, 21, 24} and that
    /// the BIP-39 checksum is correct. Bip39's `parse_in` (vs `parse`) is
    /// used here so a non-English wordlist feature accidentally enabled
    /// upstream does not silently accept non-English phrases.
    ///
    /// # Errors
    /// - [`Error::InvalidMnemonic`] if the phrase fails checksum validation
    ///   or has an unsupported word count.
    pub fn from_phrase(s: &str) -> Result<Self> {
        let inner =
            bip39::Mnemonic::parse_in(bip39::Language::English, s).map_err(map_bip39_error)?;
        Ok(Self(Secret::new(inner)))
    }

    /// Derive the BIP-39 seed (64 bytes) via PBKDF2-HMAC-SHA512
    /// (2048 rounds per BIP-39 spec).
    ///
    /// Returns `Secret<Vec<u8>>` — heap-allocated, non-`Copy`. `Vec<u8>`
    /// impls `Zeroize`, so the secret zeroizes when the `Secret` is
    /// dropped. Returning a `Vec` (vs `[u8; 64]`) prevents the caller
    /// from trivially cloning out via `*seed.expose()` (a `Copy` array
    /// would defeat zeroize-on-drop because each read makes a fresh
    /// stack copy).
    ///
    /// `passphrase` is the BIP-39 optional passphrase (not the wallet
    /// encryption passphrase — different concept). The caller's `&str` is
    /// not zeroized; v0.1 accepts this trade-off and documents it.
    pub fn to_seed(&self, passphrase: &str) -> Secret<Vec<u8>> {
        let seed = self.0.expose().to_seed(passphrase);
        let vec: Vec<u8> = seed.to_vec();
        Secret::new(vec)
    }

    /// Word count of the phrase (12, 15, 18, 21, or 24).
    pub fn word_count(&self) -> usize {
        self.0.expose().word_count()
    }

    /// Reconstruct the phrase as a single space-separated string.
    ///
    /// Returns `Secret<String>` so the phrase lives in a zeroizable wrapper
    /// rather than a bare heap `String` (F47/F53, threat-model U3/A1/A8).
    /// Callers must not extract the inner `String` and stash it elsewhere.
    pub fn to_phrase(&self) -> Secret<String> {
        let s: String = self.0.expose().words().collect::<Vec<_>>().join(" ");
        Secret::new(s)
    }
}

/// Map a `bip39::Error` to our fixed-string [`Error::InvalidMnemonic`].
///
/// Defensive against future bip39 minor versions that might enrich
/// `Display` to echo the offending word back to the caller. We never
/// store the upstream `Display` verbatim.
///
/// Note: bip39 2.2.2's `Error::UnknownWord(usize)` echoes the **index** of
/// the bad word (not the word itself), so today it is safe. We still
/// suppress it for forward-compatibility.
///
/// Match is intentionally exhaustive (no catch-all). If a future bip39
/// minor adds a variant, the compile error forces a code review here.
fn map_bip39_error(e: bip39::Error) -> Error {
    let msg: &'static str = match e {
        bip39::Error::BadEntropyBitCount(_) => "invalid entropy length",
        bip39::Error::BadWordCount(_) => "unsupported word count",
        bip39::Error::InvalidChecksum => "checksum mismatch",
        bip39::Error::UnknownWord(_) => "unknown word",
        bip39::Error::AmbiguousLanguages(_) => "ambiguous language detection",
    };
    Error::InvalidMnemonic(msg.into())
}

/// Return true iff `words` is a BIP-39 standard word count.
fn is_valid_word_count(words: usize) -> bool {
    matches!(words, 12 | 15 | 18 | 21 | 24)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_each_word_count_succeeds() {
        for &n in &[12usize, 15, 18, 21, 24] {
            let m = Mnemonic::generate(n).expect("generate");
            assert_eq!(m.word_count(), n, "word count mismatch for {n}");
            // Phrase round-trip: word count preserved
            let phrase = m.to_phrase();
            assert_eq!(phrase.expose().split_whitespace().count(), n);
        }
    }

    #[test]
    fn generate_unsupported_word_count_returns_error() {
        for &bad in &[0usize, 1, 11, 13, 16, 23, 25, 100] {
            let err = Mnemonic::generate(bad).expect_err("should fail");
            assert!(matches!(err, Error::InvalidMnemonic(_)), "got {err:?}");
            assert!(
                err.to_string().contains(&format!("{bad}")),
                "error should mention word count {bad}: {err}"
            );
        }
    }

    #[test]
    fn from_phrase_rejects_unsupported_word_count() {
        // 14 words = not a valid BIP-39 length (valid: 12, 15, 18, 21, 24)
        let bad = "abandon abandon abandon abandon abandon abandon abandon \
                   abandon abandon abandon abandon abandon abandon abandon";
        assert_eq!(bad.split_whitespace().count(), 14);
        let err = Mnemonic::from_phrase(bad).expect_err("should fail");
        assert!(matches!(err, Error::InvalidMnemonic(_)));
    }

    #[test]
    fn from_phrase_rejects_invalid_checksum() {
        // Deterministic mutation: swap two internal words of a 24-word
        // phrase. Each swap flips 11 entropy bits in two positions, so the
        // checksum fails with overwhelming probability (1 - 2^-22 ≈ 1.0).
        // Random-replacement at the last word is ~6% valid-checksum flaky.
        let m = Mnemonic::generate(24).expect("generate");
        let phrase = m.to_phrase();
        let mut words: Vec<&str> = phrase.expose().split_whitespace().collect();
        words.swap(5, 12);
        let mutated = words.join(" ");
        let err = Mnemonic::from_phrase(&mutated).expect_err("checksum should fail");
        assert!(matches!(err, Error::InvalidMnemonic(_)));
    }

    #[test]
    fn from_phrase_rejects_unknown_word() {
        let m = Mnemonic::generate(12).expect("generate");
        let phrase = m.to_phrase();
        let mut words: Vec<&str> = phrase.expose().split_whitespace().collect();
        words[5] = "notaword";
        let bad = words.join(" ");
        let err = Mnemonic::from_phrase(&bad).expect_err("should fail");
        assert!(matches!(err, Error::InvalidMnemonic(_)));
        // Defensive: error message must NOT echo "notaword" back.
        assert!(
            !err.to_string().contains("notaword"),
            "error leaks the bad word: {err}"
        );
    }

    #[test]
    fn from_phrase_round_trip_after_generate() {
        let m1 = Mnemonic::generate(24).expect("generate");
        let phrase = m1.to_phrase();
        let m2 = Mnemonic::from_phrase(phrase.expose()).expect("parse");
        // Same entropy → same phrase after re-parse
        assert_eq!(m1.to_phrase().expose(), m2.to_phrase().expose());
        assert_eq!(m1.word_count(), m2.word_count());
    }

    #[test]
    fn from_str_delegates_to_from_phrase() {
        let m1 = Mnemonic::generate(12).expect("generate");
        let phrase = m1.to_phrase();
        let m2: Mnemonic = phrase.expose().parse().expect("FromStr");
        assert_eq!(m1.word_count(), m2.word_count());
    }

    #[test]
    fn from_phrase_rejects_empty() {
        let err = Mnemonic::from_phrase("").expect_err("empty should fail");
        assert!(matches!(err, Error::InvalidMnemonic(_)));
    }

    #[test]
    fn from_phrase_rejects_whitespace_only() {
        let err = Mnemonic::from_phrase("   \t  ").expect_err("ws should fail");
        assert!(matches!(err, Error::InvalidMnemonic(_)));
    }

    #[test]
    fn from_phrase_rejects_non_ascii() {
        // Non-ASCII word is not in the BIP-39 English wordlist (2048 words).
        let err = Mnemonic::from_phrase(
            "legal winner thank year novel sentence deal \
                                  margin seven school submit légal winner thank year",
        )
        .expect_err("non-ASCII should fail");
        assert!(matches!(err, Error::InvalidMnemonic(_)));
    }

    #[test]
    fn from_phrase_normalizes_multiple_spaces() {
        // BIP-39 spec says phrases may have multiple internal spaces;
        // bip39's parse_in normalizes whitespace. Build a 12-word phrase,
        // then join with double spaces; should still parse.
        let m1 = Mnemonic::generate(12).expect("generate");
        let phrase = m1.to_phrase();
        let with_extra_ws = phrase
            .expose()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("  ");
        let m2 = Mnemonic::from_phrase(&with_extra_ws).expect("double-space ok");
        assert_eq!(m1.to_phrase().expose(), m2.to_phrase().expose());
    }

    #[test]
    fn to_seed_is_64_bytes_and_zeroizes_on_drop() {
        let m = Mnemonic::generate(12).expect("generate");
        let seed = m.to_seed("");
        assert_eq!(seed.expose().len(), 64);
        // Compile-time witness: Secret<Vec<u8>> impls ZeroizeOnDrop.
        fn assert_zeroize_on_drop<T: zeroize::ZeroizeOnDrop>() {}
        assert_zeroize_on_drop::<Secret<Vec<u8>>>();
    }

    #[test]
    fn to_seed_is_deterministic_for_same_mnemonic_and_passphrase() {
        let m1 = Mnemonic::generate(12).expect("generate");
        let phrase = m1.to_phrase();
        let m2 = Mnemonic::from_phrase(phrase.expose()).expect("parse");
        let seed1 = m1.to_seed("");
        let seed2 = m2.to_seed("");
        assert_eq!(
            seed1.expose(),
            seed2.expose(),
            "BIP-39 seed must be deterministic"
        );
    }

    #[test]
    fn to_seed_changes_with_passphrase() {
        let m = Mnemonic::generate(12).expect("generate");
        let seed_empty = m.to_seed("");
        let seed_pass = m.to_seed("TREZOR");
        assert_ne!(
            seed_empty.expose(),
            seed_pass.expose(),
            "passphrase must affect seed"
        );
    }

    #[test]
    fn two_generates_produce_different_mnemonics() {
        let m1 = Mnemonic::generate(24).expect("g1");
        let m2 = Mnemonic::generate(24).expect("g2");
        assert_ne!(
            m1.to_phrase().expose(),
            m2.to_phrase().expose(),
            "fresh entropy must differ"
        );
    }

    #[test]
    fn mnemonic_zeroizes_on_drop() {
        // Compile-time witness: our newtype impls ZeroizeOnDrop via Secret.
        fn assert_zeroize_on_drop<T: zeroize::ZeroizeOnDrop>() {}
        assert_zeroize_on_drop::<Mnemonic>();
    }

    #[test]
    fn debug_hides_secret_inner() {
        let m = Mnemonic::generate(12).expect("generate");
        let dbg = format!("{m:?}");
        // The actual phrase words must NOT appear in the Debug output.
        // Uses `finish_non_exhaustive()` so Debug renders as
        // `Mnemonic { .. }` with no field names (field names can collide
        // with BIP-39 wordlist words).
        let phrase = m.to_phrase();
        for word in phrase.expose().split_whitespace() {
            assert!(
                !dbg.contains(word),
                "Debug output leaks word {word:?}: {dbg}"
            );
        }
        // Sanity: Debug output mentions Mnemonic by name.
        assert!(
            dbg.contains("Mnemonic"),
            "Debug should name Mnemonic: {dbg}"
        );
    }

    #[test]
    fn bip39_error_messages_are_fixed_strings() {
        // Checks that our map_bip39_error never echoes the offending input.
        let err = Mnemonic::from_phrase("notaword ".to_string().repeat(12).trim())
            .expect_err("should fail");
        let msg = err.to_string();
        assert!(msg.contains("unknown word"), "got: {msg}");
        assert!(!msg.contains("notaword"), "leaks bad word: {msg}");
    }

    #[test]
    fn word_count_for_each_standard_size() {
        for &n in &[12usize, 15, 18, 21, 24] {
            let m = Mnemonic::generate(n).expect("generate");
            assert_eq!(m.word_count(), n);
        }
    }

    #[test]
    fn word_count_survives_from_phrase() {
        // Exercise 15/18/21 from from_phrase path (12 and 24 covered in
        // round_trip test).
        for &n in &[15usize, 18, 21] {
            let m1 = Mnemonic::generate(n).expect("generate");
            let phrase = m1.to_phrase();
            let m2 = Mnemonic::from_phrase(phrase.expose()).expect("parse");
            assert_eq!(m2.word_count(), n);
        }
    }
}
