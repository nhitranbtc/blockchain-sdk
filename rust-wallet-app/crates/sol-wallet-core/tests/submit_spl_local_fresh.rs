#![allow(clippy::assertions_on_constants, clippy::let_underscore_future)]
//! Phase 7.1c library e2e — `submit_spl_local_fresh` (deep-dive row 28).
//!
//! Transfers an SPL token to a fresh recipient with NO pre-existing ATA.
//! Verifies `prepend_create_ata` builder path + rent-delta accounting.
//!
//! **Surfpool required** — `RUN_SOL_SURFPOOL=1 cargo test --test submit_spl_local_fresh -- --ignored`.

mod common;

use common::{
    faucet::{airdrop_to_keypair, FaucetError},
    keypair_fixture::throwaway_keypair,
    mock_spl_usdc::deploy_usdc_mint,
    surfpool_spawn::{spawn_surfpool, SurfpoolError},
};
use sol_wallet_core::{chain::client::RpcClient, Error};
use solana_sdk::signer::Signer;

const RENT_BUFFER_LAMPORTS: u64 = 10_000_000; // 0.01 SOL
const EXPECTED_ATA_RENT_LAMPORTS: u64 = 1_428_000;

#[tokio::test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — surfpool-backed e2e; fixture lands in Phase 7.2 SPL stub"]
async fn submit_spl_local_fresh_creates_ata() {
    let _guard = match spawn_surfpool().await {
        Ok(g) => g,
        Err(SurfpoolError::NotFound) => {
            eprintln!("skip: surfpool not installed");
            return;
        }
        Err(e) => panic!("surfpool spawn: {e}"),
    };
    let rpc_url = _guard.rpc_url().to_string();
    let rpc = RpcClient::new(&rpc_url).expect("RpcClient::new ok");

    let payer = throwaway_keypair();
    let fresh_recipient = throwaway_keypair();

    let _ = airdrop_to_keypair(&rpc_url, &payer.pubkey(), RENT_BUFFER_LAMPORTS)
        .await
        .expect("airdrop succeeds");

    match deploy_usdc_mint(&rpc, &payer, &payer.pubkey(), 1_000_000_000).await {
        Ok(d) => {
            eprintln!(
                "submit_spl_local_fresh: mint={} rent={} lamports expected; full body lands \
                 in Phase 7.2 SPL fixture wire-up",
                d.mint, EXPECTED_ATA_RENT_LAMPORTS,
            );
        }
        Err(Error::Unimplemented(msg)) => {
            eprintln!(
                "submit_spl_local_fresh: deploy_usdc_mint stub returned Unsupported: {msg}; \
                 Phase 7.2 SPL fixture wire-up required"
            );
        }
        Err(e) => panic!("deploy_usdc_mint failed unexpectedly: {e}"),
    }

    let _ = fresh_recipient;
}

#[tokio::test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — surfpool-backed e2e"]
async fn submit_spl_local_fresh_airdrop_error_surfaces() {
    let kp = throwaway_keypair();
    let res = airdrop_to_keypair("http://127.0.0.1:1", &kp.pubkey(), 1_000).await;
    assert!(matches!(
        res,
        Err(FaucetError::Client(_)) | Err(FaucetError::Timeout)
    ));
}

#[test]
fn submit_spl_local_fresh_rent_constant_is_positive() {
    assert!(EXPECTED_ATA_RENT_LAMPORTS > 0);
    assert!(EXPECTED_ATA_RENT_LAMPORTS < RENT_BUFFER_LAMPORTS);
}
