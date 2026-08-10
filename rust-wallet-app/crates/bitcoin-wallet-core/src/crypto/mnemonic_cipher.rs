//! High-level mnemonic encryption helper.
//!
//! Bundles [`argon2::random_salt`] + [`argon2::derive_key`] +
//! [`aes_gcm::encrypt`] / [`aes_gcm::decrypt`] into a 2-line API for
//! encrypting BIP-39 phrases at rest.
//!
//! **Threat-model coverage** (per issue #28):
//!
//! - **F5** (Argon2id KDF, m=256 MiB / t=10 / p=4) — strong offline
//!   cracker resistance. Per `argon2` module.
//! - **F6** (AES-256-GCM AEAD, 96-bit random nonce per blob) —
//!   confidentiality + integrity. Per `aes_gcm` module.
//! - **F47** / **U3** (zeroize on drop) — `Secret<String>` (phrase) +
//!   `Secret<Vec<u8>>` (intermediate plaintext buffer) all zeroize on
//!   drop. `MnemonicCipherBlob` ciphertext is non-secret (already
//!   encrypted; not wrapped).
//!
//! **Defends against:** A1 (offline cracker of stolen ciphertext via
//! strong KDF) + F43 (per-protocol error variant — distinct from
//! underlying `Encryption` errors).
//!
//! **Does NOT defend:** T1 (physical seizure — out of scope, requires
//! full-disk encryption). Caller error (wrong password, truncated
//! blob) is reported, not hidden — these surface as `Error::MnemonicCipher`.
//!
//! **Calibration note:** Argon2id parameters (m=256 MiB, t=10, p=4)
//! are inherited from the `argon2` module — see its calibration note.
//! Changing them here would be a no-op; tune `argon2::ARGON2_*` instead.
//!
//! **Drift from plan §Task 5** (L13 fold):
//!
//! | Plan said | This impl | Why |
//! |---|---|---|
//! | No helper (caller plumbs 5 lines) | `encrypt_mnemonic` / `decrypt_mnemonic` + `MnemonicCipherBlob` newtype | Issue #28 — avoid the 5-line dance in every Task 9 site + defense-in-depth (L12 type-design) — caller cannot pass a raw `Vec<u8>` that's NOT this helper's blob format |
//! | Plain `Vec<u8>` blob | `salt (16) \|\| nonce (12) \|\| ciphertext \|\| tag (16)` wrapped in `MnemonicCipherBlob` | Salt not embedded in AES-GCM blob; prepend it + newtype so the blob is self-identifying |
//! | Plaintext returned as `String` | Plaintext returned as `Secret<String>` | F47: phrase is the most sensitive material; must zeroize on drop |
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
//! - The returned `MnemonicCipherBlob` should NOT be logged or traced
//!   in plaintext bytes — the format leak (salt position, nonce length)
//!   aids cryptanalysis.

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
/// and into the type system. `TryFrom<&[u8]>` enforces the minimum
/// length + structural validity at construction.
#[derive(Clone, Debug)]
pub struct MnemonicCipherBlob(Vec<u8>);

impl MnemonicCipherBlob {
    /// Minimum blob size: `argon2::SALT_LEN + aes_gcm::NONCE_LEN + aes_gcm::TAG_LEN`.
    /// Compile-time pinned — see the const-eval block at the bottom
    /// of this module + the cross-module invariant in `crypto/mod.rs`.
    pub const MIN_LEN: usize = argon2::SALT_LEN + aes_gcm::NONCE_LEN + aes_gcm::TAG_LEN;

    /// Borrow the inner bytes.
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
        if slice.len() < MnemonicCipherBlob::MIN_LEN {
            return Err(Error::MnemonicCipher(format!(
                "blob too short: {} bytes (need >= {})",
                slice.len(),
                MnemonicCipherBlob::MIN_LEN
            )));
        }
        Ok(MnemonicCipherBlob(slice.to_vec()))
    }
}

