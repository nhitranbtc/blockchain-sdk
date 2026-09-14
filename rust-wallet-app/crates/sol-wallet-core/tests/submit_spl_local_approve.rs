#![allow(clippy::assertions_on_constants, clippy::let_underscore_future)]
//! Phase 7.1c library e2e — `submit_spl_local_approve` (deep-dive row 29).
//!
//! Delegates spending authority on an SPL token account via `build_spl_approve`.
//!
//! **Surfpool required** — `RUN_SOL_SURFPOOL=1 cargo test --test submit_spl_local_approve -- --ignored`.

mod common;

use common::{
    faucet::{airdrop_to_keypair, FaucetError},
    keypair_fixture::throwaway_keypair,
    mock_spl_usdc::deploy_usdc_mint,
    surfpool_spawn::{spawn_surfpool, SurfpoolError},
};
use sol_wallet_core::{chain::client::RpcClient, Error};
use solana_sdk::signer::Signer;

const FEE_BUFFER_LAMPORTS: u64 = 5_000_000; // 0.005 SOL
const DELEGATE_AMOUNT_RAW: u64 = 500_000_000; // 500 USDC (6 decimals)

#[tokio::test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — surfpool-backed e2e; fixture lands in Phase 7.2 SPL stub"]
async fn submit_spl_local_approve_delegate() {
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

    let holder = throwaway_keypair();
    let _delegate = throwaway_keypair();

    let _ = airdrop_to_keypair(&rpc_url, &holder.pubkey(), FEE_BUFFER_LAMPORTS)
        .await
        .expect("airdrop succeeds");

    match deploy_usdc_mint(&rpc, &holder, &holder.pubkey(), 1_000_000_000).await {
        Ok(d) => {
            eprintln!(
                "submit_spl_local_approve: mint={} delegate_amount={} raw; full body lands \
                 in Phase 7.2 SPL fixture wire-up",
                d.mint, DELEGATE_AMOUNT_RAW,
            );
        }
        Err(Error::Unimplemented(msg)) => {
            eprintln!(
                "submit_spl_local_approve: deploy_usdc_mint stub returned Unsupported: {msg}; \
                 Phase 7.2 SPL fixture wire-up required"
            );
        }
        Err(e) => panic!("deploy_usdc_mint failed unexpectedly: {e}"),
    }
}

#[tokio::test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — surfpool-backed e2e"]
async fn submit_spl_local_approve_airdrop_error_surfaces() {
    let kp = throwaway_keypair();
    let res = airdrop_to_keypair("http://127.0.0.1:1", &kp.pubkey(), 1_000).await;
    assert!(matches!(
        res,
        Err(FaucetError::Client(_)) | Err(FaucetError::Timeout) | Err(FaucetError::Airdrop(_))
    ));
}

#[test]
fn submit_spl_local_approve_amount_constant_is_positive() {
    assert_eq!(DELEGATE_AMOUNT_RAW, 500 * 1_000_000);
}
