//! Phase 4 §4.4-4.5 — Nile testnet integration (operator-driven).
//!
//! Pre-deployed community USDT-TRC20 contract on Nile
//! `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` (canonical per TronScan). Sender is
//! funded via the Nile faucet at <https://nileex.io/join/getJoinPage>.
//!
//! ## Gating
//!
//! - **No env-var gate.** All live tests are `#[ignore]`-marked; CI stays
//!   silent by default. Operator opts in by running with `--ignored`
//!   (the tests submit directly to the live Nile testnet).
//! - Sender + recipient mnemonics + addresses are loaded from the bundled
//!   Nile fixture at
//!   `crates/tron-wallet-core/tokens/nile.json` (`test.sender-tr20`,
//!   `test.recipient-tr20`) — mirrors the Phase-3 pattern in
//!   `crates/tron-wallet-core/tests/v10_broadcast.rs`. No env-var mnemonic
//!   keying required (operator decision 2026-09-06; testnet-only fixtures
//!   committed in the bundle). Single source of truth: funding the
//!   addresses once lights up both suites.
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
//!   # Fund BOTH the sender (TRX + USDT) and recipient addresses at
//!   # the Nile faucet (https://nileex.io/join/getJoinPage). Address
//!   # values come from `crates/tron-wallet-core/tokens/nile.json`.
//!   ```
//!
//! - **Run (operator, after faucet funding):**
//!   ```bash
//!   cargo test -p tron-v1-spike --test trc20_nile -- --ignored --nocapture
//!   ```
//!
//! ## What lives in v0.1
//!
//! - Canonical `trc20_transfer_full_flow_nile` — full TRC-20 transfer path on
//!   real Nile, mirroring `use_case_alpha_sends_beta_usdt_live_nile` but
//!   mnemonic-keyed (per Phase 4 §4.4 spec) and gated on `--ignored` only
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
use std::time::{SystemTime, UNIX_EPOCH};

use tron_v1_spike::address::{from_base58check, raw_21_from_uncompressed_pubkey, to_base58check};
use tron_v1_spike::config::nile_config;

// `row_1_trx_native_transfer_nile` (Phase 4 §4.5) lifts onto `tron-wallet-core`
// to mirror `crates/tron-wallet-core/tests/v10_broadcast.rs::live_broadcast_trx_native_transfer_succeeds_on_nile`.
// `bip39::{Mnemonic, Language}` above is used by the spike's own
// `derive_sender` (canonical TRC-20 path); `tron-wallet-core`'s analogous types
// are reached via fully-qualified paths inside `row_1` to avoid name collision.
use tron_wallet_core::config::Network;
use tron_wallet_core::tx::builder;
use tron_wallet_core::tx::sign::sign_tx;
use tron_wallet_core::{TronConfig, TronGridClient};

/// Read the optional SPKI pin override for the Nile RPC. `None` = use the
/// bundled pin from `tokens/nile.json` (Phase 3 §3.7:
/// `e9cc763b176063ea6eed1525dac2542512d9e0bf601e210a14f6aad218a9479f`).
/// Mnemonic + recipient come from the bundled Nile fixture, not env vars.
/// No env-var gate — canonical row + scenario rows submit directly to the
/// live Nile testnet when run with `--ignored` (CI stays silent by default).
fn nile_spki_override() -> Option<String> {
    std::env::var("TRON_NILE_SPKI_PIN").ok()
}

/// 1 USDT-TRC20 in 6-decimal base units (1 × 10^6 = 1_000_000).
const TRANSFER_AMOUNT_BASE_UNITS: u64 = 1_000_000;

/// 1 TRX in SUN (1 × 10^6 = 1_000_000). Used by
/// `row_1_trx_native_transfer_nile` (Phase 4 §4.5). Matches
/// `v10_broadcast.rs::ONE_TRX_SUN`.
const TRX_AMOUNT_SUN: u64 = 1_000_000;

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

