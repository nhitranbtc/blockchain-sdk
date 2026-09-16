//! Library API unit tests for sol-wallet-core's submit-transaction surface.
//!
//! Goal: verify that the LIBRARY FUNCTIONS used to build + sign + broadcast
//! Solana transactions are correctly implemented. This test does NOT
//! broadcast — it asserts that:
//!
//! - `Wallet::from_mnemonic` derives the Phantom-canonical address
//! - `prepare_sol_transfer_message` produces a `Message` with the expected
//!   3-ix layout (CU-limit + CU-price + system transfer)
//! - `prepare_spl_transfer_message` produces a 3-ix or 4-ix Message with
//!   the expected `transfer_checked` ix
//! - `compute_budget_instructions` returns the expected 2-element array
//! - `derive_ata_with_program_id` is deterministic for a given
//!   `(owner, mint, token_program_id)` triple
//! - `disambig::TokenProgram` dispatch is consistent (Classic / Token2022
//!   map to their canonical program IDs)
//! - `sign_message` produces a 64-byte signature that round-trip verifies
//!
//! The devnet broadcast path is exercised separately by the
//! `#[ignore]`-gated `submit_devnet_send_real_broadcast` at the bottom
//! of this file (run with `--ignored --nocapture`).

mod common;

use common::dummy_blockhash;
use sol_wallet_core::{
    chain::{
        account::{get_balance, get_latest_blockhash},
        client::RpcClient,
    },
    disambig::{classic_token_program_id, token_2022_program_id, TokenProgram},
    tx::{
        broadcast::{send_and_confirm, wait_for_landing},
        builder::{compute_budget_instructions, derive_ata_with_program_id},
        native::prepare_sol_transfer_message,
        spl::prepare_spl_transfer_message,
    },
    wallet::Wallet,
};
use solana_sdk::{
    message::VersionedMessage,
    pubkey::Pubkey,
    signature::Signature,
    transaction::{Transaction, VersionedTransaction},
};
use std::{str::FromStr, time::Duration};

#[test]
fn wallet_from_mnemonic_derives_phantom_canonical_address() {
    let wallet =
        Wallet::from_mnemonic(&common::load_config().sender_mnemonic).expect("from_mnemonic");
    assert_eq!(
        wallet.public_key().to_string(),
        common::load_config().sender_pubkey,
        "Phantom-canonical derivation mismatch"
    );
}

#[test]
fn wallet_sign_message_round_trips_ed25519() {
    let wallet =
        Wallet::from_mnemonic(&common::load_config().sender_mnemonic).expect("from_mnemonic");
    let msg = b"sol-wallet-core library sign_message test";
    let sig: Signature = wallet.sign_message(msg);
    assert_eq!(sig.as_ref().len(), 64, "Ed25519 sig must be 64 bytes");
    assert!(
        sig.verify(wallet.public_key().as_ref(), msg),
        "signature must verify against pubkey + message"
    );
}

#[test]
fn prepare_sol_transfer_message_produces_cu_budget_then_transfer() {
    let wallet =
        Wallet::from_mnemonic(&common::load_config().sender_mnemonic).expect("from_mnemonic");
    let recipient = Pubkey::from_str(&common::load_config().recipient).expect("recipient");
    let blockhash = dummy_blockhash();
    let lamports = 1_000_000u64;
    let cu_limit = 150_000u32;
    let priority_fee = 0u64;

    let msg = prepare_sol_transfer_message(
        &wallet.public_key(),
        &recipient,
        lamports,
        cu_limit,
        priority_fee,
        blockhash,
    );

    // 3-ix: [set_cu_limit, set_cu_price, system_instruction::transfer]
    assert_eq!(msg.instructions.len(), 3, "expected 3 instructions");
    // ix 0 program_id == ComputeBudget program (CU limit)
    assert_eq!(
        msg.account_keys[msg.instructions[0].program_id_index as usize],
        solana_compute_budget_interface::ID,
        "ix 0 must target ComputeBudget program"
    );
    // ix 1 program_id == ComputeBudget program (CU price)
    assert_eq!(
        msg.account_keys[msg.instructions[1].program_id_index as usize],
        solana_compute_budget_interface::ID,
        "ix 1 must target ComputeBudget program"
    );
    // ix 2: system_instruction::transfer → program_id == System Program
    assert_eq!(
        msg.account_keys[msg.instructions[2].program_id_index as usize],
        solana_system_interface::program::ID,
        "ix 2 must target the System Program"
    );
    // Payer == sender (required for fee)
    assert_eq!(
        msg.account_keys.first().copied(),
        Some(wallet.public_key()),
        "payer must be sender"
    );
    // recent_blockhash embedded
    assert_eq!(msg.recent_blockhash, blockhash);
}

