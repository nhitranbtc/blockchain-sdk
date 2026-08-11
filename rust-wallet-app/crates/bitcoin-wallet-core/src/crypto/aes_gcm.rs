//! AES-256-GCM AEAD. Per F6: mnemonic encrypted at rest.
//!
//! **Blob layout:** `nonce (12 bytes) || ciphertext || gcm_tag (16 bytes)`.
//! GCM tag is appended by `aes-gcm` crate and is part of `ciphertext`.
//!
//! **Nonce uniqueness:** 96-bit random nonce per encrypt call (via `OsRng`).
//! Birthday-collision risk: 2^32 messages per key before 50% collision.
//! Realistic wallet volume << 2^32; document for v0.1.1 audit.
//!
//! **AAD binding:** `encrypt` / `decrypt` accept `Aad<'a>` (the typed
//! context from `crypto::aad`). Per ADR 0001 the wallet-store layer binds
//! `bitcoin::Network` discriminant; AAD is authenticated but not encrypted,
//! and an AAD mismatch causes tag verification failure.
//!
//! **Drift from plan §Task 5:**
//!
//! | Plan said | This impl | Why |
//! |---|---|---|
//! | `encrypt(key: &[u8; 32], pt)` | `encrypt(key: &Secret<Vec<u8>>, plaintext, aad: Aad<'_>)` | L15: Copy defeats zeroize; Issue #66: typed AAD closes plaintext/AAD swap at call site |
//! | `decrypt(key: &[u8; 32], blob) -> Result<Vec<u8>>` | `decrypt(key: &Secret<Vec<u8>>, blob, aad: Aad<'_>) -> Result<Secret<Vec<u8>>>` | L15: plaintext also Secret-wrapped; Issue #66: AAD parameter for context binding |
//! | key length implicit via `[u8; 32]` | key length enforced at API boundary (`Error::Encryption` on != 32) | `Key::from_slice` panics on length mismatch; we convert panic to Result |
//! | `blob.len() < NONCE_LEN` check | `blob.len() < NONCE_LEN + TAG_LEN` check | Stricter: a blob shorter than NONCE+TAG is structurally invalid (plaintext would be empty + tag missing) |
//! | `Vec<u8>` ciphertext return | `Vec<u8>` ciphertext return (unchanged) | Ciphertext is by definition non-secret; `Secret<Vec<u8>>` would be YAGNI overhead per type-design review |
//! | Caller passes plain `&[u8]` plaintext | Caller passes plain `&[u8]` plaintext | Caller contract: wrap plaintext in `Secret<Vec<u8>>` before passing if zeroize-on-use is required. The borrow points into the caller's `Secret`, whose `ZeroizeOnDrop` fires on drop. |
//!
//! **Defends against:** U3 partial (plaintext zeroizes on drop), A1
//! (file theft without password yields only ciphertext + GCM tag),
//! N5 (cross-network ciphertext reuse — AAD mismatch fails at decrypt
//! time, see ADR 0001).

