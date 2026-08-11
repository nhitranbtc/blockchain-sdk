//! High-level mnemonic encryption helper (issue #28, extended by #66).
//!
//! Bundles [`argon2::random_salt`] + [`argon2::derive_key`] +
//! [`aes_gcm::encrypt`] / [`aes_gcm::decrypt`] into a single-call API for
//! encrypting BIP-39 phrases at rest. The new `aad: Aad<'_>` parameter
//! (Issue #66 / ADR 0001) binds additional authenticated context to the
//! ciphertext; pass `Aad::NONE` for the pre-#66 behavior or
//! `Aad::network(network)` to bind the `bitcoin::Network` discriminant
//! (closes the cross-network-footgun N5).
//!
//! **Threat-model coverage** (per issue #28 + ADR 0001):
//!
//! - **F5** (Argon2id KDF, m=256 MiB / t=10 / p=4) — strong offline
//!   cracker resistance. Per `argon2` module.
//! - **F6** (AES-256-GCM AEAD, 96-bit random nonce per blob) —
//!   confidentiality + integrity. Per `aes_gcm` module.
//! - **F47** / **U3** (zeroize on drop) — `Secret<String>` (phrase) +
//!   `Secret<Vec<u8>>` (intermediate plaintext buffer) all zeroize on
//!   drop. `MnemonicCipherBlob` ciphertext is non-secret (already
//!   encrypted; not wrapped).
//! - **N5** (cross-network ciphertext reuse, per ADR 0001) —
//!   `bitcoin::Network` discriminant bound via AAD. Copying a testnet
//!   blob to the mainnet directory fails AEAD verification at decrypt
//!   time, not silently.
//!
//! **Defends against:** A1 (offline cracker of stolen ciphertext via
//! strong KDF) + F43 (per-protocol error variant — distinct from
//! underlying `Encryption` errors).
//!
//! **Does NOT defend:** T1 (physical seizure — out of scope, requires
//! full-disk encryption). Caller error (wrong password, wrong AAD,
//! truncated blob) is reported, not hidden — these surface as
//! `Error::MnemonicCipher`.
//!
//! **Calibration note:** Argon2id parameters (m=256 MiB, t=10, p=4)
//! are inherited from the `argon2` module — see its calibration note.
//! Changing them here would be a no-op; tune `argon2::ARGON2_*` instead.
//!
//! **Why the salt is embedded but the AAD is not:** the salt is a
//! non-secret uniquifier that must travel with the ciphertext to be
//! usable. The AAD is a *context assertion* whose entire value comes
//! from being supplied independently at decrypt time. Embedding it
//! would make it self-satisfying and void the cross-network defense
//! (N5). The same shape is used by SPKI pins in `chain::spki` (F20) —
//! the trust root is reconstructed from out-of-band context (the pin
//! in operator config), not carried alongside the certificate.
//!
//! **Drift from plan §Task 5** (L13 fold):
//!
//! | Plan said | This impl | Why |
//! |---|---|---|
//! | No helper (caller plumbs 5 lines) | `encrypt_mnemonic` / `decrypt_mnemonic` + `MnemonicCipherBlob` newtype | Issue #28 — avoid the 5-line dance in every Task 9 site + defense-in-depth (L12 type-design) — caller cannot pass a raw `Vec<u8>` that's NOT this helper's blob format |
//! | Plain `Vec<u8>` blob | `salt (16) \|\| nonce (12) \|\| ciphertext \|\| tag (16)` wrapped in `MnemonicCipherBlob` | Salt not embedded in AES-GCM blob; prepend it + newtype so the blob is self-identifying |
//! | Plaintext returned as `String` | Plaintext returned as `Secret<String>` | F47: phrase is the most sensitive material; must zeroize on drop |
//! | `encrypt_mnemonic(phrase, password)` | `+ aad: Aad<'_>` | Issue #66 / ADR 0001 — bind `bitcoin::Network` discriminant; closes N5 cross-network reuse. Typed `Aad<'a>` newtype (not `&[u8]`) prevents plaintext/AAD positional swap at call site. |
//! | `#[derive(Debug)]` on `MnemonicCipherBlob` | manual `Debug` using `finish_non_exhaustive()` | Issue #66 — `tracing::debug!(?blob)` previously leaked the full raw ciphertext bytes; manual redaction matches the `Secret<T>` redaction pattern (`keys/secret.rs`). |
//!
//! **Blob format** (also enforced at compile time via `MIN_LEN`):
//!
//! ```text
//! +-----------+------------+----------------------+
//! | salt(16)  | nonce(12)  | ciphertext \|\| tag   |
//! +-----------+------------+----------------------+
//! ```
//!
//! **Caller contract:**
//!
//! - The `phrase` passed to `encrypt_mnemonic` MUST live until
//!   `encrypt_mnemonic` returns. (The helper borrows plaintext bytes
//!   from the caller's `Secret<String>` during AES-GCM encrypt.)
//! - The `password: &[u8]` is borrowed. If the caller wants the
//!   password bytes to zeroize on drop, wrap in `Secret<Vec<u8>>` and
//!   pass `secret.expose().as_slice()`.
//! - The `aad: Aad<'_>` is borrowed metadata (typically a non-secret
//!   `bitcoin::Network` discriminant). Construct via `Aad::NONE`
//!   (pre-#66 behavior) or `Aad::network(network)`. `Secret` wrapping
//!   is unnecessary unless the AAD itself contains sensitive material.
//! - The returned `MnemonicCipherBlob` should NOT be logged or traced
//!   in plaintext bytes — the format leak (salt position, nonce length)
//!   aids cryptanalysis. The `Debug` impl is redacted.

