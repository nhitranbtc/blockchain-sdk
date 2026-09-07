//! Phase 4 §4.4-4.5 — Nile testnet integration tests for `tron-wallet-core`.
//!
//! ## Migration note (2026-09-07)
//!
//! Moved from `rust-wallet-app/spikes/tron-v1/tests/trc20_nile.rs` 2026-09-07
//! so the live-RPC tests exercise the **production** `tron-wallet-core`
//! implementation rather than the spike's helper layer. The spike copy was
//! the integration source-of-truth during Phase 4 spike-first development;
//! now that `tron-wallet-core` exposes every needed surface
//! (`TronGridClient::new(..., Some(SpkiPin))`, `trc20::balance_of`,
//! `tx::builder::{trc20_transfer, trx_transfer, ...}`, `tx::sign::sign_tx`,
//! `keys::{Mnemonic, DerivationPath, derive_keypair}`, `tokens::by_symbol`),
//! the test belongs with the code it tests.
//!
//! 2026-09-07 fold-in: `tests/v10_broadcast.rs` removed and its three
//! suites absorbed here. The canonical TRC-20 path (v10#1) maps to
//! `trc20_transfer_full_flow_nile` (superset: SPKI pin, balance delta,
//! confirmation poll). The native-TRX path (v10#2) maps to
//! `row_1_trx_native_transfer_nile` (superset: balance delta). The
//! rebroadcast idempotency path (v10#3) maps to
//! `row_2_broadcast_rebroadcast_idempotency_nile` (added below).
//! This file is now the **single source of truth** for operator-driven
//! Nile live-RPC tests in `tron-wallet-core`.
//!
//! ## Gating
//!
//! - All live tests are `#[ignore]`-marked (no env-var gate). CI stays
//!   silent by default. Operator opts in by running with `--ignored`.
//! - Sender + recipient (mnemonic + address) loaded from the bundled
//!   Nile fixture at `crates/tron-wallet-core/tokens/nile.json`
//!   (`test.sender-tr20`, `test.recipient-tr20`) — single source of
//!   truth within this file.
//! - Canonical flow uses **SPKI pinning** (pin from `tokens/nile.json`,
//!   Phase 3 §3.7: `e9cc763b176063ea6eed1525dac2542512d9e0bf601e210a14f6aad218a9479f`)
//!   via `TronGridClient::new(..., Some(SpkiPin::from_bytes(...)))`.
//!   Optional override: `TRON_NILE_SPKI_PIN` env var (64 lowercase hex chars).
//! - Row 1 (native TRX) + Row 2 (rebroadcast idempotency) deliberately
//!   drop the SPKI pin (`TronGridClient::new(..., None)`) to preserve
//!   the "no-pin still works" posture for operators who opt out.
//!
//! ## Operator setup (one-time)
//!
//! ```bash
//! # Derive SPKI pin from a live TLS handshake (or rely on bundled pin):
//! openssl s_client -connect nile.trongrid.io:443 -servername nile.trongrid.io \
//!   </dev/null 2>/dev/null \
//!   | openssl x509 -pubkey -noout \
//!   | openssl pkey -pubin -outform der \
//!   | openssl dgst -sha256 -binary | xxd -p -c 256
//! # Fund BOTH the sender (TRX + USDT) and recipient addresses at the
//! # Nile faucet (https://nileex.io/join/getJoinPage). Addresses come
//! # from `crates/tron-wallet-core/tokens/nile.json`.
//! ```
//!
//! ## Run
//!
//! ```bash
//! cargo test -p tron-wallet-core --test trc20_nile -- --ignored --nocapture
//! ```

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ethereum_types::U256;

use tron_wallet_core::address::Address;
use tron_wallet_core::config::{Network, TronConfig};
use tron_wallet_core::keys::{derive_keypair, DerivationPath, Language, Mnemonic};
use tron_wallet_core::trc20;
use tron_wallet_core::tx::{builder, sign::sign_tx};
use tron_wallet_core::{SpkiPin, TronGridClient};

// ---------------------------------------------------------------------------
// Constants — single source of truth within this file.
// ---------------------------------------------------------------------------

/// SLIP-44 TRON (coin 195) derivation path. Matches `examples/gen_nile_wallet.rs`.
const SENDER_PATH: &str = "m/44'/195'/0'/0/0";

/// 1 USDT-TRC20 in 6-decimal base units (1 × 10^6 = 1_000_000).
const ONE_USDT: u64 = 1_000_000;

/// 1 TRX in SUN (1 × 10^6 = 1_000_000). Used by row_1.
const ONE_TRX_SUN: u64 = 1_000_000;

