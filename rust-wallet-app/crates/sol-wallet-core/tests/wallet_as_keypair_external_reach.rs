//! Negative compile-fail test for security audit #566 (Task 10.3 /
//! PR #570 follow-up) — pins that `Wallet::as_keypair` is NOT visible
//! to external crates.
//!
//! `Wallet::as_keypair` was narrowed from `pub` to `pub(crate)` in
//! commit `abb123c6` (merged via PR #570 / squash `461773ac`). An
//! external crate (FFI consumer, downstream library) that tries to
//! call it should receive a compile error: `method 'as_keypair' is
//! private`.
//!
//! ## How to run
//!
//! This file is gated behind `--cfg negative_test`. The regular
//! `cargo test` invocation does NOT enable this cfg, so the file is
//! inert on every CI run. A dedicated CI step in
//! `.github/workflows/rust-sol-core-ci.yml` enables the cfg and
//! asserts the build fails with the expected error.
//!
//! Local reproduction:
//!   ```
//!   RUSTFLAGS="--cfg negative_test" \
//!     cargo test -p sol-wallet-core --test wallet_as_keypair_external_reach
//!   ```
//!
//! Expected: `error[E0624]: method 'as_keypair' is private`. If the
//! build succeeds under this cfg, the audit acceptance has been
//! violated — `as_keypair` has been re-exposed as `pub` and
//! downstream code can extract the secret outside the zeroize
//! lifecycle via `solana_sdk::Keypair::insecure_clone()`.

#![cfg(negative_test)]
// `cfg(negative_test)` is enabled only by the dedicated CI step in
// `.github/workflows/rust-sol-core-ci.yml::rust-test-negative-as-keypair`.
// The regular `cargo test` invocation does not pass
// `--cfg negative_test`, so this `#[allow]` silences the
// `unexpected_cfgs` warning from clippy's `-D warnings`.
#![allow(unexpected_cfgs)]

#[cfg(negative_test)]
#[allow(dead_code, unreachable_code)]
fn attempt_external_as_keypair_call() {
    use sol_wallet_core::wallet::Wallet;
    let wallet = Wallet::from_mnemonic(
        "abandon abandon abandon abandon abandon abandon abandon abandon \
         abandon abandon abandon about",
    )
    .expect("mnemonic");

    // The following line MUST fail to compile: `as_keypair` is
    // `pub(crate)`, invisible from this external test crate.
    let _kp: &solana_sdk::signature::Keypair = wallet.as_keypair();
}