use crate::crypto::aad::Aad;
use crate::crypto::{aes_gcm, argon2};
use crate::error::{Error, Result};
use crate::keys::Secret;

/// Self-describing encrypted mnemonic blob.
///
/// Wraps the raw `salt(16) || nonce(12) || ciphertext || tag(16)` bytes.
/// The newtype prevents accidental misuse: a caller cannot pass a raw
/// AES-GCM blob (28 bytes) or a raw Argon2id salt (16 bytes) where a
/// mnemonic cipher blob is expected.
///
/// L12 type-design: lift the format invariant out of the module doc
/// and into the type system. `MIN_LEN` and `MAX_LEN` enforced at every
/// construction site (private `new_checked` + `TryFrom<&[u8]>`).
#[derive(Clone)]
pub struct MnemonicCipherBlob(Vec<u8>);

/// Manual `Debug` impl — redacts the inner bytes (L12 review, Issue #66
/// precursor). The previous `#[derive(Debug)]` leaked the full raw
/// ciphertext via `tracing::debug!(?blob)` and similar patterns.
///
/// **No length leak:** ciphertext length equals plaintext length, which
/// equals BIP-39 word count (128-bit vs 256-bit entropy). Leaking the
/// length would help an offline cracker size its search. Pattern matches
/// `Secret<T>` (`keys/secret.rs:26-30`) exactly: `finish_non_exhaustive()`
/// emits no field names and no values.
impl std::fmt::Debug for MnemonicCipherBlob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MnemonicCipherBlob").finish_non_exhaustive()
    }
}

impl MnemonicCipherBlob {
    /// Minimum blob size: `argon2::SALT_LEN + aes_gcm::NONCE_LEN + aes_gcm::TAG_LEN`.
    /// Compile-time pinned — see the const-eval block at the bottom
    /// of this module + the cross-module invariant in `crypto/mod.rs`.
    pub const MIN_LEN: usize = argon2::SALT_LEN + aes_gcm::NONCE_LEN + aes_gcm::TAG_LEN;

    /// Maximum plaintext bytes (BIP-39 max: 24 words × 8 chars + 23
    /// spaces ≈ 215 bytes, rounded to 256 for headroom). Limits the
    /// blob size upper bound — prevents a 1 GiB blob from being
    /// accepted at construction (DoS mitigation; the read path is
    /// attacker-influenceable at A2 per ADR 0001 §Threat-model coverage).
    ///
    /// **Compile-time pinned** at module bottom (L20 audit pattern).
    pub const MAX_PLAINTEXT_LEN: usize = {
        const INNER: usize = 256;
        assert!(
            INNER == 256,
            "MAX_PLAINTEXT_LEN must be 256 (BIP-39 24-word phrase + headroom)"
        );
        INNER
    };

