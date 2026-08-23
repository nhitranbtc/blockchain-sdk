//! `eth-wallet-core` — Ethereum (ETH + ERC-20) wallet library built on `alloy` v1.8.x.
//!
//! Implementation per `docs/superpowers/plans/2026-08-23-eth-wallet-core.md`.
//! Task 1 (Issue #295) ships the mnemonic + address scaffold; Task 2
//! (Issue #301) adds the Argon2id + AES-GCM encrypted `WalletManager`.
//! Subsequent Tasks add an Error enum (Task 4), RPC provider (Task 5),
//! ERC-20 surface (Task 7-9), and the `eth` CLI scaffold (Task 10).

pub mod crypto;
pub mod mnemonic;
pub mod wallet;

pub use wallet::{
    EncryptedBlob, Network, Result as WalletResult, WalletCreated, WalletError, WalletInfo,
    WalletManager, WalletMeta,
};
