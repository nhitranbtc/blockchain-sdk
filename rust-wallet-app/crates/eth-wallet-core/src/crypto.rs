//! Argon2id KDF + AES-256-GCM encryption for mnemonic-at-rest.
//!
//! Mirrors BTC F5/F6 precedent (`bitcoin-wallet-core/src/crypto/argon2.rs` +
//! `aes_gcm.rs`). Per #297 B2: ships encryption from day 1 in v0.2 (no
//! plaintext fallback). Per F47: secrets stay in `Zeroizing<Vec<u8>>`.
//!
//! Issue #301 (Task 2) + Issue #297 B2 + #303 (Task 4) sized.
//!
//! **Defends against:** offline cracker of stolen ciphertext (Argon2id cost).
//! **Does NOT defend:** physical seizure (full-disk encryption out of scope).

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use thiserror::Error;
use zeroize::Zeroize;

/// Argon2id memory cost in KiB (256 MiB). Per F5.
pub const ARGON2_M_COST_KIB: u32 = {
    const INNER: u32 = 256 * 1024;
    assert!(INNER == 256 * 1024, "F5: 256 MiB Argon2id memory cost");
    INNER
};

/// Argon2id time cost (iterations). Per F5.
pub const ARGON2_T_COST: u32 = {
    const INNER: u32 = 10;
    assert!(INNER == 10, "F5: t_cost=10 Argon2id iterations");
    INNER
};

/// Argon2id parallelism lanes. Per F5.
pub const ARGON2_P_COST: u32 = {
    const INNER: u32 = 4;
    assert!(INNER == 4, "F5: p_cost=4 Argon2id parallelism");
    INNER
};

/// Salt length in bytes. Per F5.
pub const SALT_LEN: usize = {
    const INNER: usize = 16;
    assert!(INNER == 16, "F5: 16-byte Argon2id salt");
    INNER
};

/// AES-GCM nonce length. NIST SP 800-38D §5.2.1.1 standard length.
pub const NONCE_LEN: usize = {
    const INNER: u32 = 12;
    assert!(INNER == 12, "AES-GCM 96-bit nonce");
    12
};

/// AES-256 key length (32 bytes). Per FIPS 197.
pub const KEY_LEN: usize = 32;

/// Encryption error type for the crypto module — Task 4 expands into a
/// 17-variant Error enum, but for now the 3 narrow variants are sufficient.
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("salt must be exactly {SALT_LEN} bytes, got {0}")]
    BadSaltLen(usize),
    #[error("argon2 KDF error: {0}")]
    Argon2(String),
    #[error("AES-GCM error: {0}")]
    AesGcm(String),
}

pub type Result<T> = std::result::Result<T, CryptoError>;

/// Derive a 32-byte AES key from `password` + `salt` via Argon2id.
/// Returns `Zeroizing<Vec<u8>>` per F47 (heap-allocated, zeroize-on-drop).
///
/// Salt must be exactly `SALT_LEN` bytes. Use `random_salt()` to generate.
pub fn derive_key(password: &[u8], salt: &[u8]) -> Result<zeroize::Zeroizing<Vec<u8>>> {
    if salt.len() != SALT_LEN {
        return Err(CryptoError::BadSaltLen(salt.len()));
    }
    let params = Params::new(
        ARGON2_M_COST_KIB,
        ARGON2_T_COST,
        ARGON2_P_COST,
        Some(KEY_LEN),
    )
    .map_err(|e| CryptoError::Argon2(format!("params: {e}")))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key_arr = [0u8; KEY_LEN];
    argon
        .hash_password_into(password, salt, &mut key_arr)
        .map_err(|e| CryptoError::Argon2(format!("hash: {e}")))?;
    let secret = zeroize::Zeroizing::new(key_arr.to_vec());
    key_arr.zeroize();
    Ok(secret)
}

/// Generate a random 16-byte salt via OS CSPRNG.
pub fn random_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    salt
}

/// Generate a random 12-byte AES-GCM nonce via OS CSPRNG.
/// (NIST SP 800-38D §5.2.1.1: 96-bit nonce MUST be uniformly random — never
/// reused under the same key.)
pub fn random_nonce() -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    nonce
}

