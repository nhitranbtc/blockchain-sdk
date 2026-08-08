//! Argon2id password-based KDF.
//!
//! Per F5: m=256 MiB, t=10, p=4 (Sparrow reference; ~500ms wall-clock target).
//!
//! **Drift from plan §Task 5:**
//!
//! | Plan said | This impl | Why |
//! |---|---|---|
//! | `derive_key -> [u8; 32]` (Copy) | `derive_key -> Secret<Vec<u8>>` | L15: Copy defeats zeroize-on-drop |
//! | `encrypt(key: &[u8; 32], ...)` (in aes_gcm) | `encrypt(key: &Secret<Vec<u8>>, ...)` | L15: caller zeroizes on drop |
//! | `password: &[u8]` is unreadable | `password: &[u8]` with caller-Secret contract | Symmetric with L15 — caller is responsible for wrapping the password in `Secret` before passing. The borrowed `&[u8]` is a view into the caller's `Secret<Vec<u8>>` whose ZeroizeOnDrop fires on drop. Encrypting low-entropy caller-buffers (e.g. `b"literal"`) is unsafe but the same risk applies to all Rust APIs that take `&[u8]`. |
//!
//! **Defends against:** A1 (offline cracker of stolen ciphertext via strong KDF).
//! **Does NOT defend:** T1 (physical seizure — full-disk encryption out of scope).
//! **Deferred:** mlock of secret pages (Task 1.5 deferred to v0.2 per threat model).
//!
//! **Calibration note:** m=256 MiB × t=10 × p=4 is the Sparrow reference
//! calibration. Lower devices may need t_cost reduction (knob for v0.1.1).

use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroize;

use crate::error::{Error, Result};
use crate::keys::Secret;

/// Argon2id memory cost in KiB (256 MiB). Per F5.
pub const ARGON2_M_COST_KIB: u32 = 256 * 1024;

/// Argon2id time cost (iterations). Per F5.
pub const ARGON2_T_COST: u32 = 10;

/// Argon2id parallelism lanes. Per F5.
pub const ARGON2_P_COST: u32 = 4;

/// Compile-time bounds check on the Argon2id parameters. If any
/// constant is changed to an out-of-range value, the build fails
/// here instead of failing at runtime inside `Params::new`.
const _: () = {
    assert!(ARGON2_M_COST_KIB > 0, "argon2 m_cost must be > 0");
    assert!(ARGON2_T_COST >= 1, "argon2 t_cost must be >= 1");
    assert!(ARGON2_P_COST >= 1, "argon2 p_cost must be >= 1");
    assert!(DERIVED_KEY_LEN >= 4, "argon2 output_len must be >= 4");
};

/// Required salt length in bytes. Enforced at API boundary.
pub const SALT_LEN: usize = 16;

/// AES-256 key length derived by Argon2id.
pub const DERIVED_KEY_LEN: usize = 32;

/// Derive a 32-byte AES key from password + salt via Argon2id.
///
/// Returns `Secret<Vec<u8>>` (heap-allocated, zeroize-on-drop) per L15
/// — `[u8; 32]` is Copy and would defeat the zeroize-on-drop guarantee.
///
/// **Salt must be exactly [`SALT_LEN`] bytes** — shorter or longer salts
/// are rejected with `Error::Encryption` to prevent silent
/// compatibility surprises. Use `random_salt()` to generate a salt.
///
/// **Caller contract:** the `password` slice lives in caller-owned memory.
/// If the caller wants it zeroized after use, wrap in `Secret<Vec<u8>>`
/// and pass `secret.expose().as_slice()`. The `Secret`'s
/// `ZeroizeOnDrop` fires when the caller drops it.
pub fn derive_key(password: &[u8], salt: &[u8]) -> Result<Secret<Vec<u8>>> {
    if salt.len() != SALT_LEN {
        return Err(Error::Encryption(format!(
            "salt must be exactly {SALT_LEN} bytes, got {}",
            salt.len()
        )));
    }
    // Constants are vetted at compile time by the const _ assert above.
    // Params::new error path is unreachable in production but the `?`
    // stays defensive.
    let params = Params::new(
        ARGON2_M_COST_KIB,
        ARGON2_T_COST,
        ARGON2_P_COST,
        Some(DERIVED_KEY_LEN),
    )
    .map_err(|e| Error::Encryption(format!("argon2 params: {e}")))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key_arr = [0u8; DERIVED_KEY_LEN];
    argon
        .hash_password_into(password, salt, &mut key_arr)
        .map_err(|e| Error::Encryption(format!("argon2 hash: {e}")))?;
    let secret = Secret::new(key_arr.to_vec());
    key_arr.zeroize();
    Ok(secret)
}

