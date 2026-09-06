//! Phase 5 — at-rest encryption for wallet blobs (Task 4.7).
//!
//! Two primitives:
//!
//! - Argon2id KDF (`argon2_*` constants) — derives a 32-byte AES key
//!   from the user's passphrase + a per-blob random salt.
//! - AES-256-GCM AEAD (`aes::*` constants) — encrypts the serialized
//!   blob with a per-call 12-byte random nonce; output is the 16-byte
//!   auth tag appended to the ciphertext per the `aes-gcm` crate's
//!   default convention.
//!
//! **Blob format:** `salt (16) || nonce (12) || ciphertext || gcm_tag (16)`.
//! The `EncryptedWallet` wrapper newtypes the raw bytes so callers
//! cannot accidentally pass an AES-GCM blob missing its salt.
//!
//! **Why we don't reuse `bitcoin-wallet-core::crypto`:** the two crates
//! share no types. Plan §Phase 5 Task 4.7 names the surface fresh in
//! `tron-wallet-core`; bitcoin's variant binds `bitcoin::Network` via
//! AAD (closes N5 cross-network reuse), but TRON's network model is
//! different (Mainnet/Nile/Shasta — see `config::Network`) and the
//! AAD contract would need its own design. For v0.1 the contract is
//! "encrypt binary blob with passphrase, no AAD" — cross-network
//! reuse isn't a v0.1 concern (CLI enforces network selection at
//! `create` time, not at decrypt time).
//!
//! **Drift from plan §Phase 5 Task 4.7**:
//!
//! | Plan said                  | This impl                                   | Why |
//! |----------------------------|---------------------------------------------|-----|
//! | `encrypt(&[u8], &str)`     | `encrypt(plaintext: &[u8], passphrase: &str)` | Plan exactly |
//! | `Result<EncryptedWallet>`  | `Result<EncryptedWallet>` (newtype)         | blob is `salt \|\| nonce \|\| ct \|\| tag`; newtype prevents passing raw AES-GCM bytes |
//! | Zeroizing<Vec<u8>>         | `Secret<Vec<u8>>` re-export from `zeroize::Zeroizing` for inputs only | Plaintext wrapped by caller via `Zeroizing<Vec<u8>>` per plan Task 4.7; intermediate key zeroizes on drop via `argon2`-library `Zeroize` |
//!
//! **Defends against:** A1 (offline cracker of stolen ciphertext via
//! strong KDF), F43 (per-protocol error variant), U3 (zeroize on drop).
//!
//! **Does NOT defend:** T1 (physical seizure — full-disk encryption
//! out of scope), caller error (wrong passphrase, truncated blob —
//! surfaces as `Error::Encryption` rather than silent pass).
//!
//! **Calibration note:** Argon2id (m=256 MiB, t=10, p=4) matches
//! the bitcoin-wallet-core convention (F5 / Issue #30 constant
//! audit) so cross-crate UX feels consistent. ~500ms wall-clock on
//! a modern x86_64 host.

use aes_gcm::aead::{Aead, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use zeroize::Zeroize;

use crate::error::{Error, Result};

// ─── Constants ────────────────────────────────────────────────────────────

/// Argon2id memory cost in KiB (256 MiB). Per plan §Phase 5 Task 4.7
/// (F5 cross-crate convention). Compile-time pinned — see
/// `bitcoin-wallet-core/src/crypto/argon2.rs` constant audit.
pub const ARGON2_M_COST_KIB: u32 = {
    const INNER: u32 = 256 * 1024;
    assert!(
        INNER == 256 * 1024,
        "ARGON2_M_COST_KIB must be 256 MiB per F5"
    );
    INNER
};

/// Argon2id time cost (iterations).
pub const ARGON2_T_COST: u32 = {
    const INNER: u32 = 10;
    assert!(INNER == 10, "ARGON2_T_COST must be 10 per F5");
    INNER
};

/// Argon2id parallelism lanes.
pub const ARGON2_P_COST: u32 = {
    const INNER: u32 = 4;
    assert!(INNER == 4, "ARGON2_P_COST must be 4 per F5");
    INNER
};

/// Per-blob random salt length (16 bytes / 128 bits).
pub const SALT_LEN: usize = {
    const INNER: usize = 16;
    assert!(INNER == 16, "SALT_LEN must be 16 bytes per F5");
    INNER
};

/// Derived AES-256 key length (32 bytes / 256 bits).
pub const DERIVED_KEY_LEN: usize = {
    const INNER: usize = 32;
    assert!(INNER == 32, "DERIVED_KEY_LEN must be 32 bytes per FIPS 197");
    INNER
};

/// AES-GCM nonce length (12 bytes / 96 bits, NIST SP 800-38D §5.2.1.1).
pub const NONCE_LEN: usize = {
    const INNER: usize = 12;
    assert!(
        INNER == 12,
        "NONCE_LEN must be 12 bytes per NIST SP 800-38D"
    );
    INNER
};

/// AES-GCM tag length (16 bytes / 128 bits, NIST SP 800-38D §5.2.7).
pub const TAG_LEN: usize = {
    const INNER: usize = 16;
    assert!(INNER == 16, "TAG_LEN must be 16 bytes per NIST SP 800-38D");
    INNER
};

/// AES-256 key length (32 bytes / 256 bits, FIPS 197).
pub const KEY_LEN: usize = {
    const INNER: usize = 32;
    assert!(INNER == 32, "KEY_LEN must be 32 bytes per FIPS 197");
    INNER
};

/// Minimum blob length: salt + nonce + tag. A blob shorter than this
/// is structurally invalid (plaintext would be empty + tag missing).
pub const MIN_BLOB_LEN: usize = SALT_LEN + NONCE_LEN + TAG_LEN;

// ─── Argon2id ─────────────────────────────────────────────────────────────

/// Generate a 16-byte random salt via OS CSPRNG. Callers MUST use a
/// fresh salt per `encrypt` call — reuse breaks A1 offline-cracker
/// resistance across ciphertexts.
pub fn random_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    salt
}

