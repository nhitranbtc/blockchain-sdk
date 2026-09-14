#![allow(clippy::assertions_on_constants, clippy::let_underscore_future)]
//! Phase 7.1c library e2e — `submit_sol_local` (deep-dive row 26).
//!
//! Signs a native SOL transfer with `Keypair::try_sign` (raw Ed25519,
//! no WalletManager), broadcasts via `send_and_confirm`, asserts on-chain
//! balance delta on surfpool.
//!
//! The OwnedLock<Zeroizing<Keypair>> unlock path is exercised by
//! WalletManager unit tests in `tests/wallet_lifecycle.rs`; the e2e
//! here stays raw so the broadcast path is independent of the unlock
//! path (Phase 6.1 zeroize-on-drop is the primary unlock coverage).
//!
//! **Surfpool required** — `RUN_SOL_SURFPOOL=1 cargo test --test submit_sol_local -- --ignored`.

mod common;

use common::{
    faucet::{airdrop_and_wait, airdrop_to_keypair, FaucetError},
    keypair_fixture::throwaway_keypair,
    surfpool_spawn::{spawn_surfpool, SurfpoolError},
};
use sol_wallet_core::{
    chain::{
        account::{get_balance, get_latest_blockhash},
        client::RpcClient,
    },
    tx::{broadcast::send_and_confirm_versioned, builder::build_sol_transfer_with_budget},
    Result,
};
use solana_sdk::{pubkey::Pubkey, signer::Signer};

/// Acceptance criterion: after `send_and_confirm`, the recipient's
/// on-chain balance increased by the transferred amount. The 5000-lamport
/// base fee is paid by the sender (not asserted).
const TRANSFER_LAMPORTS: u64 = 1_000_000_000; // 1 SOL

#[tokio::test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — surfpool-backed e2e; binary not installed in sandbox"]
async fn submit_sol_local_round_trip() -> Result<()> {
    let _guard = match spawn_surfpool().await {
        Ok(g) => g,
        Err(SurfpoolError::NotFound) => {
            eprintln!("skip: surfpool binary not on PATH (set RUN_SOL_SURFPOOL=1 after install)");
            return Ok(());
        }
        Err(e) => panic!("surfpool spawn failed: {e}"),
    };
    let rpc_url = _guard.rpc_url().to_string();
    let rpc = RpcClient::new(&rpc_url).expect("RpcClient::new ok");

    let sender = throwaway_keypair();
    let recipient = throwaway_keypair();

    let _airdrop_sig = airdrop_to_keypair(&rpc_url, &sender.pubkey(), 2 * TRANSFER_LAMPORTS)
        .await
        .expect("airdrop succeeds");

    let (blockhash, _last_valid_slot) = get_latest_blockhash(&rpc)
        .await
        .expect("get_latest_blockhash");

    let recipient_pubkey = Pubkey::from(recipient.pubkey().to_bytes());
    // Plan Q8 default: 150k CU, 0 priority fee.
    let ixs = build_sol_transfer_with_budget(
        &sender.pubkey(),
        &recipient_pubkey,
        TRANSFER_LAMPORTS,
        150_000,
        0,
    );
    // Phase 8.5: build a `VersionedTransaction` directly via V0 — surfpool
    // 1.5.0 (and Anza RPC in 2026-Q3) reject the legacy `Transaction`
    // wire format produced by `Transaction::new`.
    let v0_msg = solana_sdk::message::v0::Message::try_compile(
        &sender.pubkey(),
        &ixs,
        &[], // no address lookups for direct simple transfer
        blockhash,
    )
    .expect("compile v0 message");
    let v0_tx = solana_sdk::transaction::VersionedTransaction::try_new(
        solana_sdk::message::VersionedMessage::V0(v0_msg),
        &[&sender],
    )
    .expect("sign v0 tx");

    let recipient_pre = get_balance(&rpc, &recipient_pubkey).await.unwrap_or(0);
    let _sig = send_and_confirm_versioned(
        &rpc,
        &v0_tx,
        solana_commitment_config::CommitmentConfig::confirmed(),
        std::time::Duration::from_secs(30),
    )
    .await
    .expect("send_and_confirm_versioned");

    let recipient_post = get_balance(&rpc, &recipient_pubkey)
        .await
        .expect("get_balance post");
    assert_eq!(
        recipient_post.saturating_sub(recipient_pre),
        TRANSFER_LAMPORTS,
        "recipient SOL balance delta == transferred amount",
    );

    Ok(())
}

#[tokio::test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — surfpool-backed e2e"]
async fn submit_sol_local_airdrop_waits_for_credit() -> Result<()> {
    let _guard = match spawn_surfpool().await {
        Ok(g) => g,
        Err(SurfpoolError::NotFound) => {
            eprintln!("skip: surfpool not installed");
            return Ok(());
        }
        Err(e) => panic!("surfpool spawn: {e}"),
    };
    let rpc_url = _guard.rpc_url().to_string();
    let kp = throwaway_keypair();
    let (_sig, post) = airdrop_and_wait(&rpc_url, &kp.pubkey(), TRANSFER_LAMPORTS, 0)
        .await
        .expect("airdrop_and_wait");
    assert!(
        post >= TRANSFER_LAMPORTS,
        "balance {post} >= airdrop amount"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — surfpool-backed e2e"]
async fn submit_sol_local_airdrop_faucet_error_surfaces() {
    let kp = throwaway_keypair();
    let res = airdrop_to_keypair("http://127.0.0.1:1", &kp.pubkey(), 1_000).await;
    assert!(
        matches!(
            res,
            Err(FaucetError::Client(_)) | Err(FaucetError::Timeout) | Err(FaucetError::Airdrop(_))
        ),
        "bad URL must surface structured error, got {res:?}",
    );
}
