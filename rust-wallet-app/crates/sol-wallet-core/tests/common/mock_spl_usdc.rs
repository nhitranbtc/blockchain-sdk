//! Phase 7.1c test helper — deploy a USDC-style SPL mint on surfpool.
//!
//! Used by `submit_spl_local_*` tests (rows 27-29, deep-dive §F). Requires
//! a running surfpool instance on `http://127.0.0.1:8899` — guarded by
//! the `RUN_SOL_SURFPOOL=1` env var in each caller's `#[ignore]` attribute.
//!
//! Mint shape mimics real USDC: 6 decimals, fixed supply 1B, single mint
//! authority derived from the deployer keypair.

#![allow(dead_code)] // Surfpool helpers only used by ignored e2e tests.

use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;

use sol_wallet_core::chain::RpcClient;

/// Address of the deployed mint (returned by `deploy_usdc_mint`).
pub struct MintDeployment {
    /// The mint address.
    pub mint: Pubkey,
}

/// Deploy a USDC-style mint (6 decimals, single mint authority).
///
/// Mints 1B tokens to `initial_holder` so `submit_spl_local_held` has
/// non-zero balance to transfer. Returns the new mint address.
///
/// **Surfpool only** — caller MUST gate via `#[ignore]` + `RUN_SOL_SURFPOOL=1`.
pub async fn deploy_usdc_mint(
    _rpc: &RpcClient,
    _deployer: &solana_sdk::signature::Keypair,
    _initial_holder: &Pubkey,
) -> anyhow::Result<MintDeployment> {
    // Scaffolded for Phase 7.1c — full SPL Token InitializeMint + MintTo
    // instructions land once the test runs against surfpool in CI.
    // Until then, calling this returns an error so ignored tests surface
    // the gap rather than silently passing.
    Err(anyhow::anyhow!(
        "mock_spl_usdc::deploy_usdc_mint not yet wired — needs SPL Token program instructions \
         (InitializeMint + MintTo). See plan 2026-09-09-sol-wallet-core-v0.1.md Phase 7.1c step 5."
    ))
}
