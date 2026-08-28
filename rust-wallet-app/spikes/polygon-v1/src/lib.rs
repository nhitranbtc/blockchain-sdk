//! Polygon PoS spike V1-V10 verification harness (Issue #417).
//!
//! Empirical resolution layer for Q1-Q8 from Issue #416 before production code
//! lands in `crates/evm-wallet-core/` (refactor of `eth-wallet-core`) +
//! `crates/polygon-wallet-core/` (thin wrapper).
//!
//! Per L29 operator-driven smoke gated on `RUN_POLYGON_AMOY=1` /
//! `RUN_POLYGON_MAINNET=1` env vars; offline tests always run.

pub mod address;
pub mod config;
pub mod eip712;
pub mod erc20;
pub mod provider;
pub mod spki;
pub mod tokens;
