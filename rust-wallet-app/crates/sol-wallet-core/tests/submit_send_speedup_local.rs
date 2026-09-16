#![allow(clippy::assertions_on_constants, clippy::let_underscore_future)]
//! Phase 7.1c library e2e — `submit_send_speedup_local` (deep-dive row 35).
//!
//! Phase 8.5 rewrite: full round-trip + speedup cycle. The original
//! `send_and_confirm` for the first broadcast returns a confirmation
//! of `sig_first`; the test then reuses `tx_first` directly (no
//! `getTransaction` re-decode) and hands it to
//! `tx::speedup::speedup_transfer` with a bumped priority fee.
//! Acceptance: `speedup_result.new_signature != sig_first` (fresh
//! blockhash + bumped fee ix guarantees a different sig) + recipient
//! balance == 2 SOL (first + speedup each transferred 1 SOL).
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
    tx::{
        broadcast::{default_send_options, send_and_confirm},
        builder::build_sol_transfer_with_budget,
        speedup::{speedup_transfer, SpeedupRequest},
        SpeedupResult,
    },
    wallet::Wallet,
    Result,
};
use solana_sdk::{pubkey::Pubkey, signer::Signer as _};

const TRANSFER_LAMPORTS: u64 = 1_000_000_000; // 1 SOL
const BASE_FEE: u64 = 5_000; // base signature fee
const PRIORITY_FEE_BUMP: u64 = 100_000; // 100k micro-lamports = 0.0001 SOL

#[tokio::test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — surfpool-backed e2e; full round-trip + speedup cycle"]
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

    let sender_kp = throwaway_keypair();
    let recipient = throwaway_keypair();
    let recipient_pubkey = Pubkey::from(recipient.pubkey().to_bytes());

    // Build a `Wallet` from the throwaway keypair so the speedup path
    // can sign via `Wallet::keypair`. Production callers use
    // `WalletManager::unlock` + `OwnedLock::wallet` instead.
    let sender_secret_bs58 = bs58::encode(sender_kp.to_bytes()).into_string();
    let wallet = Wallet::from_base58(&sender_secret_bs58).expect("wallet from base58");

    // Airdrop enough for first + speedup transfer + fees.
    let _ = airdrop_to_keypair(
        &rpc_url,
        &wallet.public_key(),
        3 * TRANSFER_LAMPORTS + 10 * BASE_FEE,
    )
    .await
    .expect("airdrop succeeds");

    // ─── First broadcast — base priority fee (0 micro-lamports) ───
    let (blockhash_first, _last_valid_slot) = get_latest_blockhash(&rpc)
        .await
        .expect("get_latest_blockhash first");
    let ixs_first = build_sol_transfer_with_budget(
        &wallet.public_key(),
        &recipient_pubkey,
        TRANSFER_LAMPORTS,
        150_000,
        0, // priority fee = 0
    );
    let v0_msg_first = solana_sdk::message::v0::Message::try_compile(
        &wallet.public_key(),
        &ixs_first,
        &[],
        blockhash_first,
    )
    .expect("compile v0 first");
    let tx_first = solana_sdk::transaction::VersionedTransaction::try_new(
        solana_sdk::message::VersionedMessage::V0(v0_msg_first),
        &[&sender_kp],
    )
    .expect("sign first v0");
    let sig_first = send_and_confirm(
        &rpc,
        &tx_first,
        default_send_options(),
        solana_commitment_config::CommitmentConfig::confirmed(),
        std::time::Duration::from_secs(30),
    )
    .await
    .expect("send_and_confirm first");
    eprintln!("submit_send_speedup: first transfer confirmed, sig={sig_first}");

    // ─── Speedup — fresh blockhash + bumped priority fee ───
    let request = SpeedupRequest {
        original_signature: sig_first,
        original_transaction: tx_first,
        new_priority_fee_micro_lamports: PRIORITY_FEE_BUMP,
        commitment: solana_commitment_config::CommitmentConfig::confirmed(),
        timeout: std::time::Duration::from_secs(30),
    };

    let SpeedupResult {
        new_signature,
        expires_at_slot,
    } = speedup_transfer(&rpc, &wallet, request)
        .await
        .expect("speedup_transfer");
    eprintln!(
        "submit_send_speedup: speedup sig={new_signature}, expires_at_slot={expires_at_slot}"
    );

    // ─── Acceptance ───
    assert_ne!(
        sig_first, new_signature,
        "speedup must produce distinct sig (different blockhash + fee ix)"
    );
    let recipient_post = get_balance(&rpc, &recipient_pubkey)
        .await
        .expect("get_balance post");
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
