//! `sol-wallet-core` — wallet file encryption (Argon2id + AES-256-GCM).
//!
//! Phase 6.1 Task 6.1 Steps 1 + 2. Per security audit
//! `docs/audit/2026-09-11-sol-wallet-core-phase6-security-review.md`
//! + L13 step 10 review (Sept 11):
//!
//! - **P6-1** — KDF params + salt + nonce bound in AES-GCM AAD
//!   (`"sol-wallet-core/v1" ‖ algorithm ‖ memory_kb_le ‖ iterations_le
//!   ‖ parallelism_le ‖ salt ‖ nonce`). Any metadata tamper fails
//!   `Error::WalletDecryptFailed`.
//! - **P6-4** — `encrypt_wallet` accepts `Zeroizing<Vec<u8>>`,
//!   `decrypt_wallet` returns `Zeroizing<Vec<u8>>`.
//! - **P6-5** — `KdfParams::default` is platform-conditional
//!   (`#[cfg(target_os = "ios"|"android")]` → 16 MB; otherwise 64 MB).
//! - **P6-7** — `OsRng` failure propagates as `Error::OsRngFailed`.
//!   No `unwrap()` / `expect()` on RNG paths.
//! - **P6-9** — Argon2id cost dominates; JSON-parse-fail vs
//!   Argon2id-fail timing differential ≤ Argon2id cost + 10 ms accepted.
//! - **P6-10** — `version: 1` discriminator; future AEAD / KDF
//!   migration gates via `Error::UnsupportedBlobVersion`.

use crate::{Error, Result};
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// Argon2id spec cap on memory_kb (~4 GB). Envelopes claiming more
/// fail `Error::InvalidKdfParams` (L13 step 10 Sept 11 — bounded
/// newtype, prevents attacker-supplied 4 TB envelope from OOMing
/// the wallet).
pub const KDF_MEMORY_KB_MAX: u32 = 4 * 1024 * 1024;

/// Argon2id parameters. Recorded in the JSON envelope and bound
/// into the AES-GCM AAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Memory cost in KB.
    pub memory_kb: u32,
    /// Iteration count.
    pub iterations: u32,
    /// Parallelism lanes.
    pub parallelism: u32,
}

impl KdfParams {
    /// Desktop default — 64 MB, t=3, p=1.
    pub const DESKTOP: Self = Self {
        memory_kb: 64 * 1024,
        iterations: 3,
        parallelism: 1,
    };

    /// Mobile default — 16 MB, t=3, p=1 (audit P6-5).
    pub const MOBILE: Self = Self {
        memory_kb: 16 * 1024,
        iterations: 3,
        parallelism: 1,
    };
}

impl Default for KdfParams {
    fn default() -> Self {
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            KdfParams::MOBILE
        }
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            KdfParams::DESKTOP
        }
    }
}

impl KdfParams {
    /// Validate the parameter shape (Argon2 spec + our memory cap).
    /// Returns `Err(InvalidKdfParams)` on shape failure.
    fn validate(&self) -> Result<()> {
        if self.memory_kb == 0
            || self.memory_kb > KDF_MEMORY_KB_MAX
            || self.iterations == 0
            || self.parallelism == 0
            || self.memory_kb < 8 * self.parallelism
        {
            return Err(Error::InvalidKdfParams {
                memory_kb: self.memory_kb,
                iterations: self.iterations,
                parallelism: self.parallelism,
            });
        }
        Ok(())
    }

    fn argon2_params(&self) -> Result<Params> {
        // `Params::new` may fail only on shape; both `DESKTOP` and
        // `MOBILE` are pre-validated. The validate() call above
        // covers the public envelope path; this is defense-in-depth.
        Params::new(self.memory_kb, self.iterations, self.parallelism, None).map_err(|_| {
            Error::InvalidKdfParams {
                memory_kb: self.memory_kb,
                iterations: self.iterations,
                parallelism: self.parallelism,
            }
        })
    }
}

/// Current envelope version. Bumped on AEAD / KDF changes.
pub const ENVELOPE_VERSION: u32 = 1;

/// JSON envelope written to disk. Per L13 step 10 Sept 11, the
/// nested `KdfBlock` / `CipherBlock` structs carry only one or two
/// fields each — inlined into `EncryptedBlob` so callers don't have
/// to thread through both blocks. Re-introduce when a second cipher
/// lands in V0.1.5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedBlob {
    /// Envelope version.
    pub version: u32,
    /// KDF algorithm (always `"argon2id"` in V0.1; reserved for
    /// future migration per L13 review).
    pub kdf_algorithm: String,
    /// Memory cost (KB).
    pub kdf_memory_kb: u32,
    /// Iterations.
    pub kdf_iterations: u32,
    /// Parallelism.
    pub kdf_parallelism: u32,
    /// Salt bytes (hex).
    pub kdf_salt: String,
    /// Cipher algorithm (always `"aes-256-gcm"` in V0.1).
    pub cipher_algorithm: String,
    /// Nonce bytes (hex).
    pub cipher_nonce: String,
    /// Base64 ciphertext + auth tag. Field name on the wire is
    /// `encrypted_payload` per L13 review Sept 11 (drop the
    /// `_b64` Rust suffix).
    #[serde(rename = "encrypted_payload")]
    pub encrypted_payload: String,
}

/// AAD prefix — domain separator that BINDs ciphertext to the
/// `sol-wallet-core/v1` envelope. Any change to this string is a
/// breaking change to the envelope.
const AAD_PREFIX: &[u8] = b"sol-wallet-core/v1";