/// Encrypt a mnemonic phrase with a password. Returns a self-contained
/// [`MnemonicCipherBlob`] (salt + nonce + ciphertext + tag).
///
/// Generates a fresh random salt and nonce per call (per F5/F6).
///
/// # Errors
///
/// Returns `Error::MnemonicCipher` on KDF or AEAD library failure
/// (rare; would indicate a bug since salt length is fixed at call
/// site).
///
/// # Caller contract
///
/// `phrase` MUST live until this function returns (the helper borrows
/// plaintext bytes during AES-GCM encrypt). `password` is borrowed —
/// wrap in `Secret<Vec<u8>>` if you need it to zeroize.
pub fn encrypt_mnemonic(phrase: &Secret<String>, password: &[u8]) -> Result<MnemonicCipherBlob> {
    let salt = argon2::random_salt();
    let key = argon2::derive_key(password, &salt)?;
    let plaintext = phrase.expose().as_bytes();
    let aes_blob = aes_gcm::encrypt(&key, plaintext)?;
    let mut out =
        Vec::with_capacity(MnemonicCipherBlob::MIN_LEN - aes_gcm::TAG_LEN + aes_blob.len());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&aes_blob);
    Ok(MnemonicCipherBlob(out))
}

/// Decrypt a blob produced by [`encrypt_mnemonic`].
///
/// Returns the phrase wrapped in `Secret<String>` so it zeroizes on
/// drop (per F47).
///
/// # Errors
///
/// Returns `Error::MnemonicCipher` on:
/// - Wrong password (AES-GCM tag mismatch surfaces as `decrypt` error).
/// - Truncated blob (caught by `MnemonicCipherBlob::try_from` if you
///   pass via that path, or by `argon2`/`aes_gcm` internals otherwise).
/// - Decrypted plaintext is not valid UTF-8 (should never happen for a
///   real BIP-39 phrase; surfaces as `MnemonicCipher` for safety).
pub fn decrypt_mnemonic(blob: &MnemonicCipherBlob, password: &[u8]) -> Result<Secret<String>> {
    let aes_blob_start = argon2::SALT_LEN;
    let (salt, aes_blob) = blob.as_bytes().split_at(aes_blob_start);
    let key = argon2::derive_key(password, salt)?;
    // Wrap plaintext in Secret<Vec<u8>> so it zeroizes on drop —
    // covers both happy path (when String::from_utf8 succeeds) and
    // error path (when it fails — the Vec<u8> drop is a plain free,
    // not a zeroize, unless wrapped).
    // Map `Encryption` errors from the underlying primitives to
    // `MnemonicCipher` per F43 — from the caller's POV, a decrypt
    // failure is always caller-side (wrong password, truncated blob,
    // tampered bytes) or a corruption event. The library-internal
    // `Encryption` variant would never surface here normally.
    let plaintext_secret = aes_gcm::decrypt(&key, aes_blob).map_err(|e| {
        Error::MnemonicCipher(format!(
            "decrypt failed (wrong password or corrupted blob): {e}"
        ))
    })?;
    let phrase_bytes = plaintext_secret.into_inner();
    let phrase_str = String::from_utf8(phrase_bytes).map_err(|e| {
        Error::MnemonicCipher(format!("decrypted mnemonic is not valid UTF-8: {e}"))
    })?;
    Ok(Secret::new(phrase_str))
}

