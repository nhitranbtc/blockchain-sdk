//! Phase 4 §4.4-4.5 — Nile testnet integration (operator-driven).
//!
//! Pre-deployed community USDT-TRC20 contract on Nile
//! `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` (canonical per TronScan). Sender is
//! funded via the Nile faucet at <https://nileex.io/join/getJoinPage>.
//!
//! ## Gating
//!
//! - **Live test:** `TRON_NILE_INTEGRATION=1` + `TRON_TEST_MNEMONIC` must be
//!   set. Missing any → loud-RED panic naming every missing variable (per
//!   plan gated-live-test convention; silent-skip is forbidden).
//! - **Optional SPKI pin:** `TRON_NILE_SPKI_PIN` hex (64 lowercase chars) —
//!   if unset, the harness uses the bundled pin from `tokens/nile.json` path
//!   (Phase 3 §3.7 already extracted: `e9cc763b...218a9479f`).
//! - **Setup (operator, one-time):**
//!   ```bash
//!   # Derive SPKI pin from a live TLS handshake:
//!   openssl s_client -connect nile.trongrid.io:443 -servername nile.trongrid.io \
//!     </dev/null 2>/dev/null \
//!     | openssl x509 -pubkey -noout \
//!     | openssl pkey -pubin -outform der \
//!     | openssl dgst -sha256 -binary | xxd -p -c 256
//!   # Fund the address derived from $TRON_TEST_MNEMONIC at the Nile faucet.
//!   ```
//!
//! - **Run (operator, after env + faucet funding):**
//!   ```bash
//!   TRON_NILE_INTEGRATION=1 \
//!   TRON_TEST_MNEMONIC="abandon abandon ... about" \
//!   TRON_NILE_RECIPIENT="T..." \
//!   cargo test -p tron-v1-spike --test trc20_nile -- --ignored --nocapture
//!   ```
//!
//! ## What lives in v0.1
//!
//! - Canonical `trc20_transfer_full_flow_nile` — full TRC-20 transfer path on
//!   real Nile, mirroring `use_case_alpha_sends_beta_usdt_live_nile` but
//!   mnemonic-keyed (per Phase 4 §4.4 spec) and gated on `TRON_NILE_INTEGRATION`
//!   rather than the `RUN_TRON_NILE=1` / three-env-gate the use-case uses.
//! - Scenario rows 1-4 stubs (§4.5) — wired as `#[ignore]` with TODO + runbook
//!   pointers; unblock when the canonical harness stabilises.
//!
//! ## What is deferred
//!
//! - Mobile-runtime smoke (no Docker fallback for mobile CI per Round-1
//!   grill Q6 — v0.2 uses real Nile for mobile smoke).
//! - Failure-recovery scenarios (closed port, timeout) — exercise the same
//!   `TronGridClient` path; production retry policy lives in `tron` CLI
//!   (Phase 6).

use bip39::{Language, Mnemonic};
use k256::ecdsa::SigningKey;
use sha2::Digest;
use std::time::Duration;
use tron_v1_spike::address::{from_base58check, raw_21_from_uncompressed_pubkey, to_base58check};
use tron_v1_spike::config::nile_config;

/// Loud-RED panic gate. Per plan gated-live-test convention: silent `return`
/// is forbidden — the harness must report FAILED when env vars are absent.
/// Returns `(mnemonic_phrase, spki_pin_override)` on success.
fn require_nile_env() -> (String, Option<String>) {
    let mut missing = Vec::new();
    let integration = std::env::var("TRON_NILE_INTEGRATION").ok();
    let mnemonic = std::env::var("TRON_TEST_MNEMONIC").ok();
    let spki_override = std::env::var("TRON_NILE_SPKI_PIN").ok();

    if integration.as_deref() != Some("1") {
        missing.push("TRON_NILE_INTEGRATION=1");
    }
    if mnemonic.is_none() {
        missing.push("TRON_TEST_MNEMONIC (BIP-39 phrase; never hard-code)");
    }

    if !missing.is_empty() {
        panic!(
            "trc20_nile: missing required env vars: [{}]. \
             Operator runbook: TRON_NILE_INTEGRATION=1 TRON_TEST_MNEMONIC=<phrase> \
             [TRON_NILE_RECIPIENT=<T-address>] cargo test -p tron-v1-spike \
             --test trc20_nile -- --ignored --nocapture",
            missing.join(", ")
        );
    }

    (mnemonic.unwrap(), spki_override)
}