/// Encrypt `plaintext` under `password` using Argon2id + AES-256-GCM.
/// Returns the JSON envelope ready to write via `persist::atomic_write`.
pub fn encrypt_wallet(plaintext: Zeroizing<Vec<u8>>, password: &str) -> Result<EncryptedBlob> {
    let params = KdfParams::default();
    params.validate()?;

    // Salt — 16 bytes from OsRng.
    let mut salt_arr = Zeroizing::new([0u8; 16]);
    getrandom::getrandom(&mut *salt_arr).map_err(|source| Error::OsRngFailed { source })?;
    let salt: &[u8] = salt_arr.as_ref();

    // Nonce — 12 bytes from OsRng.
    let mut nonce_arr = [0u8; 12];
    getrandom::getrandom(&mut nonce_arr).map_err(|source| Error::OsRngFailed { source })?;
    let nonce: &[u8] = &nonce_arr;

    // Derive 32-byte key via Argon2id.
    let key = derive_key(password.as_bytes(), salt, params)?;

    let cipher = Aes256Gcm::new_from_slice(&*key).map_err(|_| Error::CipherInit)?;

    // Build AAD.
    let aad = build_aad(params, salt, nonce);

    // Encrypt.
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: &plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| Error::CipherInit)?;

    Ok(EncryptedBlob {
        version: ENVELOPE_VERSION,
        kdf_algorithm: "argon2id".to_string(),
        kdf_memory_kb: params.memory_kb,
        kdf_iterations: params.iterations,
        kdf_parallelism: params.parallelism,
        kdf_salt: hex::encode(salt),
        cipher_algorithm: "aes-256-gcm".to_string(),
        cipher_nonce: hex::encode(nonce),
        encrypted_payload: B64.encode(&ciphertext),
    })
}

/// Decrypt `blob` under `password`. Returns plaintext wrapped in
/// `Zeroizing<Vec<u8>>`. Every internal failure wraps as
/// `Error::WalletDecryptFailed` (audit P6-9 timing-differential accepted).
pub fn decrypt_wallet(blob: &EncryptedBlob, password: &str) -> Result<Zeroizing<Vec<u8>>> {
    if blob.version != ENVELOPE_VERSION {
        return Err(Error::UnsupportedBlobVersion {
            found: blob.version,
            lo: ENVELOPE_VERSION,
            hi: ENVELOPE_VERSION,
        });
    }

    let salt_bytes = hex::decode(&blob.kdf_salt).map_err(|_| decrypt_err())?;
    let nonce_bytes = hex::decode(&blob.cipher_nonce).map_err(|_| decrypt_err())?;
    let ciphertext = B64
        .decode(&blob.encrypted_payload)
        .map_err(|_| decrypt_err())?;

    let salt_arr: [u8; 16] = salt_bytes
        .as_slice()
        .try_into()
        .map_err(|_| decrypt_err())?;
    let salt = Zeroizing::new(salt_arr);

    let nonce_arr: [u8; 12] = nonce_bytes
        .as_slice()
        .try_into()
        .map_err(|_| decrypt_err())?;

    let params = KdfParams {
        memory_kb: blob.kdf_memory_kb,
        iterations: blob.kdf_iterations,
        parallelism: blob.kdf_parallelism,
    };
    // L13 review Sept 11 — bounded KDF params before KDF attempt.
    params.validate().map_err(|_| decrypt_err())?;
    let key = derive_key(password.as_bytes(), salt.as_ref(), params).map_err(|_| decrypt_err())?;
    let cipher = Aes256Gcm::new_from_slice(&*key).map_err(|_| decrypt_err())?;

    // Reconstruct AAD from envelope — tamper-detected via auth-tag mismatch.
    let aad = build_aad(params, salt.as_ref(), &nonce_arr);

    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce_arr),
            Payload {
                msg: &ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| decrypt_err())?;

    Ok(Zeroizing::new(plaintext))
}

fn decrypt_err() -> Error {
    Error::WalletDecryptFailed {
        id: crate::WalletId(uuid::Uuid::nil()),
    }
}

fn derive_key(password: &[u8], salt: &[u8], params: KdfParams) -> Result<Zeroizing<[u8; 32]>> {
    let a2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params.argon2_params()?);
    let mut out = Zeroizing::new([0u8; 32]);
    a2.hash_password_into(password, salt, &mut *out)
        .map_err(|e| Error::KdfFailed {
            message: e.to_string(),
        })?;
    Ok(out)
}

fn build_aad(params: KdfParams, salt: &[u8], nonce: &[u8]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(AAD_PREFIX.len() + 9 + 12 + salt.len() + nonce.len());
    aad.extend_from_slice(AAD_PREFIX);
    aad.extend_from_slice(b"argon2id");
    aad.extend_from_slice(&params.memory_kb.to_le_bytes());
    aad.extend_from_slice(&params.iterations.to_le_bytes());
    aad.extend_from_slice(&params.parallelism.to_le_bytes());
    aad.extend_from_slice(salt);
    aad.extend_from_slice(nonce);
    aad
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kdf_params_default_matches_platform() {
        let p = KdfParams::default();
        #[cfg(any(target_os = "ios", target_os = "android"))]
        assert_eq!(p, KdfParams::MOBILE);
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        assert_eq!(p, KdfParams::DESKTOP);
    }

    #[test]
    fn kdf_params_reject_oversized_memory() {
        let p = KdfParams {
            memory_kb: KDF_MEMORY_KB_MAX + 1,
            iterations: 3,
            parallelism: 1,
        };
        assert!(matches!(p.validate(), Err(Error::InvalidKdfParams { .. })));
    }
}
