//! `chain` — Solana RPC client surface.
//!
//! Phase 5.1 owns three sub-modules:
//!
//! | Sub-module   | Surface                                           |
//! |--------------|---------------------------------------------------|
//! | `client`     | `RpcClient` (thin reqwest JSON-RPC), `RateLimiter` |
//! | `account`    | 15 HTTP RPC method thin wrappers (Phase 5.1 critical path) |
//! | `preflight`  | 5 preflight checks (native balance, token balance, ATA existence, mint decimals, rent) |
//!
//! V0.1 drops the Anza `solana-rpc-client` dependency per issue #555
//! (intrinsic sub-dep conflict). All 15 V0.1 RPC methods are thin
//! `reqwest` POST wrappers around the JSON-RPC envelope. The full
//! 21-method Anza spec + 5 WS subscribes are deferred to V0.1.5
//! (see plan doc §Phase 5 / Q1).
//!
//! SECURITY (Tier 2 finding #3, 2026-09-10): V0.1 reads the wallet keypair
//! from a raw 64-byte file at `$HOME/.config/sol-wallet/wallet.json`
//! (mode 0600). This file contains unencrypted private key bytes —
//! DO NOT sync to cloud storage (iCloud, Dropbox, Google Drive), DO NOT
//! commit to git, DO NOT share the file with any process you do not
//! trust. Phase 6 replaces this with Argon2id-encrypted BIP-39 mnemonic
//! storage; until then, treat the wallet file like a password.
//!
//! Importers: `tx::broadcast` (send_and_confirm uses `chain::client` +
//! `chain::account`); Phase 7 CLI (`sol balance`, `sol send`); the
//! 15 RPC method tests in `tests/rpc_methods_mock.rs`.

pub mod account;
pub mod client;
pub mod preflight;

pub use account::{
    get_account_info, get_balance, get_epoch_info, get_health, get_latest_blockhash,
    get_minimum_balance_for_rent_exemption, get_multiple_accounts, get_recent_prioritization_fees,
    get_signature_status, get_token_account_balance, get_token_accounts_by_owner, get_token_supply,
    get_transaction, get_version, request_airdrop, ConfirmationStatus, TransactionResponse,
    TransactionStatus, DEVNET_HOST_ALLOWLIST,
};
pub use client::{
    BlockhashCache, BlockhashCacheEntry, RateLimiter, RpcClient, DEFAULT_BLOCKHASH_TTL,
};