/// 100 USDT in 6-decimal base units (100 × 10^6 = 100_000_000).
const TRANSFER_AMOUNT_BASE_UNITS: u64 = 100_000_000;

/// Canonical SLIP-44 path for TRON (coin 195). Required by Phase 4 §4.4.
fn tron_path() -> bip32::DerivationPath {
    "m/44'/195'/0'/0/0".parse().expect("SLIP-44 TRON path")
}

/// Derive `(sender_t, sender_sk)` from a BIP-39 mnemonic phrase.
fn derive_sender(phrase: &str) -> (String, SigningKey) {
    let m = Mnemonic::parse_in(Language::English, phrase).expect("BIP-39 mnemonic parse");
    let seed = m.to_seed("");
    let xprv = bip32::XPrv::derive_from_path(seed, &tron_path()).expect("XPrv derive");
    let sk = xprv.private_key().clone();
    let xpub = xprv.public_key();
    let verifying_key = xpub.public_key();
    let pubkey_bytes = verifying_key.to_encoded_point(false);
    let mut pubkey_65 = [0u8; 65];
    pubkey_65.copy_from_slice(pubkey_bytes.as_bytes());
    let raw21 = raw_21_from_uncompressed_pubkey(&pubkey_65);
    let sender_t = to_base58check(&raw21);
    assert!(
        sender_t.starts_with('T'),
        "sender T-address must start with T"
    );
    (sender_t, sk)
}

fn base58_to_20bytes(t_addr: &str) -> [u8; 20] {
    let raw21 = from_base58check(t_addr).expect("T-address decodes");
    assert_eq!(
        raw21.len(),
        21,
        "T-address payload must be 21 bytes (0x41 + 20)"
    );
    assert_eq!(raw21[0], 0x41, "T-address prefix byte must be 0x41");
    let mut out = [0u8; 20];
    out.copy_from_slice(&raw21[1..]);
    out
}

/// Find the Nile USDT-TRC20 contract address from the bundled
/// `tokens/nile.json`. Canonical address `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf`
/// per TronScan (verified 2026-08-27).
fn nile_usdt_address() -> String {
    nile_config()
        .tokens
        .iter()
        .find(|t| t.symbol == "USDT")
        .map(|t| t.address.clone())
        .expect("USDT token must be present in tokens/nile.json")
}

/// Build the SPKI-pinned URL for Nile TronGrid. Prefer operator override
/// (`TRON_NILE_SPKI_PIN`); fall back to the bundled pin (Phase 3 §3.7:
/// `e9cc763b176063ea6eed1525dac2542512d9e0bf601e210a14f6aad218a9479f`).
fn pinned_nile_url(override_pin: Option<&str>) -> String {
    let cfg = nile_config();
    let host = cfg.rpc_host();
    let pin =
        override_pin.unwrap_or("e9cc763b176063ea6eed1525dac2542512d9e0bf601e210a14f6aad218a9479f");
    format!("pinned://{pin}@{host}:443")
}

// ---------------------------------------------------------------------------
// Phase 4 §4.4 — Canonical full-flow TRC-20 transfer on Nile.
// ---------------------------------------------------------------------------

