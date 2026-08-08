//! AES-256-GCM AEAD. Per F6: mnemonic encrypted at rest.
//!
//! **Blob layout:** `nonce (12 bytes) || ciphertext || gcm_tag (16 bytes)`.
//! GCM tag is appended by `aes-gcm` crate and is part of `ciphertext`.
//!
//! **Nonce uniqueness:** 96-bit random nonce per encrypt call (via `OsRng`).
//! Birthday-collision risk: 2^32 messages per key before 50% collision.
//! Realistic wallet volume << 2^32; document for v0.1.1 audit.
//!
//! **Drift from plan §Task 5:**
//!
//! | Plan said | This impl | Why |
//! |---|---|---|
//! | `encrypt(key: &[u8; 32], pt)` | `encrypt(key: &Secret<Vec<u8>>, pt)` | L15: Copy defeats zeroize |
//! | `decrypt(key: &[u8; 32], blob) -> Result<Vec<u8>>` | `decrypt(key: &Secret<Vec<u8>>, blob) -> Result<Secret<Vec<u8>>>` | L15: plaintext also Secret-wrapped |
//! | key length implicit via `[u8; 32]` | key length enforced at API boundary (`Error::Encryption` on != 32) | `Key::from_slice` panics on wrong length; we convert panic to Result |
//! | `blob.len() < NONCE_LEN` check | `blob.len() < NONCE_LEN + TAG_LEN` check | Stricter: a blob shorter than NONCE+TAG is structurally invalid (plaintext would be empty + tag missing) |
//! | `Vec<u8>` ciphertext return | `Vec<u8>` ciphertext return (unchanged) | Ciphertext is by definition non-secret; `Secret<Vec<u8>>` would be YAGNI overhead per type-design review |
//! | Caller passes plain `&[u8]` plaintext | Caller passes plain `&[u8]` plaintext | Caller contract: wrap plaintext in `Secret<Vec<u8>>` before passing if zeroize-on-use is required. The borrow points into the caller's `Secret`, whose `ZeroizeOnDrop` fires on drop. |
//!
//! **Defends against:** U3 partial (plaintext zeroizes on drop), A1
//! (file theft without password yields only ciphertext + GCM tag).

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;

use crate::error::{Error, Result};
use crate::keys::Secret;

/// AES-GCM nonce length in bytes (96 bits, NIST SP 800-38D recommended).
pub const NONCE_LEN: usize = 12;

/// AES-GCM tag length in bytes (GCM spec — 128 bits).
pub const TAG_LEN: usize = 16;

/// AES-256 key length in bytes (32 bytes = 256 bits).
pub const KEY_LEN: usize = 32;