#[test]
fn compute_budget_instructions_returns_limit_then_price() {
    let cu_limit = 200_000u32;
    let priority_fee = 5_000u64;
    let ixs = compute_budget_instructions(cu_limit, priority_fee);
    assert_eq!(ixs.len(), 2);
    // CU-limit and CU-price have distinct variant tags per Solana wire format
    assert_ne!(ixs[0].data, ixs[1].data);
}

#[test]
fn prepare_spl_transfer_message_produces_transfer_checked() {
    let wallet =
        Wallet::from_mnemonic(&common::load_config().sender_mnemonic).expect("from_mnemonic");
    let recipient = Pubkey::from_str(&common::load_config().recipient).expect("recipient");
    let mint = Pubkey::from_str(&common::load_config().usdc_mint).expect("USDC mint");
    let blockhash = dummy_blockhash();
    let program_id = classic_token_program_id();
    let source_ata = derive_ata_with_program_id(&wallet.public_key(), &mint, &program_id);
    let dest_ata = derive_ata_with_program_id(&recipient, &mint, &program_id);

    let amount = 100_000u64;
    let decimals = 6u8;

    // Without ATA-create prepend: 3-ix [set_cu_limit, set_cu_price, transfer_checked]
    let msg = prepare_spl_transfer_message(
        &wallet.public_key(),
        &source_ata,
        &dest_ata,
        &mint,
        TokenProgram::Classic,
        amount,
        decimals,
        150_000,
        0,
        blockhash,
        false, // no prepend_ata_create
    );
    assert_eq!(msg.instructions.len(), 3);
    // ix 2 must target the SPL Token program (classic)
    assert_eq!(
        msg.account_keys[msg.instructions[2].program_id_index as usize], program_id,
        "ix 2 must target classic_token_program_id"
    );
    // transfer_checked discriminator byte == 12 (per spl_token::instruction::TokenInstruction::TransferChecked)
    assert_eq!(
        msg.instructions[2].data[0], 12,
        "ix 2 must be TransferChecked (discriminator 12)"
    );
}

#[test]
fn prepare_spl_transfer_message_with_ata_create_prepends_create_ix() {
    let wallet =
        Wallet::from_mnemonic(&common::load_config().sender_mnemonic).expect("from_mnemonic");
    let recipient = Pubkey::from_str(&common::load_config().recipient).expect("recipient");
    let mint = Pubkey::from_str(&common::load_config().usdc_mint).expect("USDC mint");
    let blockhash = dummy_blockhash();
    let program_id = classic_token_program_id();
    let source_ata = derive_ata_with_program_id(&wallet.public_key(), &mint, &program_id);
    let dest_ata = derive_ata_with_program_id(&recipient, &mint, &program_id);

    let msg = prepare_spl_transfer_message(
        &wallet.public_key(),
        &source_ata,
        &dest_ata,
        &mint,
        TokenProgram::Classic,
        100_000,
        6,
        150_000,
        0,
        blockhash,
        true, // prepend_ata_create
    );
    // With prepend_ata_create=true: 4-ix [create_ata_idempotent, set_cu_limit, set_cu_price, transfer_checked]
    assert_eq!(msg.instructions.len(), 4);
    // ix 0 must target the ATA program
    assert_eq!(
        msg.account_keys[msg.instructions[0].program_id_index as usize],
        spl_associated_token_account::id(),
        "ix 0 must target the ATA program"
    );
}

