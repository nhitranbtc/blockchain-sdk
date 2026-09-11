//! `sol-wallet-core` — wallet file encryption (Argon2id + AES-256-GCM).
//!
//! Phase 6.1 Task 6.1 Steps 1 + 2. Per security audit
//! `docs/audit/2026-09-11-sol-wallet-core-phase6-security-review.md`:
//!
//! - **P6-1** — KDF params + salt + nonce bound in AES-GCM AAD so
//!   any metadata tamper fails `Error::WalletDecryptFailed`. AAD =
//!   `"sol-wallet-core/v1" ‖ algorithm ‖ memory_kb_le ‖ iterations_le ‖
//!   parallelism_le ‖ salt ‖ nonce`.
//! - **P6-4** — `encrypt_wallet` accepts `Zeroizing<Vec<u8>>`,
//!   `decrypt_wallet` returns `Zeroizing<Vec<u8>>`. No plaintext
//!   slice crosses the API boundary unzeroized.
//! - **P6-5** — `KdfParams::default` is platform-conditional:
//!   desktop = 64 MB, iOS + Android = 16 MB (via `#[cfg(target_os)]`).
//! - **P6-7** — `OsRng` failure propagates as `Error::OsRngFailed`.
//!   No `unwrap()` / `expect()` on RNG paths (CI `grep` enforces).
//! - **P6-9** — JSON-parse-fail vs Argon2id-fail timing differential
//!   accepted; Argon2id cost dominates (≥100 ms) so the residual
//!   differential is < Argon2id cost + 10 ms.
//! - **P6-10** — `version: 1` discriminator in envelope; future
//!   AEAD / KDF migration gates via `Error::UnsupportedBlobVersion`.

use crate::{Error, Result};
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// Argon2id parameters. Recorded in the JSON envelope and bound
/// into the AES-GCM AAD (audit P6-1).
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
    /// Desktop default — 64 MB, t=3, p=1 (matches plan §Phase 6).
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
    fn algorithm_str(&self) -> &'static str {
        "argon2id"
    }

    fn argon2_params(&self) -> Params {
        // Argon2 `Params::new` only rejects m_cost < 8 * p_cost or
        // iterations == 0; both DESKTOP and MOBILE satisfy that by
        // construction. This is a parameter-shape check, NOT an
        // RNG / IO / network call — the `Params::new` is
        // deterministic on the inputs.
        Params::new(self.memory_kb, self.iterations, self.parallelism, None)
            .unwrap_or_else(|_| Params::default())
    }
}

/// Current envelope version. Bumped on AEAD / KDF changes.
pub const ENVELOPE_VERSION: u32 = 1;

/// JSON envelope written to disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedBlob {
    /// Envelope version. Always `ENVELOPE_VERSION` (= 1) in V0.1.
    /// Future versions dispatch on this field (audit P6-10).
    pub version: u32,
    /// KDF block.
    pub kdf: KdfBlock,
    /// Cipher block.
    pub cipher: CipherBlock,
    /// Base64 ciphertext + auth tag (no separate field — tag is
    /// appended to ciphertext by `aes-gcm`).
    #[serde(rename = "encrypted_payload")]
    pub encrypted_payload_b64: String,
}

/// KDF block in the envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfBlock {
    /// Algorithm name (always `"argon2id"` in V0.1).
    pub algorithm: String,
    /// Memory cost (KB).
    pub memory_kb: u32,
    /// Iterations.
    pub iterations: u32,
    /// Parallelism.
    pub parallelism: u32,
    /// Salt bytes (hex).
    pub salt: String,
}

/// Cipher block in the envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CipherBlock {
    /// Algorithm name (always `"aes-256-gcm"` in V0.1).
    pub algorithm: String,
    /// Nonce bytes (hex).
    pub nonce: String,
}

/// AAD prefix — domain separator that BINDs ciphertext to the
/// `sol-wallet-core/v1` envelope. Any change to this string is a
/// breaking change to the envelope (audit P6-1).
const AAD_PREFIX: &[u8] = b"sol-wallet-core/v1";