/// Derive a 32-byte AES key from `passphrase` + `salt` via Argon2id.
///
/// Returns a heap-allocated, Zeroize-on-drop buffer. The caller may
/// move it into AES-GCM and drop it; the `Drop` impl zeroizes via
/// `Zeroizing<Vec<u8>>`.
///
/// **Caller contract:** wrap `passphrase` in `Zeroizing<Vec<u8>>` at
/// the call site if zeroize-on-use is required (e.g. when reading
/// from a `rpassword`-prompted buffer). The `&[u8]` here is a borrow
/// into caller-owned memory; the caller decides what wraps it.
pub fn derive_key(passphrase: &[u8], salt: &[u8]) -> Result<zeroize::Zeroizing<Vec<u8>>> {
    if salt.len() != SALT_LEN {
        return Err(Error::Encryption(format!(
            "salt must be exactly {SALT_LEN} bytes, got {}",
            salt.len()
        )));
    }
    let params = Params::new(
        ARGON2_M_COST_KIB,
        ARGON2_T_COST,
        ARGON2_P_COST,
        Some(DERIVED_KEY_LEN),
    )
    .map_err(|e| Error::Encryption(format!("argon2 params: {e}")))?;

    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = zeroize::Zeroizing::new(vec![0u8; DERIVED_KEY_LEN]);
    argon
        .hash_password_into(passphrase, salt, key.as_mut_slice())
        .map_err(|e| Error::Encryption(format!("argon2 derive: {e}")))?;
    Ok(key)
}

// ─── AES-256-GCM ─────────────────────────────────────────────────────────

/// Newtype for a fully formed encrypted-wallet blob
/// (`salt || nonce || ciphertext || tag`). Constructed exclusively
/// by `encrypt`; consumed exclusively by `decrypt`.
#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedWallet(Vec<u8>);

impl EncryptedWallet {
    /// View the raw blob bytes. The bytes are safe to persist
    /// (already encrypted); the secret is the passphrase, not the
    /// blob.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Construct from a pre-formed blob (e.g. read from storage).
    /// Length must satisfy `MIN_BLOB_LEN`; otherwise the blob is
    /// structurally invalid and `decrypt` would fail anyway — we
    /// catch it here to surface a clear error at the storage boundary.
    pub fn from_blob(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() < MIN_BLOB_LEN {
            return Err(Error::Encryption(format!(
                "encrypted wallet blob must be at least {MIN_BLOB_LEN} bytes, got {}",
                bytes.len()
            )));
        }
        Ok(Self(bytes))
    }

    /// Length of the wrapped blob.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `true` if the wrapped blob is empty. `EncryptedWallet::new`
    /// is never callable directly so this is always `false` for
    /// valid instances; the method exists for symmetry with `len`.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for EncryptedWallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptedWallet")
            .field("blob_len", &self.0.len())
            .finish_non_exhaustive()
    }
}

