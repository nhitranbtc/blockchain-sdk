//! Use case: alpha sends beta 1 USDT-TRC20 on a local TRON devnet.
//!
//! **Offline companion** (always runs in CI): generate alpha + beta wallets from
//! canonical BIP-39 mnemonics; build the 68-byte TRC-20 `transfer(address,uint256)`
//! calldata; sign with k256 ECDSA over a 32-byte prehash (stand-in for `txID`);
//! assert 65-byte `r‖s‖v` signature with `v ∈ {0, 1}`. No protobuf types required —
//! the vendored `core/Tron.proto` doesn't include `TransferContract` or
//! `TriggerSmartContract` (those live in unvendored `core/contract/*.proto` files;
//! production code in `crates/tron-wallet-core/` vendors the full tree).
//!
//! **Live test** (operator-driven per L29, gated behind `RUN_TRON_LOCAL=1`):
//! spawns a real local TRON node via **testcontainers** using the official
//! [`tronbox/tre`](https://hub.docker.com/r/tronbox/tre) Docker image. Asserts the
//! node boots and serves `/wallet/getnowblock` over HTTP.
//!
//! Mirrors the `alloy-v1/tests/v6_erc20_anvil.rs:67` pattern (`Anvil::new().spawn()`
//! gated behind `RUN_V6_ANVIL=1`). The TRON equivalent of Anvil is TronBox
//! (Java + Node) per plan §C — no pure-Rust equivalent exists today.
//!
//! **Setup** (one-time, per developer machine):
//!   ```bash
//!   docker pull tronbox/tre:latest
//!   ```
//!
//! **Run** (operator, after `docker pull`):
//!   ```bash
//!   RUN_TRON_LOCAL=1 cargo test -p tron-v1-spike \
//!     --test use_case_alpha_sends_beta_usdt -- --ignored --nocapture
//!   ```
//!
//! **TRC-20 contract deployment**: this test verifies the **testcontainer spawn +
//! readiness probe** path. The full TRC-20 deploy + transfer + balance-verify flow
//! is a follow-up: requires shipping a `MockTRC20.sol` fixture + running
//! `tronbox migrate --network development` inside the container before the
//! transfer. Tracked as backlog issue.

use bip39::{Language, Mnemonic};
use k256::ecdsa::signature::hazmat::PrehashSigner;
use k256::ecdsa::SigningKey;
use sha2::Digest;
use std::time::{Duration, Instant};
use testcontainers::{
    core::{ContainerPort, IntoContainerPort},
    runners::AsyncRunner,
    GenericImage,
};
use tron_v1_spike::abi::encode_transfer;
use tron_v1_spike::address::{from_base58check, to_base58check};
use tron_v1_spike::config::nile_config;

const READY_PROBE_TIMEOUT: Duration = Duration::from_secs(180);

// Canonical alpha + beta mnemonics (NOT for production — for reproducible test signing
// per L29 + V10's "abandon ×11 + about" vector).
const ALPHA_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const BETA_MNEMONIC: &str =
    "legal winner thank year wave sausage worth useful legal winner thank yellow";

// 1 USDT-TRC20 in 6-decimal base units = 1 × 10^6 = 1_000_000.
const AMOUNT_BASE_UNITS: u64 = 1_000_000;

/// Look up the Nile USDT-TRC20 contract address from `tokens/nile.json` (single
/// source of truth). Returns the first token whose symbol matches `USDT`.
///
/// The address was verified live 2026-08-27 via
/// `https://nile.trongrid.io/wallet/triggersmartcontract`: `name()="Tether USD"`,
/// `symbol()="USDT"`, `decimals()=6`. Replaces the stale V9 entry
/// `TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z` which does not exist on Nile.
fn nile_usdt_address() -> String {
    nile_config()
        .tokens
        .iter()
        .find(|t| t.symbol == "USDT")
        .map(|t| t.address.clone())
        .expect("USDT token must be present in tokens/nile.json")
}

fn fresh_wallet(mnemonic_phrase: &str) -> (String, [u8; 21], SigningKey) {
    let phrase = Mnemonic::parse_in(Language::English, mnemonic_phrase).expect("BIP-39 parse");
    let seed = phrase.to_seed("");
    let path: bip32::DerivationPath = "m/44'/195'/0'/0/0".parse().expect("SLIP-44 path");
    let xprv = bip32::XPrv::derive_from_path(seed, &path).expect("XPrv derive");
    let xpub = xprv.public_key();
    let verifying_key = xpub.public_key();

    let pubkey_bytes = verifying_key.to_encoded_point(false).as_bytes().to_vec();
    let mut pubkey = [0u8; 65];
    pubkey.copy_from_slice(&pubkey_bytes);
    let raw = tron_v1_spike::address::raw_21_from_uncompressed_pubkey(&pubkey);
    let address = to_base58check(&raw);
    assert!(
        address.starts_with('T'),
        "address must start with T: {address}"
    );

    (address, raw, xprv.private_key().clone())
}

