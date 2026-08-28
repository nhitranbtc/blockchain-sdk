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
pub const POLYGON_MAINNET_RPC_URL: &str = "https://polygon-rpc.com";
pub const POLYGON_AMOY_RPC_URL: &str = "https://polygon-amoy.drpc.org";

// Display label for the native gas token. Polygon MATIC → POL rebrand 2024-09-04.
pub const GAS_TOKEN_LABEL: &str = "POL";
/// Legacy alias for wallets that pre-date the MATIC → POL rebrand.
pub const LEGACY_GAS_TOKEN_LABEL: &str = "MATIC";
