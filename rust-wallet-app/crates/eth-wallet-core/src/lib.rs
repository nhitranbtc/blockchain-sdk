//! `eth-wallet-core` — Ethereum (ETH + ERC-20) wallet library built on `alloy` v1.8.x.
//!
//! Implementation per `docs/superpowers/plans/2026-08-23-eth-wallet-core.md`.
//! Tasks 1 + 2 ship the mnemonic scaffold + Argon2id-encrypted WalletManager.
//! Task 3 (Issue #302) adds sign-only primitives. Task 4 (Issue #303) adds
//! the canonical 17-variant `Error` + `Result` alias + stable exit-code
//! mapping per #297 M11. Subsequent Tasks add RPC provider (Task 5),
//! ERC-20 surface (Tasks 7-9), and the `eth` CLI scaffold (Task 10).

pub mod crypto;
pub mod error;
pub mod mnemonic;
pub mod signer;
pub mod wallet;

pub use error::{Error, Result};
pub use signer::{
    encoded_envelope, sign_erc20_tx_bytes, sign_message, sign_native_eth_tx, sign_typed_data,
    SignError, SignedEip1559,
};
pub use wallet::{
    EncryptedBlob, Network, Result as WalletResult, WalletCreated, WalletError, WalletInfo,
    WalletManager, WalletMeta,
};