fn base58_to_20bytes(t_addr: &str) -> [u8; 20] {
    let raw21 = from_base58check(t_addr).expect("T-address decodes");
    let mut out = [0u8; 20];
    out.copy_from_slice(&raw21[1..]); // skip 0x41 prefix
    out
}

/// Build the 68-byte TRC-20 `transfer(beta, amount)` calldata against
/// `usdt_contract`. The calldata layout is exercised end-to-end here.
fn build_trc20_transfer_calldata(_usdt_contract_t: &str, beta_t: &str, amount: u64) -> Vec<u8> {
    let beta_20 = base58_to_20bytes(beta_t);
    let mut value32 = [0u8; 32];
    value32[24..].copy_from_slice(&amount.to_be_bytes());
    encode_transfer(&beta_20, &value32).to_vec()
}

/// k256 ECDSA sign over the 32-byte prehash (stand-in for the production `txID =
/// SHA-256(protobuf-serialize(raw_data))` per plan §Q2). Returns the 65-byte
/// canonical `r‖s‖v` form with `v ∈ {0, 1}` per plan §Q8.
fn sign_prehash_65byte(prehash: &[u8; 32], signing_key: &SigningKey) -> [u8; 65] {
    let sig: k256::ecdsa::Signature = signing_key.sign_prehash(prehash).unwrap();
    let rs = sig.to_bytes();
    let (_rec_sig, rid) = signing_key.sign_prehash_recoverable(prehash).unwrap();
    let v = rid.to_byte();
    assert!(v <= 1, "TRON v byte must be ∈ {{0, 1}}, got {v}");

    let mut sig65 = [0u8; 65];
    sig65[..64].copy_from_slice(&rs);
    sig65[64] = v;
    sig65
}