/// Encrypt `plaintext` under `key` with the given `nonce` (must be 12 bytes
/// from `random_nonce()`). Returns AES-GCM ciphertext (nonce NOT included;
/// caller's responsibility to persist the nonce alongside).
pub fn encrypt(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| CryptoError::AesGcm(format!("init: {e}")))?;
    let nonce_arr = Nonce::from_slice(nonce);
    cipher
        .encrypt(nonce_arr, plaintext)
        .map_err(|e| CryptoError::AesGcm(format!("encrypt: {e}")))
}

/// Decrypt `ciphertext` under `key` with `nonce`. Returns the original
/// plaintext; caller's responsibility to wrap in Zeroizing on use.
pub fn decrypt(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], ciphertext: &[u8]) -> Result<Vec<u8>> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| CryptoError::AesGcm(format!("init: {e}")))?;
    let nonce_arr = Nonce::from_slice(nonce);
    cipher
        .decrypt(nonce_arr, ciphertext)
        .map_err(|e| CryptoError::AesGcm(format!("decrypt: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_key_returns_32_bytes() {
        let salt = [0u8; SALT_LEN];
        let key = derive_key(b"password", &salt).expect("derive");
        assert_eq!(key.len(), KEY_LEN);
    }

    #[test]
    fn derive_key_deterministic_for_same_inputs() {
        let salt = [42u8; SALT_LEN];
        let k1 = derive_key(b"hunter2", &salt).expect("k1");
        let k2 = derive_key(b"hunter2", &salt).expect("k2");
        assert_eq!(&*k1, &*k2);
    }

    #[test]
    fn derive_key_different_salt_yields_different_key() {
        let s1 = [0u8; SALT_LEN];
        let s2 = [1u8; SALT_LEN];
        let k1 = derive_key(b"hunter2", &s1).expect("k1");
        let k2 = derive_key(b"hunter2", &s2).expect("k2");
        assert_ne!(&*k1, &*k2);
    }

    #[test]
    fn derive_key_rejects_short_salt() {
        let bad = [0u8; SALT_LEN - 1];
        assert!(matches!(
            derive_key(b"x", &bad),
            Err(CryptoError::BadSaltLen(_))
        ));
    }

    #[test]
    fn derive_key_rejects_long_salt() {
        let bad = [0u8; SALT_LEN + 1];
        assert!(matches!(
            derive_key(b"x", &bad),
            Err(CryptoError::BadSaltLen(_))
        ));
    }

    #[test]
    fn random_salt_yields_16_bytes() {
        assert_eq!(random_salt().len(), SALT_LEN);
    }

    #[test]
    fn random_nonce_yields_12_bytes() {
        assert_eq!(random_nonce().len(), NONCE_LEN);
    }

    #[test]
    fn round_trip_encrypt_decrypt() {
        let salt = random_salt();
        let nonce = random_nonce();
        let key = derive_key(b"wallet-password", &salt).expect("key");
        let key_arr: [u8; KEY_LEN] = key[..].try_into().expect("KEY_LEN");
        let plaintext = b"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        let ct = encrypt(&key_arr, &nonce, plaintext).expect("encrypt");
        assert_ne!(&ct[..], &plaintext[..]);
        let pt = decrypt(&key_arr, &nonce, &ct).expect("decrypt");
        assert_eq!(pt, plaintext);
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let salt = random_salt();
        let nonce = random_nonce();
        let key1 = derive_key(b"key1", &salt).expect("k1");
        let key2 = derive_key(b"key2", &salt).expect("k2");
        let key1_arr: [u8; KEY_LEN] = key1[..].try_into().expect("len1");
        let key2_arr: [u8; KEY_LEN] = key2[..].try_into().expect("len2");

        let ct = encrypt(&key1_arr, &nonce, b"secret").expect("encrypt");
        assert!(
            decrypt(&key2_arr, &nonce, &ct).is_err(),
            "AES-GCM auth tag MUST reject wrong key"
        );
    }
}
