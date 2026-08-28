//! `eth-wallet-core` — Ethereum thin wrapper over `evm-wallet-core`.
//!
//! Phase 0 of `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md`,
//! Q1 Option A. All Ethereum-specific code (signing, RPC, ABI, persistence,
//! canonical mnemonic, EIP-712 types) lives in `evm-wallet-core` now;
//! this crate exists only to preserve the historical crate name + import
//! path for the `eth` CLI and external consumers. `polygon-wallet-core`
//! (Phase 1) mirrors this shape with its own Network::Polygon config.

pub use evm_wallet_core::*;