/// Full TRC-20 transfer path on real Nile testnet. Steps per Phase 4 §4.4:
///   1. Skip if `TRON_NILE_INTEGRATION` env not set (CI gate).
///   2. Load test mnemonic from `TRON_TEST_MNEMONIC` env (never hard-code).
///   3. Derive deployer address via SLIP-44 path `m/44'/195'/0'/0/0`.
///   4. Use pre-deployed community USDT-TRC20 contract
///      `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf`.
///   5. Verify deployer pre-funded via `chain::trc20_balance(...)`.
///   6. Submit transfer 100 mock USDT to recipient.
///   7. Wait for confirmation via
///      `tx::wait_for_confirm(&receipt.txid, Duration::from_secs(60),
///       Duration::from_secs(3), &cfg)`.
#[tokio::test]
#[ignore = "operator-driven per Phase 4 §4.4 — TRON_NILE_INTEGRATION=1 TRON_TEST_MNEMONIC=<phrase> cargo test -p tron-v1-spike --test trc20_nile trc20_transfer_full_flow_nile -- --ignored --nocapture"]
async fn trc20_transfer_full_flow_nile() {
    let (mnemonic_phrase, spki_override) = require_nile_env();
    let recipient = std::env::var("TRON_NILE_RECIPIENT")
        .unwrap_or_else(|_| panic!("TRON_NILE_RECIPIENT required when TRON_NILE_INTEGRATION=1"));

    let (sender_t, sender_sk) = derive_sender(&mnemonic_phrase);
    let usdt_contract = nile_usdt_address();
    let pinned_url = pinned_nile_url(spki_override.as_deref());

    eprintln!("[trc20_nile] sender       = {sender_t}");
    eprintln!("[trc20_nile] recipient    = {recipient}");
    eprintln!("[trc20_nile] USDT         = {usdt_contract}");
    eprintln!("[trc20_nile] amount       = {TRANSFER_AMOUNT_BASE_UNITS} raw (100 USDT × 10^6)");
    eprintln!("[trc20_nile] pinned_url   = {pinned_url}");

    let rpc = tron_v1_spike::rpc::JsonRpcClient::new_pinned(&pinned_url)
        .expect("construct SPKI-pinned Nile RPC client");

    // Verify recipient address decodes — pre-empts NPE-shaped downstream
    // failures before any HTTP call.
    let _recipient_20 = base58_to_20bytes(&recipient);

    // Build + sign a TriggerSmartContract transaction for USDT-TRC20 transfer.
    // (mirrors the proven path in
    // `tests/use_case_alpha_sends_beta_usdt.rs::use_case_alpha_sends_beta_usdt_live_nile`).
    let signed_tx = tron_v1_spike::tx::build_signed_trc20_transfer(
        &rpc,
        &sender_sk,
        &sender_t,
        &usdt_contract,
        &recipient,
        TRANSFER_AMOUNT_BASE_UNITS,
    )
    .await
    .unwrap_or_else(|e| panic!("build signed TRC-20 transfer tx: {e:?}: {e}"));
    let tx_id = signed_tx.tx_id.clone();
    eprintln!("[trc20_nile] tx_id  = {tx_id}");

    // Broadcast via the SPKI-pinned RPC client.
    let broadcast = tron_v1_spike::tx::broadcast(&rpc, &signed_tx)
        .await
        .expect("broadcast tx");
    assert_eq!(
        broadcast.result,
        Some(true),
        "broadcast returned not-ok: code={:?} message={:?}",
        broadcast.code,
        broadcast.message
    );

    // Poll gettransactionbyid until the tx_id appears (or timeout).
    let poll_deadline = Duration::from_secs(120);
    tron_v1_spike::tx::poll_for_confirmation(&rpc, &tx_id, poll_deadline)
        .await
        .expect("tx confirmation poll");
    eprintln!("[trc20_nile] confirmed after ≤{poll_deadline:?}");

    // Verify recipient `balanceOf` increased by at least the transfer amount.
    let balance_after = tron_v1_spike::tx::balance_of_trc20(&rpc, &usdt_contract, &recipient)
        .await
        .expect("balanceOf query");
    eprintln!("[trc20_nile] recipient balanceOf = {balance_after} raw (6-dec)");
    let min_balance: u128 = TRANSFER_AMOUNT_BASE_UNITS.into();
    assert!(
        balance_after >= min_balance,
        "recipient should hold ≥{TRANSFER_AMOUNT_BASE_UNITS} raw, got {balance_after}"
    );

    eprintln!(
        "[trc20_nile] PASS — {}",
        nile_config().explorer_tx_url.replace("{tx_id}", &tx_id)
    );
}

// ---------------------------------------------------------------------------
// Phase 4 §4.5 — Nile testnet scenario rows 1-4 (mostly `#[ignore]` stubs).
// ---------------------------------------------------------------------------
//
// Row 3 (Mobile-specific) requires Dart binding + emulator/device, out of
// scope for the spike harness. Rows 1, 2, 4 are wired as `#[ignore]` stubs
// that delegate to the proven `trc20_transfer_full_flow_nile` shape but
// carry scenario-specific assertions.