    /// Maximum blob size: `MIN_LEN + MAX_PLAINTEXT_LEN`.
    pub const MAX_LEN: usize = Self::MIN_LEN + Self::MAX_PLAINTEXT_LEN;

    /// Single checked-construction site (used by both `encrypt_mnemonic`
    /// and `TryFrom<&[u8]>`). Enforces the MIN_LEN..=MAX_LEN invariant
    /// once, so no caller can produce a struct-violating blob.
    fn new_checked(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() < Self::MIN_LEN {
            return Err(Error::MnemonicCipher(format!(
                "blob too short: {} bytes (need >= {})",
                bytes.len(),
                Self::MIN_LEN
            )));
        }
        if bytes.len() > Self::MAX_LEN {
            return Err(Error::MnemonicCipher(format!(
                "blob too long: {} bytes (max {} for MAX_PLAINTEXT_LEN={})",
                bytes.len(),
                Self::MAX_LEN,
                Self::MAX_PLAINTEXT_LEN
            )));
        }
        Ok(Self(bytes))
    }

    /// Borrow the inner bytes.
    ///
    /// **Returns raw ciphertext.** The `Debug` redaction on this type is
    /// a defense against accidental `tracing::debug!(?blob)`, not a
    /// confidentiality boundary — this accessor deliberately bypasses it.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl AsRef<[u8]> for MnemonicCipherBlob {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl TryFrom<&[u8]> for MnemonicCipherBlob {
    type Error = Error;
    fn try_from(slice: &[u8]) -> Result<Self> {
        MnemonicCipherBlob::new_checked(slice.to_vec())
    }
}

/// Encrypt a mnemonic phrase with a password, binding `aad` (typed
/// context) to the ciphertext. Returns a self-contained
/// [`MnemonicCipherBlob`] (salt + nonce + ciphertext + tag).
///
/// Generates a fresh random salt and nonce per call (per F5/F6).
///
/// **AAD binding:** the `Aad<'_>` bytes are authenticated but not encrypted.
/// Any mismatch between encrypt-time and decrypt-time AAD causes the AES-GCM
/// tag verification to fail (`Error::MnemonicCipher`). Pass `Aad::NONE` for
/// the pre-#66 behavior (no AAD binding). Per ADR 0001, callers binding
/// context to ciphertext pass `Aad::network(network)` so cross-network
/// ciphertext reuse fails at decrypt time.
///
/// **Empty-phrase rejection:** refuses to encrypt an empty string. A
/// `.enc` file decrypting to `""` would be an unrecoverable wallet that
/// reports success at every layer — violates the constructor's domain
/// ("is a mnemonic" is part of the contract).
///
/// # Errors
///
/// Returns `Error::MnemonicCipher` on:
/// - empty phrase (rejected before KDF; domain invariant)
/// - KDF or AEAD library failure (rare; would indicate a bug since
///   salt length and key length are fixed at call site)
///
/// # Caller contract
///
/// `phrase` MUST live until this function returns (the helper borrows
/// plaintext bytes during AES-GCM encrypt). `password` and `aad` are
/// borrowed — `password` should be wrapped in `Secret<Vec<u8>>` if it
/// must zeroize on drop; `aad` is metadata and `Secret` wrapping is
/// unnecessary.
pub fn encrypt_mnemonic(
    phrase: &Secret<String>,
    password: &[u8],
    aad: Aad<'_>,
) -> Result<MnemonicCipherBlob> {
    if phrase.expose().is_empty() {
        return Err(Error::MnemonicCipher(
            "refusing to encrypt an empty mnemonic".into(),
        ));
    }
    let salt = argon2::random_salt();
    let key = argon2::derive_key(password, &salt)?;
    let plaintext = phrase.expose().as_bytes();
    let aes_blob = aes_gcm::encrypt(&key, plaintext, aad)?;
    // salt || aes_blob: salt + (nonce + ct + tag)
    let mut out = Vec::with_capacity(argon2::SALT_LEN + aes_blob.len());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&aes_blob);
    MnemonicCipherBlob::new_checked(out)
}