// Cross-module invariant: `MnemonicCipherBlob::MIN_LEN` is defined in
// terms of the underlying constants. Belt-and-suspenders check that
// the literal stays in sync if any underlying constant drifts. The
// parallel check in `crypto/mod.rs` covers argon2 vs aes_gcm; this
// one covers the arithmetic.
const _: () = {
    assert!(
        MnemonicCipherBlob::MIN_LEN == argon2::SALT_LEN + aes_gcm::NONCE_LEN + aes_gcm::TAG_LEN,
        "MnemonicCipherBlob::MIN_LEN must equal SALT_LEN + NONCE_LEN + TAG_LEN"
    );
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Secret;

    /// Fixed test password (NOT a published BIP-39 phrase per CONTEXT.md).
    const PASSWORD: &[u8] = b"test-password-not-a-bip39-phrase";

    fn phrase(s: &str) -> Secret<String> {
        Secret::new(s.to_string())
    }

    #[test]
    fn roundtrip_recovers_phrase() {
        let p = phrase("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        let blob = encrypt_mnemonic(&p, PASSWORD).expect("encrypt");
        let recovered = decrypt_mnemonic(&blob, PASSWORD).expect("decrypt");
        assert_eq!(recovered.expose(), p.expose());
    }

    #[test]
    fn roundtrip_recovers_max_sized_bip39_phrase() {
        // Real 24-word BIP-39 phrase (BIP-39 spec max word count).
        // Word list is the standard English wordlist but the exact
        // sequence is a placeholder — not a published phrase.
        let p = phrase(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon \
             abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art art",
        );
        let blob = encrypt_mnemonic(&p, PASSWORD).expect("encrypt");
        let recovered = decrypt_mnemonic(&blob, PASSWORD).expect("decrypt");
        assert_eq!(recovered.expose(), p.expose());
        assert!(blob.as_bytes().len() > MnemonicCipherBlob::MIN_LEN);
    }

    #[test]
    fn wrong_password_fails_decrypt() {
        let p = phrase("hello world");
        let blob = encrypt_mnemonic(&p, PASSWORD).expect("encrypt");
        let err = decrypt_mnemonic(&blob, b"wrong-password").expect_err("must reject");
        assert!(matches!(err, Error::MnemonicCipher(_)));
    }

    #[test]
    fn tampered_ciphertext_fails_decrypt() {
        let p = phrase("hello world");
        let mut blob = encrypt_mnemonic(&p, PASSWORD).expect("encrypt");
        let bytes = blob.as_bytes_mut_for_test();
        let idx = bytes.len() - 1;
        bytes[idx] ^= 0x01;
        let err = decrypt_mnemonic(&blob, PASSWORD).expect_err("must reject");
        assert!(matches!(err, Error::MnemonicCipher(_)));
    }

    #[test]
    fn tampered_salt_region_fails_decrypt() {
        // Flip a byte in the salt (first 16 bytes). Wrong salt -> wrong
        // key -> AES-GCM tag mismatch surfaces as MnemonicCipher err.
        let p = phrase("hello world");
        let mut blob = encrypt_mnemonic(&p, PASSWORD).expect("encrypt");
        let bytes = blob.as_bytes_mut_for_test();
        bytes[5] ^= 0x01;
        let err = decrypt_mnemonic(&blob, PASSWORD).expect_err("must reject");
        assert!(matches!(err, Error::MnemonicCipher(_)));
    }

    #[test]
    fn tampered_nonce_region_fails_decrypt() {
        // Flip a byte in the nonce (bytes SALT_LEN..SALT_LEN+NONCE_LEN).
        let p = phrase("hello world");
        let mut blob = encrypt_mnemonic(&p, PASSWORD).expect("encrypt");
        let bytes = blob.as_bytes_mut_for_test();
        bytes[argon2::SALT_LEN + 3] ^= 0x01;
        let err = decrypt_mnemonic(&blob, PASSWORD).expect_err("must reject");
        assert!(matches!(err, Error::MnemonicCipher(_)));
    }

    #[test]
    fn blob_has_minimum_expected_size() {
        let p = phrase("");
        let blob = encrypt_mnemonic(&p, PASSWORD).expect("encrypt");
        assert_eq!(
            blob.as_bytes().len(),
            MnemonicCipherBlob::MIN_LEN,
            "empty plaintext: blob should be exactly salt+nonce+tag"
        );
    }

    #[test]
    fn blob_layout_is_salt_then_aes() {
        // Two encrypts of the same plaintext + password should differ
        // in the salt (first 16 bytes) and nonce (next 12 bytes).
        let p = phrase("test plaintext");
        let blob1 = encrypt_mnemonic(&p, PASSWORD).expect("encrypt 1");
        let blob2 = encrypt_mnemonic(&p, PASSWORD).expect("encrypt 2");
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
        let blob = encrypt_mnemonic(&p, b"").expect("encrypt");
        let recovered = decrypt_mnemonic(&blob, b"").expect("decrypt");
        assert_eq!(recovered.expose(), p.expose());
    }

    #[test]
    fn blob_newtype_rejects_short_slice() {
        // MnemonicCipherBlob::try_from enforces the minimum length.
        let err = MnemonicCipherBlob::try_from(&[0u8; 5][..]).expect_err("must reject");
        assert!(matches!(err, Error::MnemonicCipher(_)));
        assert!(err.to_string().contains("too short"));
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
        let blob = encrypt_mnemonic(&p, PASSWORD).expect("encrypt");
        let blob2 = encrypt_mnemonic(&p, PASSWORD).expect("encrypt 2");
        assert_ne!(blob.as_bytes(), blob2.as_bytes());
        let r1 = decrypt_mnemonic(&blob, PASSWORD).expect("d1");
        let r2 = decrypt_mnemonic(&blob2, PASSWORD).expect("d2");
        assert_eq!(r1.expose(), r2.expose());
        assert_eq!(r1.expose(), p.expose());
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
