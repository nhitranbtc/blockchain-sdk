//! `evm-wallet-core` — EVM-compatible (Ethereum + Polygon + future EVM L2s) wallet library.
//!
//! Refactor target: Phase 0 of `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md`.
//! Q1 Option A — `eth-wallet-core` and `polygon-wallet-core` are thin wrappers over this crate.
//! Issue #416 sub-tasks #421 (Phase 0), #423-#426 (Phases 1-4).

pub mod crypto;
pub mod erc20;
pub mod error;
pub mod mnemonic;
pub mod network;
pub mod provider;
pub mod redact;
pub mod signer;
pub mod tokens;
pub mod wallet;

pub use network::{EthereumChain, Network, PolygonChain};

pub use redact::redact_rpc_url;

pub use error::{Error, Result};
pub use provider::new_http;
// `new_http_insecure` is NOT re-exported — it is `#[cfg(any(debug_assertions, ...))]`
// so it's absent in release builds entirely. Direct callers can still reach
// it via `evm_wallet_core::provider::new_http_insecure` in dev/CI.
pub use signer::{
    encoded_envelope, sign_erc20_tx_bytes, sign_message, sign_native_eth_tx, sign_typed_data,
    SignError, SignedEip1559,
};
pub use tokens::{load_chain, lookup_by_address, lookup_by_symbol, Token};
pub use wallet::{
    EncryptedBlob, Result as WalletResult, WalletCreated, WalletError, WalletInfo, WalletManager,
    WalletMeta,
};