/// Encrypt `plaintext` with `key`. Returns `nonce || ciphertext` blob.
///
/// **Key length guard:** `key.expose().len() == 32` is enforced at the
/// API boundary. `Key::from_slice` would panic on length mismatch
/// (DoS vector); we convert to `Error::Encryption` instead.
///
/// Caller retains `key` (zeroize on drop via `Secret`); the returned
/// ciphertext does NOT need to be `Secret`-wrapped (it's already
/// encrypted, leaking it doesn't compromise the plaintext).
pub fn encrypt(key: &Secret<Vec<u8>>, plaintext: &[u8]) -> Result<Vec<u8>> {
    let key_bytes = key.expose();
    if key_bytes.len() != KEY_LEN {
        return Err(Error::Encryption(format!(
            "AES-256 requires {KEY_LEN}-byte key, got {}",
            key_bytes.len()
        )));
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key_bytes.as_slice()));
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| Error::Encryption(format!("aes-gcm encrypt: {e}")))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt `blob` (format: `nonce || ciphertext || tag`) with `key`.
/// Returns plaintext wrapped in `Secret<Vec<u8>>` per L15.
///
/// **Key length guard:** same as `encrypt` — `Error::Encryption` on
/// non-32-byte key instead of panic.
///
/// **Blob length guard:** blob must be at least `NONCE_LEN + TAG_LEN`
/// bytes (the minimum valid ciphertext: empty plaintext + tag).
pub fn decrypt(key: &Secret<Vec<u8>>, blob: &[u8]) -> Result<Secret<Vec<u8>>> {
    let key_bytes = key.expose();
    if key_bytes.len() != KEY_LEN {
        return Err(Error::Encryption(format!(
            "AES-256 requires {KEY_LEN}-byte key, got {}",
            key_bytes.len()
        )));
    }
    let min_blob = NONCE_LEN + TAG_LEN;
    if blob.len() < min_blob {
        return Err(Error::Encryption(format!(
            "blob too short: {} bytes (need >= {min_blob} for nonce + tag)",
            blob.len()
        )));
    }
    let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key_bytes.as_slice()));
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| Error::Encryption(format!("aes-gcm decrypt: {e}")))?;
    Ok(Secret::new(plaintext))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::argon2;

    fn fixed_key() -> Secret<Vec<u8>> {
        // Deterministic 32-byte key for round-trip tests.
        Secret::new(vec![7u8; KEY_LEN])
    }

    #[test]
    fn roundtrip_recovers_plaintext() {
        let key = fixed_key();
        let pt = b"hello world";
        let ct = encrypt(&key, pt).expect("encrypt");
        let pt2 = decrypt(&key, &ct).expect("decrypt");
        assert_eq!(pt, pt2.expose().as_slice());
    }

    #[test]
    fn wrong_key_fails_decrypt() {
        let k1 = Secret::new(vec![7u8; KEY_LEN]);
        let k2 = Secret::new(vec![8u8; KEY_LEN]);
        let ct = encrypt(&k1, b"secret").expect("encrypt");
        let err = decrypt(&k2, &ct).expect_err("must reject wrong key");
        assert!(matches!(err, Error::Encryption(_)));
    }

    #[test]
    fn tampered_ciphertext_fails_decrypt() {
        let key = fixed_key();
        let ct = encrypt(&key, b"secret").expect("encrypt");
        let mut tampered = ct.clone();
        // Flip a byte in the ciphertext (skip the 12-byte nonce prefix).
        let idx = NONCE_LEN + (tampered.len() - NONCE_LEN) / 2;
        tampered[idx] ^= 0x01;
        let err = decrypt(&key, &tampered).expect_err("must reject tampered ct");
        assert!(matches!(err, Error::Encryption(_)));
    }

    #[test]
    fn tampered_nonce_fails_decrypt() {
        let key = fixed_key();
        let ct = encrypt(&key, b"secret").expect("encrypt");
        let mut tampered = ct.clone();
        tampered[0] ^= 0x01;
        let err = decrypt(&key, &tampered).expect_err("must reject tampered nonce");
        assert!(matches!(err, Error::Encryption(_)));
    }

    #[test]
    fn blob_too_short_rejected() {
        let key = fixed_key();
        // 5 bytes: less than NONCE_LEN(12) + TAG_LEN(16) = 28.
        let err = decrypt(&key, &[0u8; 5]).expect_err("must reject short blob");
        assert!(matches!(err, Error::Encryption(_)));
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn blob_with_only_nonce_rejected() {
        let key = fixed_key();
        // 15 bytes: between NONCE_LEN(12) and NONCE+TAG(28). Still invalid.
        let err = decrypt(&key, &[0u8; NONCE_LEN + 3]).expect_err("must reject");
        assert!(matches!(err, Error::Encryption(_)));
    }

    #[test]
    fn encrypt_rejects_wrong_length_key() {
        let bad_key = Secret::new(vec![1u8; 16]); // too short
        let err = encrypt(&bad_key, b"x").expect_err("must reject 16-byte key");
        assert!(matches!(err, Error::Encryption(_)));
        assert!(err.to_string().contains("AES-256"));
    }

    #[test]
    fn decrypt_rejects_wrong_length_key() {
        let bad_key = Secret::new(vec![1u8; 64]); // too long
        let ct = encrypt(&fixed_key(), b"x").expect("encrypt");
        let err = decrypt(&bad_key, &ct).expect_err("must reject 64-byte key");
        assert!(matches!(err, Error::Encryption(_)));
    }

    #[test]
    fn nonce_is_random_per_call() {
        // Two encrypts of the same plaintext yield different blobs (with
        // overwhelming probability; 96-bit random nonce).
        let key = fixed_key();
        let ct1 = encrypt(&key, b"same plaintext").expect("e1");
        let ct2 = encrypt(&key, b"same plaintext").expect("e2");
        assert_ne!(ct1, ct2);
        // But both decrypt to the same plaintext.
        assert_eq!(
            decrypt(&key, &ct1).expect("decrypt ct1").expose(),
            b"same plaintext"
        );
        assert_eq!(
            decrypt(&key, &ct2).expect("decrypt ct2").expose(),
            b"same plaintext"
        );
    }

    #[test]
    fn plaintext_zero_length_works() {
        let key = fixed_key();
        let ct = encrypt(&key, b"").expect("encrypt empty");
        let pt = decrypt(&key, &ct).expect("decrypt empty");
        assert!(pt.expose().is_empty());
    }

    #[test]
    fn decrypt_returns_secret_wrapper() {
        // Compile-time witness: Secret<Vec<u8>>: ZeroizeOnDrop.
        fn assert_zod<T: zeroize::ZeroizeOnDrop>() {}
        assert_zod::<Secret<Vec<u8>>>();
        let key = fixed_key();
        let ct = encrypt(&key, b"x").expect("e");
        let pt = decrypt(&key, &ct).expect("d");
        let _: &Secret<Vec<u8>> = &pt;
    }

    #[test]
    fn end_to_end_via_argon2_aes_gcm() {
        // Integration witness: key derived via Argon2id decrypts blob
        // encrypted via AES-256-GCM (proves the two primitives compose).
        // Password is generically named (not a published phrase) per
        // CONTEXT.md hard rule #5 spirit.
        let password = b"integration-test-password-do-not-use";
        let salt = [0x42u8; argon2::SALT_LEN];
        let key = argon2::derive_key(password, &salt).expect("argon2");
        let pt = b"secret mnemonic phrase goes here";
        let ct = encrypt(&key, pt).expect("encrypt");
        let pt2 = decrypt(&key, &ct).expect("decrypt");
        assert_eq!(pt, pt2.expose().as_slice());
    }
}