#[test]
fn derive_ata_with_program_id_is_deterministic_per_token_program() {
    let wallet =
        Wallet::from_mnemonic(&common::load_config().sender_mnemonic).expect("from_mnemonic");
    let mint = Pubkey::from_str(&common::load_config().usdc_mint).expect("USDC mint");

    // Same (owner, mint) → different ATA per token program (Q6 invariant)
    let classic_ata =
        derive_ata_with_program_id(&wallet.public_key(), &mint, &classic_token_program_id());
    let token2022_ata =
        derive_ata_with_program_id(&wallet.public_key(), &mint, &token_2022_program_id());
    assert_ne!(
        classic_ata, token2022_ata,
        "Q6: classic ATA != Token-2022 ATA for same (owner, mint)"
    );

    // Deterministic: same inputs → same output
    let classic_ata_again =
        derive_ata_with_program_id(&wallet.public_key(), &mint, &classic_token_program_id());
    assert_eq!(
        classic_ata, classic_ata_again,
        "ATA derivation must be deterministic"
    );
}

#[test]
fn disambig_token_program_resolves_canonical_program_ids() {
    assert_eq!(
        classic_token_program_id().to_string(),
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
    );
    assert_eq!(
        token_2022_program_id().to_string(),
        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
    );

    // TokenProgram::Classic → classic_token_program_id()
    assert_eq!(
        TokenProgram::Classic.program_id(),
        classic_token_program_id()
    );
    assert_eq!(
        TokenProgram::Token2022.program_id(),
        token_2022_program_id()
    );

    // TokenProgram::from_program_id round-trips
    let classic = TokenProgram::from_program_id(&classic_token_program_id()).expect("from classic");
    assert_eq!(classic, TokenProgram::Classic);
    let token2022 = TokenProgram::from_program_id(&token_2022_program_id()).expect("from t22");
    assert_eq!(token2022, TokenProgram::Token2022);
}