/// Generate a random 16-byte salt using the OS CSPRNG (`OsRng`).
///
/// Use this rather than `rand::thread_rng()` — `OsRng` is guaranteed
/// to be a CSPRNG; `thread_rng` users would need to opt in to the
/// `getrandom` backend. Centralizing the choice here means no caller
/// can accidentally use a non-CSPRNG for the salt.
pub fn random_salt() -> [u8; SALT_LEN] {
    use rand::RngCore;
    let mut salt = [0u8; SALT_LEN];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    salt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_key_produces_32_bytes() {
        let salt = [0u8; SALT_LEN];
        let key = derive_key(b"password", &salt).expect("derive");
        assert_eq!(key.expose().len(), DERIVED_KEY_LEN);
    }

    #[test]
    fn derive_key_deterministic_for_same_inputs() {
        let salt = [0u8; SALT_LEN];
        let k1 = derive_key(b"password", &salt).expect("k1");
        let k2 = derive_key(b"password", &salt).expect("k2");
        assert_eq!(k1.expose(), k2.expose());
    }

    #[test]
    fn derive_key_different_salt_yields_different_key() {
        let k1 = derive_key(b"password", &[0u8; SALT_LEN]).expect("k1");
        let k2 = derive_key(b"password", &[1u8; SALT_LEN]).expect("k2");
        assert_ne!(k1.expose(), k2.expose());
    }

    #[test]
    fn derive_key_different_password_yields_different_key() {
        let salt = [0u8; SALT_LEN];
        let k1 = derive_key(b"alpha", &salt).expect("k1");
        let k2 = derive_key(b"bravo", &salt).expect("k2");
        assert_ne!(k1.expose(), k2.expose());
    }

    #[test]
    fn derive_key_rejects_short_salt() {
        let bad_salt = [0u8; SALT_LEN - 1];
        let err = derive_key(b"password", &bad_salt).expect_err("must reject short salt");
        assert!(matches!(err, Error::Encryption(_)));
        assert!(err.to_string().contains("salt"));
    }

    #[test]
    fn derive_key_rejects_long_salt() {
        let bad_salt = [0u8; SALT_LEN + 1];
        let err = derive_key(b"password", &bad_salt).expect_err("must reject long salt");
        assert!(matches!(err, Error::Encryption(_)));
        assert!(err.to_string().contains("salt"));
    }

    #[test]
    fn derive_key_with_empty_password_succeeds() {
        // Argon2id accepts an empty password as valid input. Without
        // this test, regressions in the underlying crate (panic, error,
        // or silent acceptance) go undetected.
        let salt = [0u8; SALT_LEN];
        let key = derive_key(b"", &salt).expect("empty password valid");
        assert_eq!(key.expose().len(), DERIVED_KEY_LEN);
    }

    #[test]
    fn derive_key_returns_secret_wrapper() {
        // Compile-time witness: Secret<Vec<u8>>: ZeroizeOnDrop.
        fn assert_zod<T: zeroize::ZeroizeOnDrop>() {}
        assert_zod::<Secret<Vec<u8>>>();
        let salt = [0u8; SALT_LEN];
        let key = derive_key(b"password", &salt).expect("derive");
        assert_zod::<Secret<Vec<u8>>>();
        // Key compiles as Secret<Vec<u8>>, not as [u8; 32].
        let _: &Secret<Vec<u8>> = &key;
    }

    #[test]
    fn random_salt_returns_16_bytes() {
        let salt = random_salt();
        assert_eq!(salt.len(), SALT_LEN);
    }

    #[test]
    fn random_salt_yields_unique_values() {
        // Probabilistic — birthday collision at ~2^64 calls. For 2 calls
        // the collision probability is 2^-128, dwarfed by test noise.
        let s1 = random_salt();
        let s2 = random_salt();
        assert_ne!(s1, s2);
    }
}