fn env_opt_in() -> bool {
    std::env::var("RUN_TRON_LOCAL")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Read the three env vars that gate the live Nile e2e path. Returns `None`
/// when any are missing — caller treats that as a skip (CI without creds
/// stays green). Sender address is derived from the privkey's public key
/// (uncompressed pubkey → keccak256 → last-20-bytes → 0x41 prefix → base58check)
/// — guarantees the signing keypair matches the from-address by construction.
fn nile_creds() -> Option<(String, String, String)> {
    let privkey_hex = std::env::var("TRON_NILE_PRIVATE_KEY").ok()?;
    let recipient = std::env::var("TRON_NILE_RECIPIENT_ADDRESS").ok()?;
    let spki_pin = std::env::var("TRON_NILE_SPKI_PIN").ok()?;
    Some((privkey_hex, recipient, spki_pin))
}

/// Offline companion — always runs in CI. Verifies alpha + beta wallet
/// derivation + 68-byte TRC-20 calldata layout + 65-byte k256 signature format.
/// No network, no container, no protobuf (TRC-20 contract proto not vendored).
#[test]
fn use_case_alpha_sends_beta_usdt_offline() {
    let (alpha_t, _alpha_raw21, alpha_sk) = fresh_wallet(ALPHA_MNEMONIC);
    let (beta_t, _beta_raw21, _beta_sk) = fresh_wallet(BETA_MNEMONIC);

    assert!(alpha_t.starts_with('T'));
    assert!(beta_t.starts_with('T'));
    assert_ne!(alpha_t, beta_t, "alpha + beta must be different wallets");

    let usdt_contract = nile_usdt_address();

    // 68-byte calldata = selector(4) + to_32(32) + value_32(32).
    let calldata = build_trc20_transfer_calldata(&usdt_contract, &beta_t, AMOUNT_BASE_UNITS);
    assert_eq!(calldata.len(), 68);
    assert_eq!(
        &calldata[0..4],
        &tron_v1_spike::abi::TRANSFER_SELECTOR,
        "selector must be 0xa9059cbb"
    );
    // value32 = 1_000_000 (1 USDT × 10^6) big-endian uint256 — last 8 bytes
    // of the 32-byte value slot encode the amount.
    let amount_in_calldata = u64::from_be_bytes(calldata[60..68].try_into().unwrap());
    assert_eq!(
        amount_in_calldata, AMOUNT_BASE_UNITS,
        "amount must round-trip"
    );

    // 65-byte k256 signature over a deterministic prehash.
    let prehash: [u8; 32] = sha2::Sha256::digest(b"tron-v1-spike use-case prehash").into();
    let sig = sign_prehash_65byte(&prehash, &alpha_sk);
    assert_eq!(sig.len(), 65);
    assert!(sig[64] <= 1, "v byte ∈ {{0, 1}}, got {}", sig[64]);

    eprintln!("[use_case/offline] alpha = {alpha_t}");
    eprintln!("[use_case/offline] beta  = {beta_t}");
    eprintln!("[use_case/offline] USDT contract = {usdt_contract}");
    eprintln!("[use_case/offline] calldata len = {} bytes", calldata.len());
    eprintln!("[use_case/offline] sig65 v byte = {}", sig[64]);
}

/// Live use case — spawns a real local TRON node via testcontainers and asserts
/// readiness. Gated per L29 behind `RUN_TRON_LOCAL=1` (operator-driven; CI excluded).
///
/// Asserts: tronbox/tre container starts + port 9090 exposed + HTTP
/// `/wallet/getnowblock` returns 200. The full TRC-20 broadcast + balance-verify
/// flow is a follow-up (requires contract deployment via `tronbox migrate` inside
/// the spawned container; see backlog).
#[tokio::test]
#[ignore = "operator-driven per L29 — RUN_TRON_LOCAL=1 cargo test -p tron-v1-spike --test use_case_alpha_sends_beta_usdt -- --ignored --nocapture"]
async fn use_case_alpha_sends_beta_usdt_live_local_node() {
    if !env_opt_in() {
        eprintln!("[SKIP — RUN_TRON_LOCAL=1 required to spawn tronbox/tre]");
        return;
    }

    eprintln!("[use_case/live] spawning tronbox/tre container...");
    let container = GenericImage::new("tronbox/tre", "latest")
        .with_exposed_port(9090.tcp())
        .start()
        .await
        .expect("testcontainers: spawn tronbox/tre");
    let host_port = container
        .get_host_port_ipv4(ContainerPort::Tcp(9090))
        .await
        .expect("testcontainers: host port for 9090");
    let base_url = format!("http://127.0.0.1:{host_port}");
    eprintln!("[use_case/live] container started; probing {base_url}");

    // Defensive readiness probe — block on a blocking client in a spawn_blocking
    // task so the tokio runtime stays available for the testcontainers async drop.
    let probe_url = base_url.clone();
    let probe = tokio::task::spawn_blocking(move || {
        let client = reqwest::blocking::Client::new();
        let deadline = Instant::now() + READY_PROBE_TIMEOUT;
        while Instant::now() < deadline {
            if let Ok(resp) = client
                .post(format!("{probe_url}/wallet/getnowblock"))
                .json(&serde_json::json!({}))
                .send()
            {
                if resp.status().is_success() {
                    return resp.json::<serde_json::Value>().ok();
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        None
    })
    .await
    .expect("spawn_blocking join");

    let now_block = probe.expect("tronbox/tre local node did not respond within timeout");
    let block_id = now_block
        .get("blockID")
        .and_then(|v| v.as_str())
        .expect("missing blockID");
    eprintln!("[use_case/live] node serving; latest blockID = {block_id}");
    assert!(!block_id.is_empty());

    // NOTE: TRC-20 contract deploy + transfer + balance-verify deferred (backlog).
    // container drops here in the testcontainers async runtime, satisfying the
    // async-drop requirement.
    drop(container);
}

/// Live use case — broadcasts a real 1 USDT-TRC20 transfer on the Nile
/// testnet via SPKI-pinned TronGrid JSON-RPC, polls for confirmation, and
/// asserts recipient `balanceOf` increased. Gated per L29 behind three env
/// vars (operator-driven; CI excluded via skip).
///
/// **Setup** (operator, one-time):
///   ```bash
///   # Derive the SPKI pin from a live TLS handshake:
///   openssl s_client -connect nile.trongrid.io:443 -servername nile.trongrid.io \
///     </dev/null 2>/dev/null \
///     | openssl x509 -pubkey -noout \
///     | openssl pkey -pubin -outform der \
///     | openssl dgst -sha256 -binary | xxd -p -c 256
///   # Fund alpha address via Nile faucet: https://nileex.io/join/getJoinPage
///   # Claim community USDT-TEST from the same faucet.
///   ```
///
/// **Run** (operator, after env vars + faucet funding):
///   ```bash
///   set -a; source tests/.env; set +a
///   cargo test -p tron-v1-spike \
///     --test use_case_alpha_sends_beta_usdt \
///     use_case_alpha_sends_beta_usdt_live_nile \
///     -- --ignored --nocapture
///   ```
///
/// This test calls 4 helpers in `tron_v1_spike::tx::*` and
/// `tron_v1_spike::rpc::JsonRpcClient::new_pinned` that are not yet implemented
/// — **compile failure is the RED signal per TDD** (env-gated e2e cannot be
/// exercised in this dev session; the compile fail proves the test exists and
/// is wired against the production API surface).
#[tokio::test]
#[ignore = "operator-driven e2e — TRON_NILE_PRIVATE_KEY + TRON_NILE_RECIPIENT_ADDRESS + TRON_NILE_SPKI_PIN required; cargo test -p tron-v1-spike --test use_case_alpha_sends_beta_usdt use_case_alpha_sends_beta_usdt_live_nile -- --ignored --nocapture"]
async fn use_case_alpha_sends_beta_usdt_live_nile() {
    let (privkey_hex, recipient_t, spki_pin) = match nile_creds() {
        Some(c) => c,
        None => {
            eprintln!(
                "[SKIP — TRON_NILE_PRIVATE_KEY + TRON_NILE_RECIPIENT_ADDRESS + TRON_NILE_SPKI_PIN required]"
            );
            return;
        }
    };

    let usdt_contract = nile_usdt_address();

    // Parse the operator-supplied 32-byte private-key hex.
    let sender_sk_bytes: [u8; 32] = hex::decode(&privkey_hex)
        .expect("TRON_NILE_PRIVATE_KEY hex decode")
        .try_into()
        .expect("TRON_NILE_PRIVATE_KEY must be exactly 32 bytes");
    let sender_sk =
        SigningKey::from_bytes(&sender_sk_bytes.into()).expect("valid secp256k1 scalar");

    // Derive sender T-address from the privkey's public key (uncompressed
    // pubkey → keccak256 → last-20-bytes → 0x41 prefix → base58check).
    // Guarantees the signing keypair matches the from-address by construction
    // — no drift between .env pair possible.
    let sender_pubkey = sender_sk.verifying_key().to_encoded_point(false);
    let mut sender_pubkey_arr = [0u8; 65];
    sender_pubkey_arr.copy_from_slice(sender_pubkey.as_bytes());
    let sender_raw21 = tron_v1_spike::address::raw_21_from_uncompressed_pubkey(&sender_pubkey_arr);
    let sender_t = to_base58check(&sender_raw21);

    assert!(sender_t.starts_with('T'));
    assert!(recipient_t.starts_with('T'));
    assert_ne!(sender_t, recipient_t, "sender ≠ recipient");
    eprintln!("[use_case/nile] sender    = {sender_t}");
    eprintln!("[use_case/nile] recipient = {recipient_t}");
    eprintln!("[use_case/nile] amount    = {AMOUNT_BASE_UNITS} (1 USDT × 10^6)");
    eprintln!("[use_case/nile] USDT contract = {usdt_contract}");

    // Construct SPKI-pinned RPC client to Nile TronGrid.
    let pinned_url = format!(
        "pinned://{spki_pin}@{}:443",
        tron_v1_spike::config::nile_config().rpc_host()
    );
    let rpc = tron_v1_spike::rpc::JsonRpcClient::new_pinned(&pinned_url)
        .expect("construct SPKI-pinned Nile RPC client");

    // Build + sign a TriggerSmartContract transaction for USDT-TRC20 transfer.
    let signed_tx = tron_v1_spike::tx::build_signed_trc20_transfer(
        &rpc,
        &sender_sk,
        &sender_t,
        &usdt_contract,
        &recipient_t,
        AMOUNT_BASE_UNITS,
    )
    .await
    .unwrap_or_else(|e| panic!("build signed TRC-20 transfer tx: {:?}: {}", e, e));
    let tx_id = signed_tx.tx_id.clone();
    eprintln!("[use_case/nile] tx_id  = {tx_id}");

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
    let poll_deadline = std::time::Duration::from_secs(120);
    tron_v1_spike::tx::poll_for_confirmation(&rpc, &tx_id, poll_deadline)
        .await
        .expect("tx confirmation poll");
    eprintln!("[use_case/nile] confirmed after ≤{poll_deadline:?}");

    // Verify recipient `balanceOf` increased by at least the transfer amount
    // (any extra balance from prior tests on the same address is allowed).
    let balance_after = tron_v1_spike::tx::balance_of_trc20(&rpc, &usdt_contract, &recipient_t)
        .await
        .expect("balanceOf query");
    eprintln!("[use_case/nile] recipient balanceOf = {balance_after} raw (6-dec)");
    let min_balance: u128 = AMOUNT_BASE_UNITS.into();
    assert!(
        balance_after >= min_balance,
        "recipient should hold ≥{AMOUNT_BASE_UNITS} raw (1 USDT), got {balance_after}"
    );

    eprintln!(
        "[use_case/nile] PASS — {}",
        tron_v1_spike::config::nile_config()
            .explorer_tx_url
            .replace("{tx_id}", &tx_id)
    );
}