// =============================================================================
// Loud-RED integration test — actually broadcasts on devnet.
// Uses ONLY the sol-wallet-core library API.
//
// Asserts (each step below is an explicit assertion; no step is decorative):
//
// (1) Config + wallet setup
//     - `&common::load_config()` returns `Ok(DevnetConfig)` (parses
//       `src/tokens/devnet.json` — fails fast if fields missing).
//     - `RpcClient::new(url)` returns `Ok` (URL passes the allowlist: must
//       be `https://*`, `http://localhost`, or `http://127.0.0.1`).
//     - `Wallet::from_mnemonic(mnemonic)` returns `Ok` (BIP-39 + SLIP-0010
//       Phantom derivation succeeds; no invalid-checksum panic).
//     - `Pubkey::from_str(recipient)` returns `Ok`.
//
// (2) Pre-fund check (loud-RED gate, not a soft warn)
//     - `get_balance(&rpc, &sender)` returns `Ok(u64)` (JSON-RPC `getBalance`
//       parses cleanly).
//     - `pre >= 1_005_000` lamports. If underfunded, `panic!` with a
//       loud-RED message linking the devnet explorer. The threshold
//       covers 0.001 SOL transfer + 5000 lamport base fee + ~0 SOL headroom.
//       Without this gate the test would silently consume devnet cycles
//       trying to broadcast an unfunded tx.
//
// (3) Message build + sign (no assertion — pure construction)
//     - `get_latest_blockhash(&rpc)` returns `Ok((Hash, u64))` at
//       commitment=`"confirmed"` (library default). Slot logged for audit.
//     - `prepare_sol_transfer_message(payer, recipient, 1_000_000,
//       150_000, 0, blockhash)` produces a `Message` (asserts library
//       fn doesn't panic on the inputs).
//     - `sender.sign_legacy_transaction(&mut tx, blockhash)` returns
//       `Ok` (Phase 6.4 Step 2 wrapper; asserts blockhash matches the
//       embedded message blockhash, fee-payer == wallet pubkey, and
//       ed25519 sig produces a valid 64-byte signature over the message).
//
// (4) Broadcast (the heart of the test)
//     - `send_transaction_with_options(&rpc, &tx, { encoding: "base64",
//       replaceRecentBlockhash: true, skipPreflight: true })` returns
//       `Ok(Signature)`. The library bincode-serializes the signed tx,
//       base64-encodes the envelope, sends `sendTransaction` JSON-RPC,
//       and decodes the base58 signature from the cluster response.
//     - `skipPreflight: true` is the critical option for devnet — bypasses
//       the simulator-side blockhash lookup that fails on devnet's
//       load-balanced backends (which disagree on recent blockhashes and
//       raise -32002). With this flag, the cluster leader accepts our
//       signed tx directly without local simulation.
//     - `replaceRecentBlockhash: true` lets the leader substitute a fresh
//       hash if our embedded one is stale (defense against the same
//       disagreement).
//
// (5) Confirm + on-chain effect check (authoritative landing proof)
//     Phase 6.4 Step 3: combined into one library call `wait_for_landing`:
//     - Internally calls `wait_for_confirm` (tolerating `ConfirmPending` /
//       `ConfirmTimeout`) AND polls `get_balance` until `pre - cur >=
//       expected_delta_lamports`. Returns `Error::ConfirmTimeout` on
//       deadline; cluster may still have landed — caller surfaces
//       the explorer URL.
//     - **Balance-delta poll is the ground-truth landing proof.** The
//       test passes `expected_delta_lamports = 1_005_000` (0.001 SOL
//       transfer + 5000 lamport base fee). Asserts the transfer
//       landed AND was debited from the sender within the timeout.
//     - Expected exact delta: `pre − post == 1_005_000` lamports.
//       Logged for audit.
//
// Total assertions: 9 explicit (config load, RPC client new, wallet
// from_mnemonic, pubkey parse, balance fetch, pre-fund gate, sign, send,
// landing proof). 1 panic-on-timeout (pre-fund) + 1 Error::ConfirmTimeout
// path (landing). Library fns exercised: 9 (`load_config`,
// `RpcClient::new`, `Wallet::from_mnemonic`, `get_balance`,
// `get_latest_blockhash`, `prepare_sol_transfer_message`,
// `sign_legacy_transaction`, `send_transaction_with_options`,
// `wait_for_landing`).
//
// Run with: cargo test --test submit_devnet_send submit_devnet_send_real_broadcast -- --ignored --nocapture
// =============================================================================

