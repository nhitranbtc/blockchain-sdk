//! Polygon-specific network constants re-exported for downstream convenience.
//!
//! Per plan `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` Phase 1
//! step 4. The authoritative chain-id / RPC URL / gas-token semantics live
//! on `evm_wallet_core::network::PolygonChain`; this module re-exports the
//! constants as `&'static str` / `u64` literals so the Phase 4 `polygon`
//! CLI can read them without reaching through the enum.
//!
//! All values verified 2026-08-27 per the plan §Q4 / §Q8.

// EIP-155 chain ids. Source of truth: `evm_wallet_core::network::PolygonChain::chain_id`.
pub const CHAIN_ID_POLYGON_MAINNET: u64 = 137;
pub const CHAIN_ID_POLYGON_AMOY: u64 = 80_002;

// Default RPC endpoints. Overridable via `--rpc-url` at the CLI layer (Phases 2/4).
// Source of truth: `evm_wallet_core::network::PolygonChain::rpc_url`.
// Drift from original defaults (Issue #474, 2025-Q3): `polygon-rpc.com` tightened
// keyless-tier access (HTTP 401 on estimate_eip1559_fees + get_block_number).
// Switched to publicnode.com keyless public RPC (verified 2025-Q4).
pub const POLYGON_MAINNET_RPC_URL: &str = "https://polygon-bor-rpc.publicnode.com";
pub const POLYGON_AMOY_RPC_URL: &str = "https://polygon-amoy-bor-rpc.publicnode.com";

// Display label for the native gas token. Polygon MATIC → POL rebrand 2024-09-04.
pub const GAS_TOKEN_LABEL: &str = "POL";
/// Legacy alias for wallets that pre-date the MATIC → POL rebrand.
pub const LEGACY_GAS_TOKEN_LABEL: &str = "MATIC";
