//! `polygon-wallet-core` — Polygon (PoS) thin wrapper over `evm-wallet-core`.
//!
//! Phase 1 of `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md`.
//! Q1 Option A: this crate re-exports the EVM-compatible core types and
//! adds Polygon-specific RPC URL constants + gas-token labels.
//!
//! No signing or RPC code lives here — all of it is inherited from
//! `evm-wallet-core`. The thin wrapper exists so downstream consumers
//! (Phase 4 `polygon` CLI, integration tests) can write a single
//! `polygon-wallet-core` dependency and get a Polygon-typed API surface
//! without importing the umbrella crate directly.
//!
//! Issue #416 sub-tasks: #423 (Phase 1, this file) · sibling #424 (Phase 2
//! RPC constructors) · #425 (Phase 3 token registry) · #426 (Phase 4 CLI).

pub mod disambig;
pub mod network;
pub mod tokens;

// Re-export shared EVM types so callers can depend on `polygon-wallet-core`
// alone. The `Network` enum is two-level (family + instance) per Phase 0
// of the polygon plan; `EthereumChain` / `PolygonChain` are the per-family
// instance enums.
pub use evm_wallet_core::network::{EthereumChain, Network, PolygonChain};
pub use evm_wallet_core::{Error, Result};

// T6c re-export: WalletManager is the canonical wallet-state container.
// Re-exporting here lets the polygon CLI consume it through the
// single-import-surface (Q1 Option A) without depending on `evm-wallet-core`
// directly — per design doc Drift #5 + §10.3 (Q1 Option A thin wrapper).
pub use evm_wallet_core::wallet::WalletInfo;
pub use evm_wallet_core::wallet::WalletManager;

// T6c1 re-export: provider constructors (added by Phase 2 PR #424 / #431)
// — `new_http`, `new_http_insecure`, `new_http_polygon_mainnet`,
// `new_http_polygon_amoy`. These return `RootProvider<Ethereum>`
// directly (no ProviderBuilder transport inference rough edges that
// block real `wallet_balance` impl). Re-exporting preserves the
// Q1 Option A single-import-surface invariant.
pub use evm_wallet_core::provider::{
    new_http, new_http_insecure, new_http_polygon_amoy, new_http_polygon_mainnet,
};

// T6c4: re-export `WalletCreated` + `WalletError` so the polygon CLI's
// `map_wallet_err` helper (per design doc §5.3) can translate the
// lib's wallet-specific error variants (`AlreadyExists`, `Crypto(_)`,
// `Mnemonic(_)`, etc.) onto the CLI's canonical `Error::InvalidInput`
// exit-2 surface. Same explicit per-item re-export pattern as
// `Network`/`PolygonChain`/`EthereumChain`/`Error`/`Result` above and
// `WalletInfo`/`WalletManager`/`new_http*` below — `pub use glob`
// would over-export lib internals (Q1 Option A single-import-surface
// property, per design §10.3). Additive, backward-compatible.
pub use evm_wallet_core::wallet::WalletCreated;
pub use evm_wallet_core::wallet::WalletError;

// T6c5: re-export `sign_native_eth_tx` + `encoded_envelope` so the
// polygon CLI's `wallet_send_native_v2` + `wallet_send_speedup_v2`
// can build EIP-1559 envelopes + broadcast via the single-import-
// surface invariant (Q1 Option A — no direct `evm-wallet-core`
// dependency in the CLI crate). Mirrors the `eth` CLI's pattern at
// `eth/src/handlers.rs:680, 934`. Additive, backward-compatible.
pub use evm_wallet_core::signer::{encoded_envelope, sign_native_eth_tx};

// T6c3 follow-up #3: `TxSummary` is the Transfer-event summary type
// returned by the `polygon wallet sync` handler. Lives here (not in
// the polygon binary crate, which is `publish = false` and effectively
// private to that crate) so downstream consumers (future `--export`
// writer, integration tests, sister CLIs) share one canonical type.
// Derives mirror alloy's `B256` / `Address` / `U256` serde defaults
// (0x-prefixed hex for the fixed-size types, decimal for U256).
use alloy_primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};

/// Lightweight Transfer-event summary.
///
/// Per design doc §5.4 (amended T6c3 follow-up #3) the full
/// `Vec<Transaction>` payload is too heavy for the CLI summary path;
/// this minimal subset is what operator UX displays in `--json`
/// output and the eventual `--export` Zeroizing payload. Field names
/// match Etherscan-style labels so operators recognize them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxSummary {
    /// Block number containing the Transfer log (0 if pending / unknown).
    pub block_number: u64,
    /// Transaction hash emitting the Transfer log.
    pub tx_hash: B256,
    /// Sender address (topics[1] in ERC-20 Transfer).
    pub from: Address,
    /// Recipient address (topics[2] in ERC-20 Transfer).
    pub to: Address,
    /// Transfer amount in token base units (decoded from log data).
    pub value: U256,
}