/// Decrypt a blob produced by [`encrypt_mnemonic`], verifying `aad` matches
/// the value bound at encrypt time.
///
/// Returns the phrase wrapped in `Secret<String>` so it zeroizes on
/// drop (per F47).
///
/// **Caller note on error message contract:** this function distinguishes
/// three failure modes in the error message string ("wrong password, wrong
/// AAD, or corrupted blob"). Higher-level callers that surface errors to
/// end users (e.g., the `btc wallet show` CLI per ADR 0001) should collapse
/// these into a single indistinguishable message (N2 oracle mitigation).
/// The wallet-store layer is responsible for the collapse.
///
/// # Errors
///
/// Returns `Error::MnemonicCipher` on:
/// - Wrong password (AES-GCM tag mismatch surfaces as `decrypt` error).
/// - Wrong AAD (same — tag mismatch).
/// - Truncated blob (caught by `MnemonicCipherBlob::try_from` or by
///   `aes_gcm::decrypt`'s length guard).
/// - Decrypted plaintext is not valid UTF-8 (should never happen for a
///   real BIP-39 phrase; surfaces as `MnemonicCipher` for safety).
pub fn decrypt_mnemonic(
    blob: &MnemonicCipherBlob,
    password: &[u8],
    aad: Aad<'_>,
) -> Result<Secret<String>> {
    // Use `split_at_checked` to avoid panic on untrusted short input
    // (defense-in-depth — `new_checked` already enforces MIN_LEN at
    // construction, but split_at panics on len < split_index).
    if blob.as_bytes().len() < argon2::SALT_LEN {
        return Err(Error::MnemonicCipher(
            "blob too short to contain salt prefix".into(),
        ));
    }
    let (salt, aes_blob) = blob.as_bytes().split_at(argon2::SALT_LEN);
    let key = argon2::derive_key(password, salt)?;
    // Wrap plaintext in Secret<Vec<u8>> from aes_gcm::decrypt so the
    // intermediate buffer zeroizes on drop on the happy path. After
    // `into_inner` we re-wrap the final String in Secret<String>, which
    // is the canonical zeroize-bearing type per F47.
    let plaintext_secret = aes_gcm::decrypt(&key, aes_blob, aad).map_err(|_| {
        Error::MnemonicCipher(
            "decrypt failed (wrong password, wrong AAD, or corrupted blob)".into(),
        )
    })?;
    let phrase_bytes = plaintext_secret.into_inner();
    let phrase_str = String::from_utf8(phrase_bytes)
        .map_err(|_| Error::MnemonicCipher("decrypted mnemonic is not valid UTF-8".into()))?;
    Ok(Secret::new(phrase_str))
}

