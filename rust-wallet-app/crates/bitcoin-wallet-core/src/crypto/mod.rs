//! Cryptographic primitives: Argon2id (KDF), AES-256-GCM (AEAD), BIP-137 (msg sig).
//!
//! Filled by Task 5 (argon2, aes_gcm) and Task 6 (bip137).
//!
//! `mnemonic_cipher` (issue #28) bundles argon2 + aes_gcm into a 2-line
//! encrypt/decrypt API for at-rest mnemonic encryption.

pub mod aes_gcm;
pub mod argon2;
pub mod bip137;
pub mod mnemonic_cipher;

/// Cross-module invariant: the Argon2id-derived key length must equal
/// the AES-256 key length. Both constants now pin themselves to 32 via
/// in-`const` self-asserts (see `argon2::DERIVED_KEY_LEN` and
/// `aes_gcm::KEY_LEN`); this block is the belt-and-suspenders check
/// that catches the case where both literals change together (which
/// would pass each module's local assert in isolation).
///
/// Issue #30 constant audit — see
/// `docs/audit/2026-08-09-l20-constant-audit.md`.
const _: () = {
    assert!(
        crate::crypto::argon2::DERIVED_KEY_LEN == crate::crypto::aes_gcm::KEY_LEN,
        "DERIVED_KEY_LEN (argon2) must equal KEY_LEN (AES-256) — both must be 32 bytes"
    );
};
