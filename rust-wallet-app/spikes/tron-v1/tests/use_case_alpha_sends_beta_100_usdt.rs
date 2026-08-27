//! Use case: alpha sends beta 100 USDT-TRC20 on a local TRON devnet.
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
//!     --test use_case_alpha_sends_beta_100_usdt -- --ignored --nocapture
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

const READY_PROBE_TIMEOUT: Duration = Duration::from_secs(180);

// Canonical alpha + beta mnemonics (NOT for production — for reproducible test signing
// per L29 + V10's "abandon ×11 + about" vector).
const ALPHA_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const BETA_MNEMONIC: &str =
    "legal winner thank year wave sausage worth useful legal winner thank yellow";

// 100 USDT-TRC20 in 6-decimal base units = 100 × 10^6 = 100_000_000.
const AMOUNT_BASE_UNITS: u64 = 100_000_000;

// Community USDT on local + Nile testnet (per plan §Q9).
const USDT_CONTRACT_NILE: &str = "TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z";

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

/// Offline companion — always runs in CI. Verifies alpha + beta wallet
/// derivation + 68-byte TRC-20 calldata layout + 65-byte k256 signature format.
/// No network, no container, no protobuf (TRC-20 contract proto not vendored).
#[test]
fn use_case_alpha_sends_beta_100_usdt_offline() {
    let (alpha_t, _alpha_raw21, alpha_sk) = fresh_wallet(ALPHA_MNEMONIC);
    let (beta_t, _beta_raw21, _beta_sk) = fresh_wallet(BETA_MNEMONIC);

    assert!(alpha_t.starts_with('T'));
    assert!(beta_t.starts_with('T'));
    assert_ne!(alpha_t, beta_t, "alpha + beta must be different wallets");

    // 68-byte calldata = selector(4) + to_32(32) + value_32(32).
    let calldata = build_trc20_transfer_calldata(USDT_CONTRACT_NILE, &beta_t, AMOUNT_BASE_UNITS);
    assert_eq!(calldata.len(), 68);
    assert_eq!(
        &calldata[0..4],
        &tron_v1_spike::abi::TRANSFER_SELECTOR,
        "selector must be 0xa9059cbb"
    );
    // value32 = 100_000_000 (100 USDT × 10^6) big-endian uint256 — last 8 bytes
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
    eprintln!("[use_case/offline] USDT contract = {USDT_CONTRACT_NILE}");
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
#[ignore = "operator-driven per L29 — RUN_TRON_LOCAL=1 cargo test -p tron-v1-spike --test use_case_alpha_sends_beta_100_usdt -- --ignored --nocapture"]
async fn use_case_alpha_sends_beta_100_usdt_live_local_node() {
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
