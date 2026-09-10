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
pub mod error;
pub mod read_only_wallet;
pub mod tx;
pub mod wallet;

pub use error::{Error, Result};

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