use aes_gcm::aead::{Aead, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;

use crate::crypto::aad::Aad;
use crate::error::{Error, Result};
use crate::keys::Secret;

/// AES-GCM nonce length in bytes (96 bits, NIST SP 800-38D §5.2.1.1 recommended).
/// Compile-time pinned: if the literal value drifts, the build fails here
/// (see Issue #30 constant audit at `docs/audit/2026-08-09-l20-constant-audit.md`).
pub const NONCE_LEN: usize = {
    const INNER: usize = 12;
    assert!(
        INNER == 12,
        "NONCE_LEN must be 12 bytes per NIST SP 800-38D §5.2.1.1 (got see source)"
    );
    INNER
};

/// AES-GCM tag length in bytes (128 bits, NIST SP 800-38D §5.2.7).
/// Compile-time pinned — see Issue #30.
pub const TAG_LEN: usize = {
    const INNER: usize = 16;
    assert!(
        INNER == 16,
        "TAG_LEN must be 16 bytes per NIST SP 800-38D §5.2.7 (got see source)"
    );
    INNER
};

/// AES-256 key length in bytes (256 bits, FIPS 197).
/// Compile-time pinned — see Issue #30.
pub const KEY_LEN: usize = {
    const INNER: usize = 32;
    assert!(
        INNER == 32,
        "KEY_LEN must be 32 bytes per FIPS 197 (got see source)"
    );
    INNER
};

/// Encrypt `plaintext` with `key`, binding `aad` (typed context) to the
/// ciphertext. Returns `nonce || ciphertext` blob.
///
/// **AAD binding:** the `Aad<'_>` bytes are authenticated but not encrypted.
/// Any mismatch between encrypt-time and decrypt-time AAD causes the AES-GCM
/// tag verification to fail. Per ADR 0001, callers bind `bitcoin::Network`
/// discriminant via `Aad::network(network)`.
/// **Caller contract for `Aad::NONE`** — the no-AAD case (`Aad::NONE`) is
/// byte-equivalent to no AAD at the AES-GCM layer (GHASH absorbs 0 blocks
/// for empty AAD). Pre-#66 callers passing `Aad::NONE` see identical behavior
/// to the pre-extension API. Pre-#66 on-disk blobs (written with no AAD)
/// must be decrypted with `Aad::NONE`; there is no in-band version byte to
/// detect this (see ADR 0001 §Cross-references, deferred version field).
///
/// # Errors
///
/// Returns `Error::Encryption` on:
/// - key length mismatch (not 32 bytes) — guards `Key::from_slice` panic (DoS vector)
/// - AEAD library failure (rare; indicates a bug since inputs are pre-validated)
pub fn encrypt(key: &Secret<Vec<u8>>, plaintext: &[u8], aad: Aad<'_>) -> Result<Vec<u8>> {
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
    let payload = Payload {
        msg: plaintext,
        aad: aad.as_slice(),
    };
    // Note: library error string is intentionally dropped from the
    // user-facing Error::Encryption (oracle hygiene per L12 review).
    // Internal callers map to Error::MnemonicCipher with their own
    // caller-specific message; the library detail leaks no path.
    let ciphertext = cipher
        .encrypt(nonce, payload)
        .map_err(|_| Error::Encryption("aes-gcm encrypt failed".into()))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt `blob` (format: `nonce || ciphertext || tag`) with `key`, verifying
/// `aad` matches the value bound at encrypt time. Returns plaintext wrapped
/// in `Secret<Vec<u8>>` per L15.
///
/// **AAD mismatch:** if the supplied `aad` does not match the encrypt-time AAD,
/// AES-GCM tag verification fails and `Error::Encryption` is returned. This is
/// the cryptographic-binding property used by `MnemonicCipherBlob` to defend
/// against cross-context ciphertext reuse (N5).
///
/// # Errors
///
/// Returns `Error::Encryption` on:
/// - key length mismatch (not 32 bytes)
/// - blob length < `NONCE_LEN + TAG_LEN` (structurally invalid)
/// - AEAD tag mismatch (wrong key, wrong AAD, tampered ciphertext, or tampered nonce)
pub fn decrypt(key: &Secret<Vec<u8>>, blob: &[u8], aad: Aad<'_>) -> Result<Secret<Vec<u8>>> {
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
    let payload = Payload {
        msg: ciphertext,
        aad: aad.as_slice(),
    };
    let plaintext = cipher
        .decrypt(nonce, payload)
        .map_err(|_| Error::Encryption("aes-gcm decrypt failed".into()))?;
    Ok(Secret::new(plaintext))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::aad::Aad;
    use crate::crypto::argon2;

    fn fixed_key() -> Secret<Vec<u8>> {
        // Deterministic 32-byte key for round-trip tests.
        Secret::new(vec![7u8; KEY_LEN])
    }

    #[test]
    fn roundtrip_recovers_plaintext() {
        let key = fixed_key();
        let pt = b"hello world";
        let ct = encrypt(&key, pt, Aad::NONE).expect("encrypt");
        let pt2 = decrypt(&key, &ct, Aad::NONE).expect("decrypt");
        assert_eq!(pt, pt2.expose().as_slice());
    }

    #[test]
    fn wrong_key_fails_decrypt() {
        let k1 = Secret::new(vec![7u8; KEY_LEN]);
        let k2 = Secret::new(vec![8u8; KEY_LEN]);
        let ct = encrypt(&k1, b"secret", Aad::NONE).expect("encrypt");
        let err = decrypt(&k2, &ct, Aad::NONE).expect_err("must reject wrong key");
        assert!(matches!(err, Error::Encryption(_)));
    }

    #[test]
    fn tampered_ciphertext_fails_decrypt() {
        let key = fixed_key();
        let ct = encrypt(&key, b"secret", Aad::NONE).expect("encrypt");
        let mut tampered = ct.clone();
        // Flip a byte in the ciphertext (skip the 12-byte nonce prefix).
        let idx = NONCE_LEN + (tampered.len() - NONCE_LEN) / 2;
        tampered[idx] ^= 0x01;
        let err = decrypt(&key, &tampered, Aad::NONE).expect_err("must reject tampered ct");
        assert!(matches!(err, Error::Encryption(_)));
    }

    #[test]
    fn tampered_nonce_fails_decrypt() {
        let key = fixed_key();
        let ct = encrypt(&key, b"secret", Aad::NONE).expect("encrypt");
        let mut tampered = ct.clone();
        tampered[0] ^= 0x01;
        let err = decrypt(&key, &tampered, Aad::NONE).expect_err("must reject tampered nonce");
        assert!(matches!(err, Error::Encryption(_)));
    }

    #[test]
    fn blob_too_short_rejected() {
        let key = fixed_key();
        // 5 bytes: less than NONCE_LEN(12) + TAG_LEN(16) = 28.
        let err = decrypt(&key, &[0u8; 5], Aad::NONE).expect_err("must reject short blob");
        assert!(matches!(err, Error::Encryption(_)));
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn blob_with_only_nonce_rejected() {
        let key = fixed_key();
        // 15 bytes: between NONCE_LEN(12) and NONCE+TAG(28). Still invalid.
        let err = decrypt(&key, &[0u8; NONCE_LEN + 3], Aad::NONE).expect_err("must reject");
        assert!(matches!(err, Error::Encryption(_)));
    }

    #[test]
    fn encrypt_rejects_wrong_length_key() {
        let bad_key = Secret::new(vec![1u8; 16]); // too short
        let err = encrypt(&bad_key, b"x", Aad::NONE).expect_err("must reject 16-byte key");
        assert!(matches!(err, Error::Encryption(_)));
        assert!(err.to_string().contains("AES-256"));
    }

    #[test]
    fn decrypt_rejects_wrong_length_key() {
        let bad_key = Secret::new(vec![1u8; 64]); // too long
        let ct = encrypt(&fixed_key(), b"x", Aad::NONE).expect("encrypt");
        let err = decrypt(&bad_key, &ct, Aad::NONE).expect_err("must reject 64-byte key");
        assert!(matches!(err, Error::Encryption(_)));
    }

    #[test]
    fn nonce_is_random_per_call() {
        // Two encrypts of the same plaintext yield different blobs (with
        // overwhelming probability; 96-bit random nonce).
        let key = fixed_key();
        let ct1 = encrypt(&key, b"same plaintext", Aad::NONE).expect("e1");
        let ct2 = encrypt(&key, b"same plaintext", Aad::NONE).expect("e2");
        assert_ne!(ct1, ct2);
        // But both decrypt to the same plaintext.
        assert_eq!(
            decrypt(&key, &ct1, Aad::NONE)
                .expect("decrypt ct1")
                .expose(),
            b"same plaintext"
        );
        assert_eq!(
            decrypt(&key, &ct2, Aad::NONE)
                .expect("decrypt ct2")
                .expose(),
            b"same plaintext"
        );
    }

    #[test]
    fn plaintext_zero_length_works() {
        let key = fixed_key();
        let ct = encrypt(&key, b"", Aad::NONE).expect("encrypt empty");
        let pt = decrypt(&key, &ct, Aad::NONE).expect("decrypt empty");
        assert!(pt.expose().is_empty());
    }

    #[test]
    fn decrypt_returns_secret_wrapper() {
        // Compile-time witness: Secret<Vec<u8>>: ZeroizeOnDrop.
        fn assert_zod<T: zeroize::ZeroizeOnDrop>() {}
        assert_zod::<Secret<Vec<u8>>>();
        let key = fixed_key();
        let ct = encrypt(&key, b"x", Aad::NONE).expect("e");
        let pt = decrypt(&key, &ct, Aad::NONE).expect("d");
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
        let ct = encrypt(&key, pt, Aad::NONE).expect("encrypt");
        let pt2 = decrypt(&key, &ct, Aad::NONE).expect("decrypt");
        assert_eq!(pt, pt2.expose().as_slice());
    }

    // --- AAD tests (Issue #66, ADR 0001) ---
    // The mnemonic_cipher wrapper has its own AAD tests; these verify
    // the primitive-level AAD contract that the wrapper depends on.

    fn multi_byte_aad() -> Vec<u8> {
        vec![0xAB; 32]
    }

    #[test]
    fn aad_roundtrip_with_multi_byte_aad_recovers_plaintext() {
        let key = fixed_key();
        let aad_bytes = multi_byte_aad();
        let aad = Aad::from_bytes(&aad_bytes).expect("within cap");
        let pt = b"secret data";
        let ct = encrypt(&key, pt, aad).expect("encrypt");
        let pt2 = decrypt(&key, &ct, aad).expect("decrypt");
        assert_eq!(pt, pt2.expose().as_slice());
    }

    #[test]
    fn aad_mismatch_fails_decrypt() {
        let key = fixed_key();
        let aad_a_bytes = multi_byte_aad();
        let aad_a = Aad::from_bytes(&aad_a_bytes).expect("within cap");
        let mut aad_b_bytes = multi_byte_aad();
        aad_b_bytes[0] ^= 0x01; // 1-byte diff
        let aad_b = Aad::from_bytes(&aad_b_bytes).expect("within cap");
        let ct = encrypt(&key, b"secret", aad_a).expect("encrypt");
        let err = decrypt(&key, &ct, aad_b).expect_err("must reject");
        assert!(matches!(err, Error::Encryption(_)));
    }

    #[test]
    fn empty_aad_blob_rejects_nonempty_aad_at_decrypt() {
        let key = fixed_key();
        let ct = encrypt(&key, b"x", Aad::NONE).expect("encrypt");
        let err = decrypt(&key, &ct, Aad::from_bytes(b"\x01").unwrap())
            .expect_err("AAD mismatch must reject");
        assert!(matches!(err, Error::Encryption(_)));
    }

    #[test]
    fn nonempty_aad_blob_rejects_empty_aad_at_decrypt() {
        let key = fixed_key();
        let aad = Aad::from_bytes(b"\x01").unwrap();
        let ct = encrypt(&key, b"x", aad).expect("encrypt");
        let err = decrypt(&key, &ct, Aad::NONE).expect_err("AAD mismatch must reject");
        assert!(matches!(err, Error::Encryption(_)));
    }

    #[test]
    fn aad_is_not_stored_in_blob() {
        // AAD is authenticated but not encrypted, AND not embedded — same
        // length regardless of AAD length. This is the witness for ADR 0001
        // §Cross-references (the AAD is caller-side context only).
        let key = fixed_key();
        let pt = b"hello world";
        let ct_no_aad = encrypt(&key, pt, Aad::NONE).expect("encrypt no-aad");
        let ct_with_aad = encrypt(&key, pt, Aad::from_bytes(&multi_byte_aad()).unwrap())
            .expect("encrypt with-aad");
        assert_eq!(ct_no_aad.len(), ct_with_aad.len());
    }
}