/// Confirmation-poll deadline. Matches the spike's prior 120s window.
const POLL_DEADLINE: Duration = Duration::from_secs(120);

/// Confirmation-poll interval. Matches the spike's prior 3s cadence.
const POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Bundled SPKI pin (Phase 3 §3.7, extracted 2026-09-06).
const NILE_SPKI_PIN_HEX: &str = "e9cc763b176063ea6eed1525dac2542512d9e0bf601e210a14f6aad218a9479f";

/// Hard ceiling for transport-error round-trip; row_4 asserts against this.
const TRANSPORT_ERROR_BUDGET: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Bundled Nile fixture — single on-disk shape used by every test below.
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct TestWallet {
    mnemonic: String,
    address: String,
}

#[derive(serde::Deserialize)]
struct NileFixture {
    test: NileFixtureTest,
}

#[derive(serde::Deserialize)]
#[allow(non_snake_case)]
struct NileFixtureTest {
    #[serde(rename = "sender-tr20")]
    sender_tr20: TestWallet,
    #[serde(rename = "recipient-tr20")]
    recipient_tr20: TestWallet,
}

/// Load the `test.{sender-tr20, recipient-tr20}` blocks from the bundled
/// `tokens/nile.json`. Single source of truth within this file.
fn load_nile_fixture() -> NileFixture {
    let json = include_str!("../tokens/nile.json");
    serde_json::from_str(json)
        .expect("crates/tron-wallet-core/tokens/nile.json must parse as NileFixture")
}

// ---------------------------------------------------------------------------
// SPKI pin — env-var override beats bundled pin.
// ---------------------------------------------------------------------------

/// Resolve the SPKI pin for the Nile RPC: operator override
/// (`TRON_NILE_SPKI_PIN`, 64 lowercase hex chars) wins, else fall back to
/// the bundled pin extracted 2026-09-06 (Phase 3 §3.7).
fn nile_spki_pin() -> SpkiPin {
    let hex_str = std::env::var("TRON_NILE_SPKI_PIN")
        .ok()
        .unwrap_or_else(|| NILE_SPKI_PIN_HEX.to_string());
    let bytes = hex::decode(&hex_str).expect("SPKI pin must be valid hex");
    assert_eq!(
        bytes.len(),
        32,
        "SPKI pin must be 32 bytes (64 hex chars); got {}",
        bytes.len()
    );
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    SpkiPin::from_bytes(arr)
}

// ---------------------------------------------------------------------------
// Test-local polling helper. `tron-wallet-core` exposes `get_tx_info` as a
// single-shot receipt probe but no high-level waiter; the test owns the
// loop so the deadline + interval stay co-located with the assertion.
// ---------------------------------------------------------------------------

