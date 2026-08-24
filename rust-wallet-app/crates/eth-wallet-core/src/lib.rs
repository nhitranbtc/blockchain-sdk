//! `eth-wallet-core` — Ethereum (ETH + ERC-20) wallet library built on `alloy` v1.8.x.
//!
//! Implementation per `docs/superpowers/plans/2026-08-23-eth-wallet-core.md`.
//! Tasks 1 + 2 ship the mnemonic scaffold + Argon2id-encrypted WalletManager.
//! Task 3 (Issue #302) adds sign-only primitives. Task 4 (Issue #303) adds
//! the canonical `Error` + `Result` alias + stable exit-code mapping per
//! #297 M11. Subsequent Tasks add RPC provider (Task 5 — see note),
//! ERC-20 surface (Tasks 7-9), and the `eth` CLI scaffold (Task 10).
//!
//! **Task 5 (Issue #304) status:** SPKI pin verifier + `new_http_pinned`
//! were removed entirely pending a future webpki composition. All RPC
//! traffic uses `new_http` (default rustls TLS + system CAs). Reintroduce
//! when the verifier wiring has webpki delegation for chain/hostname/expiry.

pub mod crypto;
pub mod erc20;
pub mod error;
pub mod mnemonic;
pub mod provider;
pub mod signer;
pub mod tokens;
pub mod wallet;

pub use error::{Error, Result};
pub use provider::new_http;
// `new_http_insecure` is NOT re-exported — it is `#[cfg(any(debug_assertions, ...))]`
// so it's absent in release builds entirely. Direct callers can still reach
// it via `eth_wallet_core::provider::new_http_insecure` in dev/CI.
pub use signer::{
    encoded_envelope, sign_erc20_tx_bytes, sign_message, sign_native_eth_tx, sign_typed_data,
    SignError, SignedEip1559,
};
pub use tokens::{load_chain, lookup_by_symbol, Token};
pub use wallet::{
    EncryptedBlob, Network, Result as WalletResult, WalletCreated, WalletError, WalletInfo,
    WalletManager, WalletMeta,
};
