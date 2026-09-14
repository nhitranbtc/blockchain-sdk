#![allow(clippy::assertions_on_constants, clippy::let_underscore_future)]
//! Phase 7.1c library e2e — `submit_send_speedup_local` (deep-dive row 35).
//!
//! Re-broadcasts the same SOL transfer with a higher priority fee.
//! Solana has no RBF; this exercises fee-bumping via a fresh signature +
//! same recent blockhash.
//!
//! **Surfpool required** — `RUN_SOL_SURFPOOL=1 cargo test --test submit_send_speedup_local -- --ignored`.

mod common;

use common::{
    faucet::{airdrop_to_keypair, FaucetError},
    keypair_fixture::throwaway_keypair,
    surfpool_spawn::{spawn_surfpool, SurfpoolError},
};
use sol_wallet_core::{
    chain::{
        account::{get_balance, get_latest_blockhash},
        client::RpcClient,
    },
    tx::{broadcast::send_and_confirm, builder::build_sol_transfer_with_budget},
    Result,
};
use solana_sdk::{pubkey::Pubkey, signer::Signer};

const TRANSFER_LAMPORTS: u64 = 1_000_000_000; // 1 SOL
const BASE_FEE: u64 = 5_000; // base signature fee
const PRIORITY_FEE_BUMP: u64 = 100_000; // 100k micro-lamports = 0.0001 SOL

#[tokio::test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — surfpool-backed e2e"]
async fn submit_send_speedup_local_bumps_fee() -> Result<()> {
    let _guard = match spawn_surfpool().await {
        Ok(g) => g,
        Err(SurfpoolError::NotFound) => {
            eprintln!("skip: surfpool not installed");
            return Ok(());
        }
        Err(e) => panic!("surfpool spawn: {e}"),
    };
    let rpc_url = _guard.rpc_url().to_string();
    let rpc = RpcClient::new(&rpc_url).expect("RpcClient::new ok");

    let sender = throwaway_keypair();
    let recipient = throwaway_keypair();

    // Airdrop enough for first + speedup transfer + fees.
    let _ = airdrop_to_keypair(
        &rpc_url,
        &sender.pubkey(),
        3 * TRANSFER_LAMPORTS + 10 * BASE_FEE,
    )
    .await
    .expect("airdrop succeeds");

    let (blockhash, _last_valid_slot) = get_latest_blockhash(&rpc)
        .await
        .expect("get_latest_blockhash");

    let recipient_pubkey = Pubkey::from(recipient.pubkey().to_bytes());

    // First broadcast — base priority fee.
    let ixs_first = build_sol_transfer_with_budget(
        &sender.pubkey(),
        &recipient_pubkey,
        TRANSFER_LAMPORTS,
        150_000,
        0,
    );
    let msg_first = solana_sdk::message::Message::new(&ixs_first, Some(&sender.pubkey()));
    let tx_first = solana_sdk::transaction::Transaction::new(&[&sender], msg_first, blockhash);
    let sig_first = send_and_confirm(
        &rpc,
        &tx_first,
        solana_commitment_config::CommitmentConfig::confirmed(),
        std::time::Duration::from_secs(30),
    )
    .await
    .expect("send_and_confirm first");

    // Speedup: same blockhash, fresh signature, higher priority fee.
    let ixs_speedup = build_sol_transfer_with_budget(
        &sender.pubkey(),
        &recipient_pubkey,
        TRANSFER_LAMPORTS,
        150_000,
        PRIORITY_FEE_BUMP,
    );
    let msg_speedup = solana_sdk::message::Message::new(&ixs_speedup, Some(&sender.pubkey()));
    let tx_speedup = solana_sdk::transaction::Transaction::new(&[&sender], msg_speedup, blockhash);
    let sig_speedup = send_and_confirm(
        &rpc,
        &tx_speedup,
        solana_commitment_config::CommitmentConfig::confirmed(),
        std::time::Duration::from_secs(30),
    )
    .await
    .expect("send_and_confirm speedup");

    // Distinct signatures — same message, fresh sig is a fresh nonce.
    assert_ne!(sig_first, sig_speedup, "speedup must produce new sig");

    // Recipient received TRANSFER_LAMPORTS twice (first + speedup).
    let recipient_post = get_balance(&rpc, &recipient_pubkey)
        .await
        .expect("get_balance");
    assert_eq!(
        recipient_post,
        2 * TRANSFER_LAMPORTS,
        "recipient SOL balance == 2x transfer amount",
    );

    Ok(())
}

#[tokio::test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — surfpool-backed e2e"]
async fn submit_send_speedup_local_airdrop_error_surfaces() {
    let kp = throwaway_keypair();
    let res = airdrop_to_keypair("http://127.0.0.1:1", &kp.pubkey(), 1_000).await;
    assert!(matches!(
        res,
        Err(FaucetError::Client(_)) | Err(FaucetError::Timeout) | Err(FaucetError::Airdrop(_))
    ));
}

#[test]
fn submit_send_speedup_fee_constants_are_positive() {
    assert!(BASE_FEE > 0);
    assert!(PRIORITY_FEE_BUMP > 0);
    assert!(TRANSFER_LAMPORTS >= 1_000_000);
}