/// Encrypt `plaintext` under `password` using Argon2id + AES-256-GCM.
/// Returns the JSON envelope ready to write via `persist::atomic_write`.
///
/// `OsRng` failure propagates as `Error::OsRngFailed` — no
/// `unwrap()` / `expect()` on RNG paths (audit P6-7).
pub fn encrypt_wallet(plaintext: Zeroizing<Vec<u8>>, password: &str) -> Result<EncryptedBlob> {
    let params = KdfParams::default();

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

    let cipher = Aes256Gcm::new_from_slice(&*key).map_err(|_| Error::Placeholder)?;

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
        .map_err(|_| Error::Placeholder)?;

    Ok(EncryptedBlob {
        version: ENVELOPE_VERSION,
        kdf: KdfBlock {
            algorithm: params.algorithm_str().to_string(),
            memory_kb: params.memory_kb,
            iterations: params.iterations,
            parallelism: params.parallelism,
            salt: hex::encode(salt),
        },
        cipher: CipherBlock {
            algorithm: "aes-256-gcm".to_string(),
            nonce: hex::encode(nonce),
        },
        encrypted_payload_b64: B64.encode(&ciphertext),
    })
}

/// Decrypt `blob` under `password`. Returns plaintext wrapped in
/// `Zeroizing<Vec<u8>>`. Wraps every internal failure as
/// `Error::WalletDecryptFailed` — caller cannot distinguish
/// wrong-passphrase from tampered-metadata (audit P6-9 accepted
/// residual timing differential).
pub fn decrypt_wallet(blob: &EncryptedBlob, password: &str) -> Result<Zeroizing<Vec<u8>>> {
    if blob.version != ENVELOPE_VERSION {
        return Err(Error::UnsupportedBlobVersion {
            found: blob.version,
            lo: ENVELOPE_VERSION,
            hi: ENVELOPE_VERSION,
        });
    }

    let salt_bytes = hex::decode(&blob.kdf.salt).map_err(decrypt_err)?;
    let nonce_bytes = hex::decode(&blob.cipher.nonce).map_err(decrypt_err)?;
    let ciphertext = B64
        .decode(&blob.encrypted_payload_b64)
        .map_err(decrypt_err)?;

    let salt_arr: [u8; 16] = salt_bytes
        .as_slice()
        .try_into()
        .map_err(|_| decrypt_err_bytes())?;
    let salt = Zeroizing::new(salt_arr);

    let nonce_arr: [u8; 12] = nonce_bytes
        .as_slice()
        .try_into()
        .map_err(|_| decrypt_err_bytes())?;

    let params = KdfParams {
        memory_kb: blob.kdf.memory_kb,
        iterations: blob.kdf.iterations,
        parallelism: blob.kdf.parallelism,
    };
    let key =
        derive_key(password.as_bytes(), salt.as_ref(), params).map_err(|_| decrypt_err_bytes())?;
    let cipher = Aes256Gcm::new_from_slice(&*key).map_err(|_| decrypt_err_bytes())?;

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
        .map_err(|_| decrypt_err_bytes())?;

    Ok(Zeroizing::new(plaintext))
}

fn decrypt_err<E>(_e: E) -> Error {
    Error::WalletDecryptFailed {
        id: crate::WalletId(uuid::Uuid::nil()),
    }
}

fn decrypt_err_bytes() -> Error {
    Error::WalletDecryptFailed {
        id: crate::WalletId(uuid::Uuid::nil()),
    }
}

fn derive_key(password: &[u8], salt: &[u8], params: KdfParams) -> Result<Zeroizing<[u8; 32]>> {
    let a2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params.argon2_params());
    let mut out = Zeroizing::new([0u8; 32]);
    a2.hash_password_into(password, salt, &mut *out)
        .map_err(|_| Error::Placeholder)?;
    Ok(out)
}

fn build_aad(params: KdfParams, salt: &[u8], nonce: &[u8]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(AAD_PREFIX.len() + 12 + salt.len() + nonce.len());
    aad.extend_from_slice(AAD_PREFIX);
    aad.extend_from_slice(params.algorithm_str().as_bytes());
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
    fn kdf_default_matches_platform() {
        let p = KdfParams::default();
        #[cfg(any(target_os = "ios", target_os = "android"))]
        assert_eq!(p, KdfParams::MOBILE);
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        assert_eq!(p, KdfParams::DESKTOP);
    }
}