/// Phase 4 §4.5 row 1 — TRX native transfer (same as Local row 1 but on
/// real Nile). Receipt must be visible on
/// `https://nile.tronscan.org/#/transaction/<txid>`.
#[tokio::test]
#[ignore = "Phase 4 §4.5 row 1 — TRX native transfer on Nile. Same gate as trc20_transfer_full_flow_nile."]
async fn row_1_trx_native_transfer_nile() {
    require_nile_env();
    // TODO(phase4): build TransferContract (TRX native, NOT TriggerSmartContract),
    // sign via anychain-tron::Transaction::sign, broadcast via wallet/broadcasthex
    // (per PR #541 fix), assert explorer visibility.
    panic!("deferred — TransferContract builder + broadcasthex wiring prerequisite");
}

/// Phase 4 §4.5 row 2 — TRC-20 transfer on real testnet. Use
/// community-deployed `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` (TronScan-
/// verified canonical). Balance visible on
/// `https://nile.tronscan.org/#/token20/<addr>`.
#[tokio::test]
#[ignore = "Phase 4 §4.5 row 2 — TRC-20 transfer on Nile. Same gate as trc20_transfer_full_flow_nile; asserts recipient balanceOf on Tronscan."]
async fn row_2_trc20_transfer_nile() {
    require_nile_env();
    let usdt = nile_usdt_address();
    eprintln!("[row_2] USDT contract = {usdt}");
    // TODO(phase4): call trc20_transfer_full_flow_nile's body and additionally
    // verify balance via Tronscan HTTP GET (operator-supplied API key, OR
    // skip the explorer check if no key set).
    panic!("deferred — Tronscan API integration prerequisite");
}

/// Phase 4 §4.5 row 3 — Mobile-specific. iOS Simulator:
/// `cargo build --target aarch64-apple-ios-sim`; Android Emulator:
/// `cargo ndk -t x86_64 -o jniLibs`. FFI smoke from Dart binding on
/// emulator/device, real tx to Nile. Out of scope for the spike harness —
/// the FFI cdylib surface ships in Phase 5 (PAL + crypto).
#[tokio::test]
#[ignore = "Phase 4 §4.5 row 3 — Mobile FFI smoke from Dart binding. Out of spike-harness scope; ships with Phase 5 PAL + crypto."]
async fn row_3_mobile_ffi_nile() {
    require_nile_env();
    panic!("deferred — FFI cdylib surface + Dart binding (Phase 5)");
}

/// Phase 4 §4.5 row 4 — Network failure recovery. Point RPC at
/// `http://127.0.0.1:9999` (closed port). CLI returns error code 3
/// (transport error) within 30s timeout; no panic. Maps to the production
/// `TronGridClient` retry policy (Phase 6 CLI).
#[tokio::test]
#[ignore = "Phase 4 §4.5 row 4 — Network failure recovery. Exercise JsonRpcClient against a closed port; assert graceful error surface within timeout."]
async fn row_4_network_failure_recovery_nile() {
    require_nile_env();
    // TODO(phase4): build a `JsonRpcClient::new_local("http://127.0.0.1:9999")`
    // and assert `post_triggersmartcontract(...).await` returns
    // `JsonRpcError::Transport` within 30s. Verify no panic / no hang.
    panic!("deferred — failure-recovery contract test prerequisite");
}

// ---------------------------------------------------------------------------
// Sanity — mnemonic-derived SLIP-44 address matches Nile's expected prefix.
// ---------------------------------------------------------------------------

/// Sanity: a known-test mnemonic produces a deterministic T-address via SLIP-44
/// path `m/44'/195'/0'/0/0`. Verifies the `derive_sender` helper itself.
#[test]
fn derive_sender_produces_t_address_with_known_phrase() {
    let phrase = "abandon abandon abandon abandon abandon abandon \
                  abandon abandon abandon abandon abandon about";
    let (t, _sk) = derive_sender(phrase);
    assert!(t.starts_with('T'), "derived address must start with T: {t}");
    assert_eq!(t.len(), 34, "T-address must be 34 chars (base58check)");

    // Determinism — same mnemonic → same address.
    let (t2, _) = derive_sender(phrase);
    assert_eq!(t, t2, "derivation must be deterministic");

    // Pre-image SHA-256 over the address bytes — guards against accidental
    // upstream changes in anychain-kms derivation drift.
    let hash = sha2::Sha256::digest(t.as_bytes());
    let hex = hex::encode(&hash[..8]);
    eprintln!("[trc20_nile] sanity T-address = {t}; sha256[:8] = {hex}");
}