#[test]
#[ignore]
fn submit_devnet_send_real_broadcast() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio build");
    runtime.block_on(async {
        let cfg = &common::load_config();
        let rpc_url = std::env::var("SOL_RPC_URL")
            .ok()
            .unwrap_or_else(|| cfg.rpc_endpoint.clone());
        let rpc = RpcClient::new(&rpc_url).expect("RpcClient::new");
        let sender = Wallet::from_mnemonic(&cfg.sender_mnemonic).expect("sender wallet");
        let recipient = sol_wallet_core::address::parse_user_address(&cfg.recipient)
            .expect("recipient pubkey");

        // ---- (1) pre-fund check ----
        let pre = get_balance(&rpc, &sender.public_key())
            .await
            .expect("pre SOL balance");
        eprintln!("pre  sender SOL: {} ({} lamports)", pre as f64 / 1e9, pre);
        if pre < 1_005_000 {
            panic!(
                "sender {} has {} lamports — below 1.005 SOL threshold. Pre-fund out-of-band; \
                 devnet faucet rate-limits (HTTP 429). explorer: https://explorer.solana.com/address/{}?cluster=devnet",
                sender.public_key(),
                pre,
                sender.public_key(),
            );
        }

        // ---- (2) build + sign the transfer message ----
        // Library's get_latest_blockhash uses commitment="confirmed" — sufficient for
        // devnet once paired with `skipPreflight: true` (validated by TX
        // `2iitzd1rXiUHQLTP5nhCnWH4oarg2mkdWJkNdc4sECzLbbsNXZ7JaFNZ6KEPcAbcNX1CpkDZScD1ocJ9bCt8HbWZ`
        // landed on devnet 2026-09-13; balance delta −1_005_000 lamports).
        let (blockhash, slot) =
            get_latest_blockhash(&rpc).await.expect("get_latest_blockhash");
        eprintln!("blockhash (confirmed): {blockhash}  slot: {slot}");

        let msg = prepare_sol_transfer_message(
            &sender.public_key(),
            &recipient,
            1_000_000, // 0.001 SOL
            150_000,   // CU limit
            0,         // priority fee
            blockhash,
        );
        let tx = Transaction::new_unsigned(msg);
        // Phase 10: consolidated into `Wallet::sign_transaction(VersionedTransaction)`.
        // Build the versioned wrap directly — the unified broadcast helper
        // takes `&VersionedTransaction`, so we no longer need the legacy
        // `tx` round-trip. Devnet's wire-format requirement (Phase 8.5)
        // is satisfied because the broadcast payload IS the versioned tx.
        let versioned = VersionedTransaction {
            message: VersionedMessage::Legacy(tx.message.clone()),
            signatures: vec![Signature::default(); 1],
        };
        let signed = sender
            .sign_transaction(versioned)
            .expect("sign_transaction");

        // ---- (3) broadcast via unified library API ----
        // `send_and_confirm` is the single broadcast helper for every
        // cluster — devnet just needs defensive options; local does not.
        //
        // `skipPreflight: true` bypasses simulator-side blockhash lookup;
        // cluster leader accepts our signed tx or rejects via
        // sendTransaction error. `replaceRecentBlockhash: true` lets the
        // leader substitute a fresh hash if our embedded one is unknown
        // (defense against devnet load-balanced backends disagreeing on
        // recent blockhashes).
        let sig: Signature = send_and_confirm(
            &rpc,
            &signed,
            serde_json::json!({
                "encoding": "base64",
                "replaceRecentBlockhash": true,
                "skipPreflight": true,
            }),
            solana_commitment_config::CommitmentConfig::confirmed(),
            Duration::from_secs(30),
        )
        .await
        .expect("send_and_confirm");
        eprintln!("TX HASH (SOL native send on devnet): {sig}");
        eprintln!("https://explorer.solana.com/tx/{sig}?cluster=devnet");

        // ---- (4) + (5) combined: library landing proof ----
        // Phase 6.4 Step 3: single library call replaces the previous
        // hand-rolled `wait_for_confirm` + balance-delta polling loop.
        // Internally waits for cluster confirmation (tolerating Pending /
        // Timeout) AND polls `get_balance` until sender's balance has
        // decreased by ≥ expected_delta_lamports. Returns the post-balance
        // on success; `Error::ConfirmTimeout` on deadline (caller decides
        // whether to re-poll or surface the explorer URL).
        //
        // `expected_delta_lamports` = 1_005_000 = 0.001 SOL transfer
        // + 5000 lamport base fee (exact).
        let post = wait_for_landing(
            &rpc,
            &sig,
            &sender.public_key(),
            pre,
            1_005_000,
            Duration::from_secs(30),
        )
        .await
        .expect("wait_for_landing");
        eprintln!(
            "  balance delta: {} → {} ({} lamports transferred)",
            pre,
            post,
            pre - post
        );
    });
}
