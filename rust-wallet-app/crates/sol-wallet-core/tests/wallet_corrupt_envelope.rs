//! Phase 10 / Task 10.1 — FFI mapping test for `Error::InvalidSeed`.
//!
//! Acceptance (plan Task 10.1): a corrupted envelope whose decrypted
//! 64-byte secret+pubkey fails `Keypair::try_from` must surface at the
//! FFI boundary as `FfiError::DecryptFailed` (= 6), never
//! `FfiError::Panic` (= 99).
//!
//! This integration test reaches the FFI mapping via the public `Error`
//! + `FfiError` re-exports from `sol_wallet_core`.
//!
//! The unit-level proof that `Wallet::from_bytes` itself no longer
//! panics lives in the `wallet::tests` module of `src/wallet.rs`. The
//! fn is `pub(crate)`, so it cannot be reached from an integration
//! test in `tests/`.
//!
//! Issue: #564 — `Wallet::from_bytes` panics on `Keypair::try_from`
//! rejection, triggers `FfiError::Panic = 99` at FFI boundary.

use sol_wallet_core::error::Error;
use sol_wallet_core::ffi::FfiError;

/// FFI exit-code contract — append-only. Do NOT reorder.
#[test]
fn invalid_seed_maps_to_decrypt_failed_not_panic() {
    // The crash that motivated Task 10.1 surfaced as `FfiError::Panic`
    // (= 99). The fix maps `Error::InvalidSeed` to `FfiError::DecryptFailed`
    // (= 6). Lock the contract here.
    let mapped = FfiError::from(Error::InvalidSeed);
    assert_eq!(
        mapped,
        FfiError::DecryptFailed,
        "Error::InvalidSeed must map to FfiError::DecryptFailed, got {:?}",
        mapped
    );
    assert_eq!(
        FfiError::DecryptFailed.code(),
        6,
        "DecryptFailed code drifted from FFI contract (= 6)"
    );
    assert_ne!(
        FfiError::DecryptFailed.code(),
        FfiError::Panic.code(),
        "DecryptFailed must not collapse to Panic (= 99) — that was the audit finding"
    );
}

/// Regression: every other `Error` variant that currently maps to
/// `DecryptFailed` (`WalletDecryptFailed`) still does. Guards against a
/// refactor that drops the arm when the new `InvalidSeed` arm is added.
#[test]
fn wallet_decrypt_failed_still_maps_to_decrypt_failed() {
    use sol_wallet_core::error::WalletId;
    let id = WalletId::new().expect("osrng");
    let mapped = FfiError::from(Error::WalletDecryptFailed { id });
    assert_eq!(mapped, FfiError::DecryptFailed);
}