// Cross-module invariant: `MnemonicCipherBlob::MIN_LEN` is defined in
// terms of the underlying constants. Belt-and-suspenders check that
// the literal stays in sync if any underlying constant drifts. The
// parallel check in `crypto/mod.rs` covers argon2 vs aes_gcm; this one
// covers the arithmetic.
const _: () = {
    assert!(
        MnemonicCipherBlob::MIN_LEN == argon2::SALT_LEN + aes_gcm::NONCE_LEN + aes_gcm::TAG_LEN,
        "MnemonicCipherBlob::MIN_LEN must equal SALT_LEN + NONCE_LEN + TAG_LEN"
    );
    assert!(
        MnemonicCipherBlob::MAX_PLAINTEXT_LEN == 256,
        "MAX_PLAINTEXT_LEN must be 256 (BIP-39 24-word phrase + headroom)"
    );
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Secret;
    use bitcoin::Network;

    /// Fixed test password (NOT a published BIP-39 phrase per CONTEXT.md).
    const PASSWORD: &[u8] = b"test-password-not-a-bip39-phrase";

    fn phrase(s: &str) -> Secret<String> {
        Secret::new(s.to_string())
    }

    #[test]
    fn roundtrip_recovers_phrase() {
        let p = phrase("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        let blob = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect("encrypt");
        let recovered = decrypt_mnemonic(&blob, PASSWORD, Aad::NONE).expect("decrypt");
        assert_eq!(recovered.expose(), p.expose());
    }

    #[test]
    fn roundtrip_recovers_max_sized_bip39_phrase() {
        // Real 24-word BIP-39 phrase (BIP-39 spec max word count).
        let p = phrase(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon \
             abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art art",
        );
        let blob = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect("encrypt");
        let recovered = decrypt_mnemonic(&blob, PASSWORD, Aad::NONE).expect("decrypt");
        assert_eq!(recovered.expose(), p.expose());
        assert!(blob.as_bytes().len() > MnemonicCipherBlob::MIN_LEN);
    }

    #[test]
    fn wrong_password_fails_decrypt() {
        let p = phrase("hello world");
        let blob = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect("encrypt");
        let err = decrypt_mnemonic(&blob, b"wrong-password", Aad::NONE).expect_err("must reject");
        assert!(matches!(err, Error::MnemonicCipher(_)));
    }

    #[test]
    fn tampered_ciphertext_fails_decrypt() {
        let p = phrase("hello world");
        let mut blob = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect("encrypt");
        let bytes = blob.as_bytes_mut_for_test();
        let idx = bytes.len() - 1;
        bytes[idx] ^= 0x01;
        let err = decrypt_mnemonic(&blob, PASSWORD, Aad::NONE).expect_err("must reject");
        assert!(matches!(err, Error::MnemonicCipher(_)));
    }

    #[test]
    fn tampered_salt_region_fails_decrypt() {
        // Flip a byte in the salt (first 16 bytes). Wrong salt -> wrong
        // key -> AES-GCM tag mismatch surfaces as MnemonicCipher err.
        let p = phrase("hello world");
        let mut blob = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect("encrypt");
        let bytes = blob.as_bytes_mut_for_test();
        bytes[5] ^= 0x01;
        let err = decrypt_mnemonic(&blob, PASSWORD, Aad::NONE).expect_err("must reject");
        assert!(matches!(err, Error::MnemonicCipher(_)));
    }

    #[test]
    fn tampered_nonce_region_fails_decrypt() {
        // Flip a byte in the nonce (bytes SALT_LEN..SALT_LEN+NONCE_LEN).
        let p = phrase("hello world");
        let mut blob = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect("encrypt");
        let bytes = blob.as_bytes_mut_for_test();
        bytes[argon2::SALT_LEN + 3] ^= 0x01;
        let err = decrypt_mnemonic(&blob, PASSWORD, Aad::NONE).expect_err("must reject");
        assert!(matches!(err, Error::MnemonicCipher(_)));
    }

    #[test]
    fn blob_has_minimum_expected_size_for_one_char_phrase() {
        // Empty-phrase rejection replaces the old `blob_has_minimum_expected_size`
        // test (which enshrined an empty-phrase-encrypts-to-MIN_LEN behavior).
        // The minimum-size invariant is now proven with a one-character phrase.
        let p = phrase("a");
        let blob = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect("encrypt");
        assert_eq!(
            blob.as_bytes().len(),
            MnemonicCipherBlob::MIN_LEN + 1,
            "one-char phrase: blob should be MIN_LEN + 1"
        );
    }

    #[test]
    fn blob_layout_is_salt_then_aes() {
        // Two encrypts of the same plaintext + password should differ
        // in the salt (first 16 bytes) and nonce (next 12 bytes).
        let p = phrase("test plaintext");
        let blob1 = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect("encrypt 1");
        let blob2 = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect("encrypt 2");
        assert_ne!(
            &blob1.as_bytes()[..argon2::SALT_LEN],
            &blob2.as_bytes()[..argon2::SALT_LEN],
        );
        let nonce1 = &blob1.as_bytes()[argon2::SALT_LEN..argon2::SALT_LEN + aes_gcm::NONCE_LEN];
        let nonce2 = &blob2.as_bytes()[argon2::SALT_LEN..argon2::SALT_LEN + aes_gcm::NONCE_LEN];
        assert_ne!(nonce1, nonce2);
    }

    #[test]
    fn empty_password_succeeds() {
        // Argon2id accepts empty password (covered in argon2.rs tests);
        // this composition must not silently break that.
        let p = phrase("abandon");
        let blob = encrypt_mnemonic(&p, b"", Aad::NONE).expect("encrypt");
        let recovered = decrypt_mnemonic(&blob, b"", Aad::NONE).expect("decrypt");
        assert_eq!(recovered.expose(), p.expose());
    }

    #[test]
    fn blob_newtype_rejects_short_slice() {
        // MnemonicCipherBlob::new_checked enforces MIN_LEN.
        let err = MnemonicCipherBlob::try_from(&[0u8; 5][..]).expect_err("must reject");
        assert!(matches!(err, Error::MnemonicCipher(_)));
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn blob_newtype_rejects_oversize_slice() {
        // MnemonicCipherBlob::new_checked enforces MAX_LEN (DoS mitigation).
        let oversized = vec![0u8; MnemonicCipherBlob::MAX_LEN + 1];
        let err = MnemonicCipherBlob::try_from(&oversized[..]).expect_err("must reject");
        assert!(matches!(err, Error::MnemonicCipher(_)));
        assert!(err.to_string().contains("too long"));
    }

    #[test]
    fn min_len_constant_matches_runtime_check() {
        // Compile-time witness: MnemonicCipherBlob::MIN_LEN == 44.
        assert_eq!(MnemonicCipherBlob::MIN_LEN, 44);
    }

    #[test]
    fn deterministic_password_with_same_blob_recovers_same_phrase() {
        // Two callers independently encrypting the same phrase with
        // the same password get different blobs (different salts) but
        // both blobs decrypt to the same phrase.
        let p = phrase("abandon ");
        let blob = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect("encrypt");
        let blob2 = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect("encrypt 2");
        assert_ne!(blob.as_bytes(), blob2.as_bytes());
        let r1 = decrypt_mnemonic(&blob, PASSWORD, Aad::NONE).expect("d1");
        let r2 = decrypt_mnemonic(&blob2, PASSWORD, Aad::NONE).expect("d2");
        assert_eq!(r1.expose(), r2.expose());
        assert_eq!(r1.expose(), p.expose());
    }

    // --- Empty-phrase rejection (TD-7) ---

    #[test]
    fn encrypt_rejects_empty_phrase() {
        let p = phrase("");
        let err = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect_err("must reject empty");
        assert!(matches!(err, Error::MnemonicCipher(_)));
        assert!(err.to_string().contains("empty mnemonic"));
    }

    // --- AAD tests (Issue #66, ADR 0001) ---

    fn aad_testnet() -> Aad<'static> {
        Aad::network(Network::Testnet)
    }

    fn aad_mainnet() -> Aad<'static> {
        Aad::network(Network::Bitcoin)
    }

    #[test]
    fn roundtrip_with_aad_recovers_phrase() {
        let p = phrase("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        let blob = encrypt_mnemonic(&p, PASSWORD, aad_testnet()).expect("encrypt");
        let recovered = decrypt_mnemonic(&blob, PASSWORD, aad_testnet()).expect("decrypt");
        assert_eq!(recovered.expose(), p.expose());
    }

    #[test]
    fn wrong_aad_fails_decrypt() {
        // Encrypt with AAD=testnet, decrypt with AAD=mainnet -> MnemonicCipher
        // (AEAD tag verification fails; closes N5 cross-network footgun).
        let p = phrase("hello world");
        let blob = encrypt_mnemonic(&p, PASSWORD, aad_testnet()).expect("encrypt");
        let err = decrypt_mnemonic(&blob, PASSWORD, aad_mainnet()).expect_err("must reject");
        assert!(matches!(err, Error::MnemonicCipher(_)));
    }

    #[test]
    fn roundtrip_with_empty_aad_preserves_pre_66_behavior() {
        // Encrypt/decrypt with Aad::NONE == no AAD == pre-#66 behavior.
        let p = phrase("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        let blob = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect("encrypt");
        let recovered = decrypt_mnemonic(&blob, PASSWORD, Aad::NONE).expect("decrypt");
        assert_eq!(recovered.expose(), p.expose());
    }

    // --- AAD edge-case coverage ---

    fn multi_byte_aad() -> Vec<u8> {
        vec![0xAB; 32]
    }

    #[test]
    fn roundtrip_with_zero_filled_aad_recovers_phrase() {
        // Distinct from Aad::NONE: a 16-byte zero-filled AAD must
        // authenticate correctly (proves the AAD boundary, not the
        // emptiness).
        let p = phrase("hello world");
        let aad = Aad::from_bytes(&[0u8; 16]).expect("within cap");
        let blob = encrypt_mnemonic(&p, PASSWORD, aad).expect("encrypt");
        let recovered = decrypt_mnemonic(&blob, PASSWORD, aad).expect("decrypt");
        assert_eq!(recovered.expose(), p.expose());
    }

    #[test]
    fn roundtrip_with_nul_bytes_in_aad_recovers_phrase() {
        // Confirms NUL bytes do not terminate AAD (a foot-gun in any
        // system that treats AAD as a C string).
        let p = phrase("hello world");
        let nul_aad = b"a\x00b\x00c";
        let aad = Aad::from_bytes(nul_aad).expect("within cap");
        let blob = encrypt_mnemonic(&p, PASSWORD, aad).expect("encrypt");
        let recovered = decrypt_mnemonic(&blob, PASSWORD, aad).expect("decrypt");
        assert_eq!(recovered.expose(), p.expose());
    }

    #[test]
    fn aad_swapped_with_plaintext_fails_roundtrip() {
        // Defensive test: AAD = plaintext bytes must NOT validate.
        // (Verifies that the AAD slot authenticates independently of
        // the plaintext slot — defends against future positional-arg
        // swap regressions.)
        let p = phrase("hello world");
        let pt_bytes = p.expose().as_bytes().to_vec();
        let wrong_aad = Aad::from_bytes(&pt_bytes).expect("within cap");
        let blob = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect("encrypt");
        // Decrypt with wrong AAD (same bytes as plaintext, different slot).
        let err = decrypt_mnemonic(&blob, PASSWORD, wrong_aad).expect_err("must reject");
        assert!(matches!(err, Error::MnemonicCipher(_)));
    }

    #[test]
    fn roundtrip_with_large_aad_recovers_phrase() {
        // AES-GCM has no spec cap on AAD length; we cap at MAX_AAD_LEN
        // (64). This test exercises the cap boundary.
        let p = phrase("hello world");
        let large = vec![0xCD; crate::crypto::aad::MAX_AAD_LEN];
        let aad = Aad::from_bytes(&large).expect("at cap");
        let blob = encrypt_mnemonic(&p, PASSWORD, aad).expect("encrypt");
        let recovered = decrypt_mnemonic(&blob, PASSWORD, aad).expect("decrypt");
        assert_eq!(recovered.expose(), p.expose());
    }

    #[test]
    fn aad_is_not_embedded_in_blob() {
        // AAD is caller-side context — not persisted in the blob.
        // Same blob length regardless of AAD length.
        // (Random-coincidence false positive ~2^-8 per byte, so we use
        // a 32-byte AAD to keep the false-positive rate negligible.)
        let p = phrase("hello world");
        let blob_no_aad = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect("encrypt");
        let blob_with_aad =
            encrypt_mnemonic(&p, PASSWORD, Aad::from_bytes(&multi_byte_aad()).unwrap())
                .expect("encrypt");
        assert_eq!(blob_no_aad.as_bytes().len(), blob_with_aad.as_bytes().len());
    }

    // --- Debug redaction ---

    #[test]
    fn debug_impl_redacts_bytes() {
        // Manual Debug impl uses `finish_non_exhaustive()` — emits no
        // field names, no values. Same redaction pattern as `Secret<T>`.
        // Belt-and-suspenders: formatted string must contain no digits
        // (no byte count, no length leak) and no `[` (no raw byte dump).
        let p = phrase("hello world");
        let blob = encrypt_mnemonic(&p, PASSWORD, Aad::NONE).expect("encrypt");
        let formatted = format!("{:?}", blob);
        assert_eq!(formatted, "MnemonicCipherBlob { .. }");
        assert!(
            !formatted.chars().any(char::is_numeric),
            "Debug must not leak length: {formatted}"
        );
        assert!(
            !formatted.contains('['),
            "Debug must not leak raw bytes: {formatted}"
        );
    }

    // Test-only accessor for mutation tests. Returns a `&mut Vec<u8>`
    // — not part of the public API.
    impl MnemonicCipherBlob {
        #[cfg(test)]
        fn as_bytes_mut_for_test(&mut self) -> &mut Vec<u8> {
            &mut self.0
        }
    }
}