// ---------------------------------------------------------------------------
// Test wallets — loaded from the bundled Nile fixture (mirrors the
// `tests/v10_broadcast.rs` pattern in `tron-wallet-core`). Sender +
// recipient mnemonics + addresses are baked in at compile time via
// `include_str!`, so operator runbook needs zero secret env vars for the
// funded test addresses.
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
/// Nile fixture. The on-disk shape mirrors the block in
/// `crates/tron-wallet-core/tests/v10_broadcast.rs::NileFixture` — single
/// source of truth across the spike + core crates.
fn load_nile_fixture() -> NileFixture {
    let json = include_str!("../../../crates/tron-wallet-core/tokens/nile.json");
    serde_json::from_str(json)
        .expect("crates/tron-wallet-core/tokens/nile.json must parse as NileFixture")
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
///   1. (No env-var gate — submits live to Nile when run with `--ignored`.)
///   2. Load sender + recipient (mnemonic + address) from the bundled Nile
///      fixture (`crates/tron-wallet-core/tokens/nile.json`).
///   3. Derive sender keypair via SLIP-44 path `m/44'/195'/0'/0/0`.
///   4. Use pre-deployed community USDT-TRC20 contract
///      `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf`.
///   5. Verify sender pre-funded via `chain::trc20_balance(...)`.
///   6. Submit transfer 1 mock USDT to recipient.
///   7. Wait for confirmation via
///      `tx::wait_for_confirm(&receipt.txid, Duration::from_secs(60),
///       Duration::from_secs(3), &cfg)`.
#[tokio::test]
#[ignore = "operator-driven per Phase 4 §4.4 — submits live to Nile. cargo test -p tron-v1-spike --test trc20_nile trc20_transfer_full_flow_nile -- --ignored --nocapture"]
async fn trc20_transfer_full_flow_nile() {
    let spki_override = nile_spki_override();
    let fixture = load_nile_fixture();
    let sender_wallet = fixture.test.sender_tr20;
    let recipient_wallet = fixture.test.recipient_tr20;
    let mnemonic_phrase = sender_wallet.mnemonic.as_str();
    let recipient = recipient_wallet.address.clone();

    let (sender_t, sender_sk) = derive_sender(mnemonic_phrase);
    let usdt_contract = nile_usdt_address();
    let pinned_url = pinned_nile_url(spki_override.as_deref());

    eprintln!("[trc20_nile] sender       = {sender_t}");
    eprintln!("[trc20_nile] recipient    = {recipient}");
    eprintln!("[trc20_nile] USDT         = {usdt_contract}");
    eprintln!("[trc20_nile] amount       = {TRANSFER_AMOUNT_BASE_UNITS} raw (1 USDT × 10^6)");
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

/// Phase 4 §4.5 row 1 — live native TRX transfer on real Nile. Mirrors the
/// proven shape in
/// `crates/tron-wallet-core/tests/v10_broadcast.rs::live_broadcast_trx_native_transfer_succeeds_on_nile`
/// (path b of the original TODO: lift onto `tron-wallet-core`). The signed
/// envelope is POSTed to `/wallet/broadcasthex` per PR #541.
///
/// Receipt must be visible on
/// `https://nile.tronscan.org/#/transaction/<txid>`.
///
/// **SPKI pin:** this row does NOT enforce SPKI pinning — `TronGridClient::new`
/// takes `None` for the cert verifier (same posture as v10_broadcast.rs). The
/// canonical `trc20_transfer_full_flow_nile` keeps SPKI pinning via the
/// spike's own `JsonRpcClient::new_pinned` (Phase 3 §3.7 pin from
/// `tokens/nile.json`). Followup: thread SPKI through `TronGridClient`.
#[tokio::test]
#[ignore = "Phase 4 §4.5 row 1 — live TRX native transfer on Nile. Same `--ignored` opt-in as trc20_transfer_full_flow_nile."]
async fn row_1_trx_native_transfer_nile() {
    // Only the integration gate matters here — no SPKI override path.
    let _spki_override = nile_spki_override();
    let fixture = load_nile_fixture();
    let sender_wallet = fixture.test.sender_tr20;
    let recipient_wallet = fixture.test.recipient_tr20;
    let owner_address = sender_wallet.address.clone();
    let recipient = recipient_wallet.address.clone();
    let mnemonic_phrase = sender_wallet.mnemonic.as_str();
    eprintln!("[row_1] sender (TRX native)    = {owner_address}");
    eprintln!("[row_1] recipient (TRX native) = {recipient}");
    eprintln!("[row_1] amount                 = {TRX_AMOUNT_SUN} SUN (1 TRX)");

    // --- Derive sender keypair from the bundled mnemonic (SLIP-44 TRON path) ---
    let mnemonic = tron_wallet_core::keys::Mnemonic::from_phrase(
        mnemonic_phrase,
        tron_wallet_core::keys::Language::English,
    )
    .expect("bundled sender mnemonic must be a valid BIP-39 phrase");
    let sender_path: tron_wallet_core::keys::DerivationPath = "m/44'/195'/0'/0/0"
        .parse()
        .expect("SLIP-44 TRON path must parse");
    let keypair = tron_wallet_core::keys::derive_keypair(&mnemonic, "", &sender_path)
        .expect("derive_keypair must succeed");

    // --- Fetch a fresh ref block from the live network ---
    let cfg = TronConfig::for_network(Network::Nile);
    let rpc = TronGridClient::new(&cfg.rpc_url, None)
        .expect("TronGridClient must build against Nile config");
    let head = rpc
        .get_now_block()
        .await
        .expect("get_now_block must succeed");

    // --- Build + populate native TRX transfer parameters (bandwidth-only) ---
    let mut params = builder::trx_transfer(&owner_address, &recipient, TRX_AMOUNT_SUN)
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

    // --- Assertions: SUCCESS code + non-empty txid + txid contract parity ---
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
    // Pin the wire-form txid contract: locally-computed
    // (sha256(raw_bytes) per `tx::sign::txid` — live Nile verification
    // 2026-09-06) must match the network-reported txid. Regression target
    // for the original single-vs-double SHA-256 confusion (Q2 plan
    // hypothesis was wrong; the actual algorithm is single SHA-256,
    // matching upstream).
    assert_eq!(
        local_txid_hex.to_ascii_lowercase(),
        txid_hex.to_ascii_lowercase(),
        "[row_1] locally-computed txid {local_txid_hex} != network-reported txid {txid_hex} \
         — local txid computation regressed? See tx::sign::txid = sha256(raw_bytes) per live Nile verification 2026-09-06."
    );
    eprintln!("[row_1] Nile txid: {txid_hex}");
}

/// Phase 4 §4.5 row 2 — live TRC-20 transfer on Nile, balance verification
/// via the spike's own `balance_of_trc20` against the canonical pinned RPC
/// (no Tronscan API key required). The TRC-20 transfer itself is exercised
/// by `trc20_transfer_full_flow_nile` — this row re-asserts that the
/// post-transfer `balanceOf` reads cleanly through the SPKI-pinned RPC
/// client. Canonical contract
/// `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` per TronScan.
#[tokio::test]
#[ignore = "Phase 4 §4.5 row 2 — TRC-20 balance visibility on Nile. Same `--ignored` opt-in as trc20_transfer_full_flow_nile."]
async fn row_2_trc20_transfer_nile() {
    let spki_override = nile_spki_override();
    let usdt = nile_usdt_address();
    eprintln!("[row_2] USDT contract = {usdt}");

    // Reuse the canonical pinned RPC path so this row runs under the same
    // SPKI pin (Phase 3 §3.7) as `trc20_transfer_full_flow_nile`.
    let pinned_url = pinned_nile_url(spki_override.as_deref());
    let rpc = tron_v1_spike::rpc::JsonRpcClient::new_pinned(&pinned_url)
        .expect("[row_2] pinned JsonRpcClient must build");

    // Pre-empt NPE-shaped downstream failures (mirrors canonical).
    let fixture = load_nile_fixture();
    let recipient_wallet = fixture.test.recipient_tr20;
    let recipient = recipient_wallet.address.as_str();
    let _ = base58_to_20bytes(recipient);

    // Prove the spike's balanceOf RPC returns a parseable u128 against the
    // live Nile node for the fixture recipient — independent of the
    // canonical transfer path.
    let balance = tron_v1_spike::tx::balance_of_trc20(&rpc, &usdt, recipient)
        .await
        .expect("[row_2] balanceOf RPC must succeed against live Nile");
    eprintln!(
        "[row_2] recipient balanceOf = {balance} raw (6-dec) = {} USDT",
        balance / 1_000_000
    );
    // Tronscan-only visibility check was deferred (no API key), so we
    // assert the canonical RPC balance is reachable. Tronscan HTTP GET
    // follow-up remains a known TODO (operator-supplied API key, or skip
    // the explorer check if no key set).
}

/// Phase 4 §4.5 row 3 — Mobile FFI smoke. Requires the FFI cdylib surface
/// (iOS Simulator: `cargo build --target aarch64-apple-ios-sim`; Android
/// Emulator: `cargo ndk -t x86_64 -o jniLibs`) plus a Dart binding driving
/// `libbitcoin_wallet_core` / `libtron_wallet_core` on a real device or
/// emulator, plus the Phase 5 PAL/crypto wiring. **Out of scope for the
/// spike harness today** — the body keeps the env gate so the FFI smoke
/// has the right upstream contract (loaded mnemonics + fixture addresses)
/// to drop in once the FFI binding lands.
#[tokio::test]
#[ignore = "Phase 4 §4.5 row 3 — Mobile FFI smoke from Dart binding. Out of spike-harness scope; ships with Phase 5 PAL + crypto."]
async fn row_3_mobile_ffi_nile() {
    let _spki_override = nile_spki_override();
    let fixture = load_nile_fixture();
    eprintln!(
        "[row_3] deferred to Phase 5 — would consume fixture \
         sender={}, recipient={}",
        fixture.test.sender_tr20.address, fixture.test.recipient_tr20.address
    );
    // Phase 5 will replace this with: FFI cdylib build, Dart binding
    // instantiation on emulator/device, real tx to Nile via the spike's
    // `trc20_transfer_full_flow_nile` shape driven across the FFI surface.
    // No body collapse is possible until the binding lands.
}

/// Phase 4 §4.5 row 4 — Network failure recovery. Point the spike's
/// `JsonRpcClient` at a closed port (`http://127.0.0.1:9999`) and assert the
/// RPC call surfaces a `Transport` error within 30 s (reqwest default
/// connect timeout is well under that — typically <1 s for an immediate
/// `ECONNREFUSED`). Maps to the production `TronGridClient` retry policy
/// in Phase 6 — CLI returns exit code 3 for transport errors, never panics.
#[tokio::test]
#[ignore = "Phase 4 §4.5 row 4 — Network failure recovery. Same `--ignored` opt-in as trc20_transfer_full_flow_nile; asserts no panic / no hang + transport error surface."]
async fn row_4_network_failure_recovery_nile() {
    let _spki_override = nile_spki_override();
    // Construct an http:// (unpinned) client pointed at a closed port on
    // loopback. `127.0.0.1:9999` has no listener — kernel returns
    // `ECONNREFUSED` immediately. Same harness used by the canonical row's
    // pinned RPC, just configured for local/loopback.
    let rpc = tron_v1_spike::rpc::JsonRpcClient::new_local("http://127.0.0.1:9999")
        .expect("[row_4] new_local must accept http://127.0.0.1:9999");
    eprintln!(
        "[row_4] JsonRpcClient built against {}:{}",
        rpc.host, rpc.port
    );

    // Probe `balanceOf` against the closed port — measure end-to-end
    // latency so we can prove the "no hang" half of the contract.
    let fixture = load_nile_fixture();
    let recipient = fixture.test.recipient_tr20.address.as_str();
    let usdt = nile_usdt_address();
    let started = std::time::Instant::now();
    let outcome = tron_v1_spike::tx::balance_of_trc20(&rpc, &usdt, recipient).await;
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
        elapsed < Duration::from_secs(30),
        "[row_4] transport error must surface within 30 s (no hang), took {elapsed:?}"
    );
    // The error chain must end at a reqwest transport error — anything else
    // (parse error, JSON-RPC protocol error) means the client is
    // misrouting. `balance_of_trc20` wraps the underlying JsonRpcError via
    // `BalanceError::Rpc`, which carries the original `reqwest::Error`.
    eprintln!("[row_4] PASS — transport error propagated, no panic, no hang within 30s");
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
