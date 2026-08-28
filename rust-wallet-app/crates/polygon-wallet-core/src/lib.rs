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

pub mod network;

// Re-export shared EVM types so callers can depend on `polygon-wallet-core`
// alone. The `Network` enum is two-level (family + instance) per Phase 0
// of the polygon plan; `EthereumChain` / `PolygonChain` are the per-family
// instance enums.
pub use evm_wallet_core::network::{EthereumChain, Network, PolygonChain};