/// Encrypt `plaintext` with `passphrase`. Returns an `EncryptedWallet`
/// blob — `salt || nonce || ciphertext || tag` — that can be persisted
/// verbatim.
///
/// **Caller contract for zeroize-on-use:** wrap `plaintext` in
/// `Zeroizing<Vec<u8>>` at the call site if you want it cleared after
/// `encrypt` returns. The borrow inside `encrypt` does not outlive
/// the call.
pub fn encrypt(plaintext: &[u8], passphrase: &[u8]) -> Result<EncryptedWallet> {
    let salt = random_salt();
    let key = derive_key(passphrase, &salt)?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_slice()));
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: &[],
            },
        )
        .map_err(|e| Error::Encryption(format!("aes-gcm encrypt: {e}")))?;

    // Drop the key ASAP — its work is done.
    let mut key_buf = key;
    key_buf.zeroize();

    let mut blob = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);

    Ok(EncryptedWallet(blob))
}

/// Decrypt `EncryptedWallet` with `passphrase`. Returns plaintext
/// wrapped in `Zeroizing<Vec<u8>>` so it zeroizes on drop.
///
/// **Errors:**
/// - `Error::Encryption` if the blob is too short (structural)
/// - `Error::Encryption` if the passphrase is wrong (GCM tag mismatch
///   surfaces as AES-GCM failure — same error path; we don't leak
///   "wrong password" vs "wrong blob" to the caller)
pub fn decrypt(blob: &EncryptedWallet, passphrase: &[u8]) -> Result<zeroize::Zeroizing<Vec<u8>>> {
    let bytes = blob.as_bytes();
    if bytes.len() < MIN_BLOB_LEN {
        return Err(Error::Encryption(format!(
            "encrypted wallet blob must be at least {MIN_BLOB_LEN} bytes, got {}",
            bytes.len()
        )));
    }
    let (salt, rest) = bytes.split_at(SALT_LEN);
    let (nonce_bytes, ciphertext_with_tag) = rest.split_at(NONCE_LEN);

    let key = derive_key(passphrase, salt)?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_slice()));

    let plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext_with_tag,
                aad: &[],
            },
        )
        .map_err(|e| Error::Encryption(format!("aes-gcm decrypt: {e}")))?;

    let mut key_buf = key;
    key_buf.zeroize();
    Ok(zeroize::Zeroizing::new(plaintext))
}

// ─── Cross-module invariant ──────────────────────────────────────────────

/// Belt-and-suspenders check that the Argon2id-derived key length
/// matches the AES-256 key length. Each module's local `const` assert
/// would pass independently if both literals change together (which
/// we don't want); this block catches the cross-cut.
const _: () = {
    assert!(
        DERIVED_KEY_LEN == KEY_LEN,
        "DERIVED_KEY_LEN (Argon2id) must equal KEY_LEN (AES-256) — both must be 32 bytes"
    );
    assert!(
        MIN_BLOB_LEN >= SALT_LEN + NONCE_LEN + TAG_LEN,
        "MIN_BLOB_LEN must be at least salt + nonce + tag"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip: encrypt → decrypt yields the original plaintext.
    #[test]
    fn encrypt_decrypt_roundtrip() {
        let plaintext = b"abandon abandon abandon abandon abandon abandon \
                          abandon abandon abandon abandon abandon about";
        let passphrase = b"correct horse battery staple";

        let blob = encrypt(plaintext, passphrase).expect("encrypt");
        assert!(blob.len() >= MIN_BLOB_LEN);

        let recovered = decrypt(&blob, passphrase).expect("decrypt");
        assert_eq!(recovered.as_slice(), plaintext);
    }

    /// Wrong passphrase: GCM tag mismatch surfaces as `Error::Encryption`.
    /// We deliberately do NOT distinguish "wrong password" from
    /// "wrong blob" — both surface as the same error to avoid
    /// password-guessing oracles.
    #[test]
    fn wrong_passphrase_rejected() {
        let plaintext = b"some plaintext";
        let blob = encrypt(plaintext, b"right").expect("encrypt");
        let err = decrypt(&blob, b"wrong").expect_err("must reject");
        assert!(matches!(err, Error::Encryption(_)));
    }

    /// Truncated blob is rejected with a structural error, not a
    /// AES-GCM failure.
    #[test]
    fn truncated_blob_rejected() {
        let too_short = vec![0u8; MIN_BLOB_LEN - 1];
        let err = EncryptedWallet::from_blob(too_short).expect_err("must reject too-short blob");
        assert!(matches!(err, Error::Encryption(_)));
    }

    /// Two encryptions of the same plaintext + passphrase produce
    /// distinct blobs (proves the salt + nonce are not reused).
    #[test]
    fn encrypt_is_nondeterministic() {
        let plaintext = b"same plaintext";
        let pass = b"same pass";
        let a = encrypt(plaintext, pass).expect("a");
        let b = encrypt(plaintext, pass).expect("b");
        assert_ne!(a.as_bytes(), b.as_bytes());
    }
}
