//! Phase 7.1c test helper — airdrop SOL to a keypair via surfpool.
//!
//! Surfpool's `request_airdrop` is a devnet-style RPC; on a local validator
//! it credits the account instantly. Used by `submit_*_local` tests to
//! fund ephemeral keypairs before signing transactions.

#![allow(dead_code)] // Surfpool helpers only used by ignored e2e tests.

use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;

use sol_wallet_core::chain::RpcClient;

/// Airdrop `lamports` SOL to `recipient` via surfpool's `requestAirdrop`.
///
/// **Surfpool only** — caller MUST gate via `#[ignore]` + `RUN_SOL_SURFPOOL=1`.
pub async fn airdrop_surfpool(
    _rpc: &RpcClient,
    _recipient: &Pubkey,
    _lamports: u64,
) -> anyhow::Result<()> {
    // Scaffolded — full requestAirdrop call lands with surfpool CI integration.
    Err(anyhow::anyhow!(
        "faucet::airdrop_surfpool not yet wired — needs RPC client request_airdrop call \
         with poll-for-confirm. See plan 2026-09-09-sol-wallet-core-v0.1.md Phase 7.1c step 5."
    ))
}

/// Airdrop helper that takes `&Keypair` (recipient = keypair.pubkey()).
pub async fn airdrop_to_keypair(
    rpc: &RpcClient,
    keypair: &solana_sdk::signature::Keypair,
    lamports: u64,
) -> anyhow::Result<()> {
    airdrop_surfpool(rpc, &keypair.pubkey(), lamports).await
}