async fn poll_for_confirmation(rpc: &TronGridClient, txid_hex: &str, deadline: Duration) {
    let started = std::time::Instant::now();
    loop {
        match rpc.get_tx_info(txid_hex).await {
            Ok(info) if info.id.is_some() => return,
            Ok(_) => {} // not yet visible — keep polling
            Err(e) => panic!("[trc20_nile] get_tx_info error: {e}"),
        }
        if started.elapsed() > deadline {
            panic!("[trc20_nile] confirmation poll timed out after {deadline:?}; txid={txid_hex}");
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

// ===========================================================================
// Canonical — Phase 4 §4.4. Full TRC-20 transfer path on real Nile.
// ===========================================================================

/// Full TRC-20 transfer path on real Nile testnet. Steps per Phase 4 §4.4:
///   1. Load sender + recipient from bundled `tokens/nile.json`.
///   2. Derive sender keypair via SLIP-44 path `m/44'/195'/0'/0/0`.
///   3. Look up canonical Nile USDT-TRC20 contract via bundled
///      `tokens::by_symbol(Network::Nile, "USDT")`.
///   4. Build SPKI-pinned RPC via `TronGridClient::new(rpc_url,
///      Some(nile_spki_pin()))`.
///   5. Snapshot recipient `balanceOf` BEFORE the transfer
///      (`balance_before`).
///   6. Build + sign TriggerSmartContract via
///      `tx::builder::trc20_transfer` + `tx::sign::sign_tx`.
///   7. Broadcast via `/wallet/broadcasthex` (PR #541 amendment) via
///      `TronGridClient::broadcast`.
///   8. Assert SUCCESS + non-empty txid + locally-computed txid equals
///      network-reported txid (single-SHA-256 per live Nile 2026-09-06).
///   9. Poll confirmation via `get_tx_info` until the tx_id appears
///      (or 120 s timeout).
///  10. Snapshot recipient `balanceOf` AFTER (`balance_after`) and
///      assert **`balance_after - balance_before ≥ ONE_USDT`** — delta-
///      based (not absolute floor) so a pre-funded recipient cannot
///      pass without the broadcast actually moving funds.
#[tokio::test]
#[ignore = "operator-driven per Phase 4 §4.4 — submits live to Nile. cargo test -p tron-wallet-core --test trc20_nile trc20_transfer_full_flow_nile -- --ignored --nocapture"]
async fn trc20_transfer_full_flow_nile() {
    // --- Load sender + recipient from the bundled Nile fixture ---
    let fixture = load_nile_fixture();
    let sender_wallet = fixture.test.sender_tr20;
    let recipient_wallet = fixture.test.recipient_tr20;
    let owner_address = sender_wallet.address.clone();
    let recipient = recipient_wallet.address.clone();
    let mnemonic_phrase = sender_wallet.mnemonic.as_str();

    // --- Look up canonical Nile USDT contract via bundled token registry ---
    let usdt = tron_wallet_core::tokens::by_symbol(Network::Nile, "USDT")
        .expect("Nile USDT must be in bundled token registry");
    let usdt_address = usdt.address.clone();

    eprintln!("[trc20_nile] sender       = {owner_address}");
    eprintln!("[trc20_nile] recipient    = {recipient}");
    eprintln!("[trc20_nile] USDT         = {usdt_address}");
    eprintln!("[trc20_nile] amount       = {ONE_USDT} raw (1 USDT × 10^6)");

    // --- Derive sender keypair from bundled mnemonic ---
    let mnemonic = Mnemonic::from_phrase(mnemonic_phrase, Language::English)
        .expect("bundled sender mnemonic must be a valid BIP-39 phrase");
    let sender_path: DerivationPath = SENDER_PATH.parse().expect("SLIP-44 TRON path must parse");
    let keypair = derive_keypair(&mnemonic, "", &sender_path).expect("derive_keypair must succeed");

    // --- Build SPKI-pinned RPC client ---
    let cfg = TronConfig::for_network(Network::Nile);
    let rpc = TronGridClient::new(&cfg.rpc_url, Some(nile_spki_pin()))
        .expect("SPKI-pinned TronGridClient must build against Nile config");

    // --- Fetch a fresh ref block from the live network ---
    let head = rpc
        .get_now_block()
        .await
        .expect("get_now_block must succeed");
    eprintln!(
        "[trc20_nile] head block   = #{} id={}",
        head.block_number, head.block_id
    );

    // --- Snapshot recipient USDT balance BEFORE the transfer ---
    let balance_before: U256 = trc20::balance_of(&rpc, &usdt_address, &recipient)
        .await
        .expect("balanceOf query (before)");
    eprintln!("[trc20_nile] recipient balanceOf (before) = {balance_before} raw (6-dec)");

    // --- Build + populate the TRC-20 transfer parameters ---
    let amount = U256::from(ONE_USDT);
    let mut params = builder::trc20_transfer(&owner_address, &usdt_address, &recipient, amount)
        .expect("trc20_transfer builder must succeed");
    builder::set_ref_block(&mut params, head.block_number as i64, &head.block_id)
        .expect("set_ref_block must accept head");
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after epoch")
        .as_millis() as i64;
    builder::set_timestamp(&mut params, ts_ms);

    // --- Sign the transfer with the sender's secret ---
    let signed = sign_tx(keypair.secret_bytes(), &params).expect("sign_tx must succeed");
    let local_txid_hex = hex::encode(signed.txid);
    eprintln!("[trc20_nile] local txid   = {local_txid_hex}");

    // --- Broadcast against the live Nile node via /wallet/broadcasthex ---
    let receipt = rpc
        .broadcast(&signed.signed_envelope_hex)
        .await
        .expect("broadcast must reach the Nile node without transport failure");

    assert!(
        receipt.is_success(),
        "[trc20_nile] Nile broadcast rejected: code={:?} message={:?} error={:?}",
        receipt.code,
        receipt.message,
        receipt.error
    );
    let txid_hex = receipt
        .txid
        .as_deref()
        .expect("successful broadcast must include txid");
    assert!(
        !txid_hex.is_empty(),
        "[trc20_nile] successful broadcast returned empty txid"
    );
    // Pin the wire-form txid contract: locally-computed
    // (sha256(raw_bytes) per tx::sign::txid — live Nile verification
    // 2026-09-06) must match the network-reported txid. Regression target
    // for the original single-vs-double SHA-256 confusion (Q2 plan
    // hypothesis was wrong; the actual algorithm is single SHA-256,
    // matching upstream).
    assert_eq!(
        local_txid_hex.to_ascii_lowercase(),
        txid_hex.to_ascii_lowercase(),
        "[trc20_nile] locally-computed txid {local_txid_hex} != network-reported txid {txid_hex} \
         — local txid computation regressed? See tx::sign::txid = sha256(raw_bytes) per live Nile verification 2026-09-06."
    );
    eprintln!("[trc20_nile] Nile txid    = {txid_hex}");

    // --- Poll for confirmation ---
    poll_for_confirmation(&rpc, txid_hex, POLL_DEADLINE).await;
    eprintln!("[trc20_nile] confirmed    ≤{POLL_DEADLINE:?}");

    // --- Snapshot recipient USDT balance AFTER + assert strict delta ---
    let balance_after: U256 = trc20::balance_of(&rpc, &usdt_address, &recipient)
        .await
        .expect("balanceOf query (after)");
    eprintln!("[trc20_nile] recipient balanceOf (after)  = {balance_after} raw (6-dec)");
    let delta = balance_after.saturating_sub(balance_before);
    eprintln!(
        "[trc20_nile] recipient balanceOf delta    = {delta} raw (6-dec) \
         (expected ≥ {ONE_USDT})"
    );
    assert!(
        delta >= U256::from(ONE_USDT),
        "[trc20_nile] recipient balanceOf should have grown by ≥{ONE_USDT} raw; \
         before={balance_before} after={balance_after} delta={delta} \
         — broadcast returned SUCCESS but funds did not move on-chain."
    );

    eprintln!("[trc20_nile] PASS — https://nile.tronscan.org/#/transaction/{txid_hex}");
}

// ===========================================================================
// Phase 4 §4.5 row 1 — native TRX transfer on real Nile.
// ===========================================================================

/// Phase 4 §4.5 row 1 — live native TRX transfer on real Nile. Adds the
/// 2026-09-07 pre/post `get_account(balance_sun)` balance assertion on
/// top of the v10-era broadcast-only contract.
///
/// **SPKI pin:** this row does NOT enforce SPKI pinning —
/// `TronGridClient::new` passes `None` for the cert verifier. The
/// canonical row above keeps SPKI pinning via
/// `TronGridClient::new(..., Some(SpkiPin::from_bytes(...)))`.
#[tokio::test]
#[ignore = "Phase 4 §4.5 row 1 — live TRX native transfer on Nile. cargo test -p tron-wallet-core --test trc20_nile row_1_trx_native_transfer_nile -- --ignored --nocapture"]
async fn row_1_trx_native_transfer_nile() {
    let fixture = load_nile_fixture();
    let sender_wallet = fixture.test.sender_tr20;
    let recipient_wallet = fixture.test.recipient_tr20;
    let owner_address = sender_wallet.address.clone();
    let recipient = recipient_wallet.address.clone();
    let mnemonic_phrase = sender_wallet.mnemonic.as_str();
    eprintln!("[row_1] sender (TRX native)    = {owner_address}");
    eprintln!("[row_1] recipient (TRX native) = {recipient}");
    eprintln!("[row_1] amount                 = {ONE_TRX_SUN} SUN (1 TRX)");

    // --- Derive sender keypair from the bundled mnemonic (SLIP-44 TRON path) ---
    let mnemonic = Mnemonic::from_phrase(mnemonic_phrase, Language::English)
        .expect("bundled sender mnemonic must be a valid BIP-39 phrase");
    let sender_path: DerivationPath = SENDER_PATH.parse().expect("SLIP-44 TRON path must parse");
    let keypair = derive_keypair(&mnemonic, "", &sender_path).expect("derive_keypair must succeed");

    // --- Build RPC client (NO SPKI pin — deliberate, mirrors row_1 + row_2 posture) ---
    let cfg = TronConfig::for_network(Network::Nile);
    let rpc = TronGridClient::new(&cfg.rpc_url, None)
        .expect("TronGridClient must build against Nile config");
    let head = rpc
        .get_now_block()
        .await
        .expect("get_now_block must succeed");

    // --- Snapshot sender TRX balance BEFORE the transfer ---
    let sender_balance_before = rpc
        .get_account(&owner_address)
        .await
        .expect("get_account (before) must succeed against Nile")
        .balance_sun;
    eprintln!(
        "[row_1] sender TRX balance (before) = {sender_balance_before} SUN \
         ({} TRX)",
        sender_balance_before / 1_000_000
    );

    // --- Build + populate native TRX transfer parameters (bandwidth-only) ---
    let mut params = builder::trx_transfer(&owner_address, &recipient, ONE_TRX_SUN)
        .expect("trx_transfer builder must succeed");
    builder::set_ref_block(&mut params, head.block_number as i64, &head.block_id)
        .expect("set_ref_block must accept head");
    builder::set_fee_limit(&mut params, 0);
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after epoch")
        .as_millis() as i64;
    builder::set_timestamp(&mut params, ts_ms);

    // --- Sign the transfer with the sender's secret ---
    let signed = sign_tx(keypair.secret_bytes(), &params).expect("sign_tx must succeed");
    let local_txid_hex = hex::encode(signed.txid);
    eprintln!("[row_1] local txid: {local_txid_hex}");

    // --- Broadcast against the live Nile node via /wallet/broadcasthex ---
    let receipt = rpc
        .broadcast(&signed.signed_envelope_hex)
        .await
        .expect("broadcast must reach the Nile node without transport failure");

    assert!(
        receipt.is_success(),
        "[row_1] Nile broadcast rejected: code={:?} message={:?} error={:?}",
        receipt.code,
        receipt.message,
        receipt.error
    );
    let txid_hex = receipt
        .txid
        .as_deref()
        .expect("successful broadcast must include txid");
    assert!(
        !txid_hex.is_empty(),
        "[row_1] successful broadcast returned empty txid"
    );
    assert_eq!(
        local_txid_hex.to_ascii_lowercase(),
        txid_hex.to_ascii_lowercase(),
        "[row_1] locally-computed txid {local_txid_hex} != network-reported txid {txid_hex} \
         — local txid computation regressed? See tx::sign::txid = sha256(raw_bytes) per live Nile verification 2026-09-06."
    );
    eprintln!("[row_1] Nile txid: {txid_hex}");

    // --- Wait for on-chain confirmation BEFORE reading post-balance ---
    // `broadcast()`'s `is_success()` only proves the node accepted the tx
    // into its mempool; for TRX-native bandwidth-only transfers the gap
    // between mempool acceptance and block inclusion is enough to make an
    // immediate `get_account` read the pre-transfer balance. The canonical
    // TRC20 row above already polls; mirror that here so the balance delta
    // reflects a transaction that is actually on-chain.
    poll_for_confirmation(&rpc, txid_hex, POLL_DEADLINE).await;

    // --- Snapshot sender TRX balance AFTER + assert strict delta ---
    let sender_balance_after = rpc
        .get_account(&owner_address)
        .await
        .expect("get_account (after) must succeed against Nile")
        .balance_sun;
    let spent_sun = sender_balance_before.saturating_sub(sender_balance_after);
    eprintln!(
        "[row_1] sender TRX balance (after)  = {sender_balance_after} SUN \
         ({} TRX)",
        sender_balance_after / 1_000_000
    );
    eprintln!(
        "[row_1] sender TRX spent            = {spent_sun} SUN \
         (expected ≥ {ONE_TRX_SUN} SUN for the transfer, \
         remainder = burned bandwidth)"
    );
    assert!(
        spent_sun >= ONE_TRX_SUN,
        "[row_1] sender should have spent ≥{ONE_TRX_SUN} SUN; \
         before={sender_balance_before} after={sender_balance_after} spent={spent_sun} \
         — broadcast returned SUCCESS but funds did not move on-chain."
    );
}

// ===========================================================================
// Phase 4 §4.5 row 2 — Rebroadcast idempotency on live Nile.
// ===========================================================================

/// Phase 4 §4.5 row 2 — re-POST the same signed envelope and assert the
/// network treats it as idempotent. Plan Phase 7 Task 6.8 (V7a).
///
/// Contract (live verified 2026-09-06 on Nile): pure envelope rebroadcast
/// returns `code = "DUP_TRANSACTION_ERROR"`, `message = "Dup transaction."`
/// with the SAME txid echoed back. Sender is NOT double-charged. Pin both
/// branches:
///   - SUCCESS path: txid MUST equal the first broadcast's txid (no
///     double-charge). Any other txid = bug.
///   - non-SUCCESS path: code OR message MUST mention `DUP_TRANSACTION`.
///     A generic `FAILED` or empty rejection is a regression.
///
/// **SPKI pin:** this row does NOT enforce SPKI pinning —
/// `TronGridClient::new` passes `None` for the cert verifier (mirrors
/// `row_1_trx_native_transfer_nile`). Idempotency is a wire-protocol
/// invariant; the cert verifier does not affect it.
///
/// Spending: 1 USDT-TRC20 (single on-chain transfer; second POST is
/// short-circuited by the node).
#[tokio::test]
#[ignore = "Phase 4 §4.5 row 2 — rebroadcast idempotency. cargo test -p tron-wallet-core --test trc20_nile row_2_broadcast_rebroadcast_idempotency_nile -- --ignored --nocapture"]
async fn row_2_broadcast_rebroadcast_idempotency_nile() {
    // --- Load sender + recipient + Nile USDT contract from bundled fixture ---
    let fixture = load_nile_fixture();
    let sender_wallet = fixture.test.sender_tr20;
    let recipient_wallet = fixture.test.recipient_tr20;
    let owner_address = sender_wallet.address.clone();
    let recipient = recipient_wallet.address.clone();
    let mnemonic_phrase = sender_wallet.mnemonic.as_str();
    eprintln!("[row_2] sender    = {owner_address}");
    eprintln!("[row_2] recipient = {recipient}");

    let usdt = tron_wallet_core::tokens::by_symbol(Network::Nile, "USDT")
        .expect("Nile USDT must be in bundled token registry");
    let usdt_address = usdt.address.clone();
    eprintln!("[row_2] USDT      = {usdt_address}");

    // --- Derive sender keypair (SLIP-44 TRON path) ---
    let mnemonic = Mnemonic::from_phrase(mnemonic_phrase, Language::English)
        .expect("bundled sender mnemonic must be a valid BIP-39 phrase");
    let sender_path: DerivationPath = SENDER_PATH.parse().expect("SLIP-44 TRON path must parse");
    let keypair = derive_keypair(&mnemonic, "", &sender_path).expect("derive_keypair must succeed");

    // --- Build RPC client (NO SPKI pin — matches row_1 posture) ---
    let cfg = TronConfig::for_network(Network::Nile);
    let rpc = TronGridClient::new(&cfg.rpc_url, None)
        .expect("TronGridClient must build against Nile config");
    let head = rpc
        .get_now_block()
        .await
        .expect("get_now_block must succeed");
    eprintln!(
        "[row_2] head block = #{} id={}",
        head.block_number, head.block_id
    );

    // --- Build + sign one USDT transfer envelope ---
    let amount = U256::from(ONE_USDT);
    let mut params = builder::trc20_transfer(&owner_address, &usdt_address, &recipient, amount)
        .expect("trc20_transfer builder must succeed");
    builder::set_ref_block(&mut params, head.block_number as i64, &head.block_id)
        .expect("set_ref_block must accept head");
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after epoch")
        .as_millis() as i64;
    builder::set_timestamp(&mut params, ts_ms);
    let signed = sign_tx(keypair.secret_bytes(), &params).expect("sign_tx must succeed");
    let local_txid_hex = hex::encode(signed.txid);
    eprintln!("[row_2] local txid = {local_txid_hex}");

    // --- Snapshot recipient USDT balance BEFORE any broadcast ---
    //
    // This MUST be captured pre-broadcast, not after. The single-broadcast
    // already credits the recipient on-chain; capturing after the broadcast
    // would measure the rebroadcast's delta (which is 0 by the idempotency
    // contract), masking any EVM-revert on the first broadcast.
    let balance_before: U256 = trc20::balance_of(&rpc, &usdt_address, &recipient)
        .await
        .expect("balanceOf (before) must succeed");
    eprintln!("[row_2] recipient balanceOf (before) = {balance_before} raw");

    // --- First broadcast (must succeed) ---
    let receipt1 = rpc
        .broadcast(&signed.signed_envelope_hex)
        .await
        .expect("first broadcast must reach the Nile node without transport failure");
    assert!(
        receipt1.is_success(),
        "[row_2] first broadcast rejected: code={:?} message={:?} error={:?}",
        receipt1.code,
        receipt1.message,
        receipt1.error
    );
    let txid1 = receipt1
        .txid
        .as_deref()
        .expect("first successful broadcast must include txid")
        .to_owned();
    eprintln!("[row_2] first  Nile txid = {txid1}");

    // Pin the wire-form txid contract: locally-computed
    // (sha256(raw_bytes) per tx::sign::txid — live Nile verification
    // 2026-09-06) must match the first broadcast's network-reported txid.
    assert_eq!(
        local_txid_hex.to_ascii_lowercase(),
        txid1.to_ascii_lowercase(),
        "[row_2] locally-computed txid {local_txid_hex} != first-broadcast network txid {txid1} \
         — local txid computation regressed? See tx::sign::txid = sha256(raw_bytes) per live Nile verification 2026-09-06."
    );

    // --- Re-POST the SAME signed envelope ---
    let receipt2 = rpc
        .broadcast(&signed.signed_envelope_hex)
        .await
        .expect("rebroadcast must reach the Nile node without transport failure");
    eprintln!(
        "[row_2] second code={:?} message={:?} txid={:?}",
        receipt2.code, receipt2.message, receipt2.txid
    );

    // Idempotency contract. Pin both branches — see doc-comment above.
    let same_txid = receipt2.txid.as_deref() == Some(txid1.as_str());
    if receipt2.is_success() {
        assert!(
            same_txid,
            "[row_2] rebroadcast returned SUCCESS with a different txid (potential double-charge): \
             first={txid1} second={:?}",
            receipt2.txid
        );
    } else {
        let code = receipt2.code.as_deref().unwrap_or_default();
        let msg = receipt2.message.as_deref().unwrap_or_default();
        assert!(
            code.contains("DUP_TRANSACTION_ERROR")
                || msg.to_ascii_uppercase().contains("DUP TRANSACTION"),
            "[row_2] rebroadcast failure does not look like DUP_TRANSACTION_ERROR \
             (operator should re-investigate canonical node contract): code={code:?} message={msg:?}",
        );
    }

    // --- Poll for confirmation so we can assert NO double-charge on-chain ---
    poll_for_confirmation(&rpc, &txid1, POLL_DEADLINE).await;
    eprintln!("[row_2] confirmed ≤{POLL_DEADLINE:?}");

    // --- Verify the first broadcast actually EXECUTED, not just appeared in a block ---
    //
    // EVM reverts (out-of-energy, contract condition, etc.) leave the tx
    // on-chain with a populated `id` but an empty/failed `contract_result`.
    // `poll_for_confirmation` only checks `info.id.is_some()` — true for any
    // on-chain tx, reverted or not. Re-fetch and verify both:
    //   1. `block_number.is_some()` — mined into a block.
    //   2. `contract_result` does not contain "FAILED" / "REVERT".
    // The balance-delta assertion below is the ground truth; this block is
    // diagnostics so a future failure names the cause directly.
    let tx_info = rpc
        .get_tx_info(&txid1)
        .await
        .expect("get_tx_info after poll must succeed");
    eprintln!(
        "[row_2] tx_info: block_number={:?} contract_result={:?} fee={:?}",
        tx_info.block_number, tx_info.contract_result, tx_info.fee
    );
    assert!(
        tx_info.block_number.is_some(),
        "[row_2] tx {txid1} appeared in mempool but never mined into a block; \
         https://nile.tronscan.org/#/transaction/{txid1}"
    );
    for entry in &tx_info.contract_result {
        let upper = entry.to_ascii_uppercase();
        assert!(
            !upper.contains("FAILED") && !upper.contains("REVERT"),
            "[row_2] tx {txid1} mined but contract_result entry {entry:?} indicates EVM revert \
             (likely out-of-energy — default fee_limit may be insufficient); \
             https://nile.tronscan.org/#/transaction/{txid1}"
        );
    }

    // --- Snapshot recipient USDT balance AFTER — strict single-credit invariant ---
    let balance_after: U256 = trc20::balance_of(&rpc, &usdt_address, &recipient)
        .await
        .expect("balanceOf (after) must succeed");
    let delta = balance_after.saturating_sub(balance_before);
    eprintln!("[row_2] recipient balanceOf (after)  = {balance_after} raw");
    eprintln!(
        "[row_2] recipient balanceOf delta     = {delta} raw \
         (expected exactly {ONE_USDT} — no double-charge)"
    );
    assert!(
        delta >= U256::from(ONE_USDT),
        "[row_2] recipient balanceOf should have grown by ≥{ONE_USDT} raw; \
         before={balance_before} after={balance_after} delta={delta} \
         — first broadcast reported SUCCESS but funds did not move on-chain."
    );
    assert!(
        delta < U256::from(ONE_USDT * 2),
        "[row_2] recipient balanceOf grew by {delta} raw — expected exactly {ONE_USDT}, \
         got ≥ 2×{ONE_USDT} = {} raw — rebroadcast double-charged the sender.",
        ONE_USDT * 2
    );
    eprintln!("[row_2] PASS — no double-charge; txid={txid1}");
}

// ===========================================================================
// Phase 4 §4.5 row 3 — Mobile FFI smoke (deferred stub).
// ===========================================================================

/// Phase 4 §4.5 row 3 — Mobile FFI smoke. Requires the FFI cdylib surface
/// (iOS Simulator: `cargo build --target aarch64-apple-ios-sim`; Android
/// Emulator: `cargo ndk -t x86_64 -o jniLibs`) plus a Dart binding driving
/// `libtron_wallet_core` on a real device or emulator, plus the Phase 5
/// PAL/crypto wiring. **Out of scope for the core test harness today** —
/// the body keeps the fixture load so the FFI binding drops in once the
/// cdylib surface lands.
#[tokio::test]
#[ignore = "Phase 4 §4.5 row 3 — Mobile FFI smoke. Out of core-harness scope; ships with Phase 5 PAL + FFI cdylib surface."]
async fn row_3_mobile_ffi_nile() {
    let fixture = load_nile_fixture();
    eprintln!(
        "[row_3] deferred to Phase 5 — would consume fixture \
         sender={}, recipient={}",
        fixture.test.sender_tr20.address, fixture.test.recipient_tr20.address
    );
}

// ===========================================================================
// Phase 4 §4.5 row 4 — Network failure recovery.
// ===========================================================================

/// Phase 4 §4.5 row 4 — Network failure recovery. Point `TronGridClient`
/// at a closed port (`http://127.0.0.1:9999`) and assert the RPC call
/// surfaces a transport error within `TRANSPORT_ERROR_BUDGET` (30 s).
/// reqwest's default connect timeout is well under that — typically
/// <1 s for an immediate `ECONNREFUSED`. Maps to the production CLI
/// retry policy: transport error → exit code 3, never panic.
#[tokio::test]
#[ignore = "Phase 4 §4.5 row 4 — Network failure recovery. Same `--ignored` opt-in as canonical; asserts no panic / no hang + transport error surface."]
async fn row_4_network_failure_recovery_nile() {
    // Construct an http:// (unpinned) client pointed at a closed port on
    // loopback. `127.0.0.1:9999` has no listener — kernel returns
    // `ECONNREFUSED` immediately.
    let rpc = TronGridClient::new("http://127.0.0.1:9999", None)
        .expect("[row_4] TronGridClient must accept http://127.0.0.1:9999");

    // Probe `balanceOf` against the closed port — measure end-to-end
    // latency so we can prove the "no hang" half of the contract.
    let fixture = load_nile_fixture();
    let recipient = fixture.test.recipient_tr20.address.clone();
    let usdt = tron_wallet_core::tokens::by_symbol(Network::Nile, "USDT")
        .expect("Nile USDT must be in bundled token registry");
    let usdt_address = usdt.address.clone();

    let started = std::time::Instant::now();
    let outcome = trc20::balance_of(&rpc, &usdt_address, &recipient).await;
    let elapsed = started.elapsed();
    eprintln!("[row_4] elapsed = {elapsed:?}, outcome = {outcome:?}");

    let err = outcome.expect_err(
        "closed port must surface an error — a successful return would imply \
         the local loopback unexpectedly serves Nile's TRC-20 RPC",
    );
    eprintln!("[row_4] surfaced error: {err}");
    // Spec: no panic, no hang within 30s. The actual reqwest connect timeout
    // is shorter (default ~10s), and `ECONNREFUSED` is sub-millisecond.
    assert!(
        elapsed < TRANSPORT_ERROR_BUDGET,
        "[row_4] transport error must surface within {TRANSPORT_ERROR_BUDGET:?} (no hang), took {elapsed:?}"
    );
    eprintln!(
        "[row_4] PASS — transport error propagated, no panic, no hang within {TRANSPORT_ERROR_BUDGET:?}"
    );
}

// ===========================================================================
// Sanity — SLIP-44 derivation determinism.
// ===========================================================================

/// Sanity: a known-test mnemonic produces a deterministic T-address via
/// SLIP-44 path `m/44'/195'/0'/0/0`. Verifies `derive_keypair` +
/// `Address::from_public_key` end-to-end (the path the canonical flow
/// relies on for `owner_address` derivation when not pinned by fixture).
#[test]
fn derive_sender_produces_t_address_with_known_phrase() {
    use sha2::Digest;

    let phrase = "abandon abandon abandon abandon abandon abandon \
                  abandon abandon abandon abandon abandon about";
    let mnemonic = Mnemonic::from_phrase(phrase, Language::English)
        .expect("known test mnemonic must parse as BIP-39");
    let path: DerivationPath = SENDER_PATH.parse().expect("SLIP-44 TRON path must parse");
    let keypair = derive_keypair(&mnemonic, "", &path).expect("derive_keypair must succeed");

    let addr = Address::from_public_key(keypair.public_key())
        .expect("Address from public key must succeed");
    let t = addr.to_base58();

    assert!(t.starts_with('T'), "derived address must start with T: {t}");
    assert_eq!(t.len(), 34, "T-address must be 34 chars (base58check): {t}");

    // Determinism — same mnemonic + path → same address.
    let keypair2 = derive_keypair(&mnemonic, "", &path).expect("derive_keypair must succeed");
    let addr2 = Address::from_public_key(keypair2.public_key())
        .expect("Address from public key must succeed");
    assert_eq!(t, addr2.to_base58(), "derivation must be deterministic");

    // Pre-image SHA-256 over the address bytes — guards against accidental
    // upstream changes in anychain-kms derivation drift.
    let hash = sha2::Sha256::digest(t.as_bytes());
    let hex = hex::encode(&hash[..8]);
    eprintln!("[trc20_nile] sanity T-address = {t}; sha256[:8] = {hex}");
}
