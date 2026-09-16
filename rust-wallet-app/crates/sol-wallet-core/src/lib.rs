//! `sol-wallet-core` — Solana wallet engine.
//!
//! Phase 0 scaffold: compiles, exports a facade over `solana_sdk`, and
//! declares three empty module stubs. No behaviour. Phase 1+ fills them
//! in via the per-Phase plan tickets:
//!
//! | Phase | Lands in                                          |
//! |-------|---------------------------------------------------|
//! | 1     | `wallet` (Phantom-equivalent keypair)             |
//! | 2     | `address` (base58 + is_on_curve + PDA)            |
//! | 3     | `amount` + `tx::builder` (SOL transfer + CU)      |
//! | 4     | `disambig` + `tokens` + `tx::builder` (SPL+ATA+Token-2022) |
//! | 5-7   | `error` (full 21-variant enum)                    |
//!
//! Threat model + spec: `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md`
//! and `docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md`.
//!
//! Per `docs/agents/domain.md`: every signing path that touches a
//! `Secret<...>` must route through `keys::Secret::into_inner` before
//! `ZeroizeOnDrop` fires. Phase 1+ enforces this; Phase 0 has no keys.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod address;
pub mod amount;
pub mod chain;
pub mod crypto;
pub mod disambig;
pub mod error;
pub mod ffi;
pub mod ffi_mnemonic;
pub mod panic_scrubber;
pub mod persist;
pub mod platform;
pub mod read_only_wallet;
pub mod tokens;
pub mod tx;
pub mod wallet;
pub mod wallet_manager;

pub use error::{Error, Result, WalletId};

// Phase 9.1 — re-export the BIP-39 phrase generator as a library-level public API
// (was FFI-internal-only in `ffi_mnemonic`). Examples + downstream library consumers
// (mobile, CLI) call `sol_wallet_core::generate_12_word_english()` directly without
// going through the FFI cdylib. RNG failure surfaces as `Error::OsRngFailed`
// (L13 step 10 Sept 11 — no `unwrap()`/`expect()` on RNG paths).
pub use ffi_mnemonic::generate_12_word_english;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facade_compiles() {
        // Smoke: the lib's public surface resolves without panicking.
        // First real test lands in Phase 1.1 (`wallet::from_mnemonic`).
        let _: fn() -> Result<()> = || Ok(());
    }
}
