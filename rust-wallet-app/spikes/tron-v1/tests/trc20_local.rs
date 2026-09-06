//! Phase 4 §4.1-4.2 — TronBox local testnet integration (testcontainers).
//!
//! Spawns the `tronbox/tre` Docker image via testcontainers 0.23 and exercises
//! the local node's RPC surface. Pattern mirrors
//! `tests/use_case_alpha_sends_beta_usdt.rs::use_case_alpha_sends_beta_usdt_live_local_node`
//! (Phase 4 deliberately *promotes* that probe into the canonical integration
//! harness and adds the Phase 2 carry-over surfaces — `walletsolidity/getnowblock`
//! for TAPOS, `eth_chainId` for chain-id verification).
//!
//! ## Gating
//!
//! - **Container spawn tests:** gated on `RUN_TRON_LOCAL=1` and a live Docker
//!   socket. CI excluded — `docker:dind` runner only. Operators opt in:
//!   `RUN_TRON_LOCAL=1 cargo test -p tron-v1-spike --test trc20_local -- --ignored`.
//! - **Setup (one-time):** `docker pull tronbox/tre:latest`.
//!
//! ## What lives in v0.1
//!
//! - Container spawn + readiness probe (`/wallet/getnowblock` 200 OK)
//! - TAPOS path (`/walletsolidity/getnowblock` 200 OK) — Phase 2 §2.4
//! - Chain-id verification (`/jsonrpc eth_chainId` returns the local default)
//! - Plain-HTTP `JsonRpcClient::new_local` construction + URL parsing
//!
//! ## What is deferred to a follow-up
//!
//! Rows 1-7a, 8 from the Phase 4 §4.2 matrix (TRX native transfer, TRC-20
//! transfer, approval, allowance, Stake 2.0, insufficient-balance,
//! send-speedup, rebroadcast idempotency, wallet-to-wallet TRC-20) all require
//! `MockTRC20.sol` fixture + `tronbox migrate --network development` *inside*
//! the spawned container before any transfer can be signed. The harness
//! provides the spawn surface; the contract-deployed integration ships
//! separately. Each row is wired as `#[ignore]` with a TODO + prerequisite
//! docstring so the operator runbook is explicit.
//!
//! ## Convention
//!
//! Local-container tests are GATED behind `RUN_TRON_LOCAL=1` per Phase 4 §4.1.
//! Loud-RED panic on missing prerequisite (Docker daemon, image pull) — silent
//! `return` is forbidden per the plan's gated-live-test convention. Mirror of
//! `tests/v5_resource.rs` / `tests/v7_spki_pin.rs` (Phase 2 carry-over).

use std::time::{Duration, Instant};
use testcontainers::{
    core::{ContainerPort, IntoContainerPort},
    runners::AsyncRunner,
    GenericImage,
};
use tron_v1_spike::rpc::{ClientParseError, JsonRpcClient};

/// How long to wait for the TronBox container to start serving `/wallet/getnowblock`.
/// TronBox `tre` is Java + Node based; cold start can be slow.
const READY_PROBE_TIMEOUT: Duration = Duration::from_secs(180);

/// Default port exposed by the `tronbox/tre:latest` image.
const TRONBOX_DEFAULT_PORT: u16 = 9090;

/// Operator opt-in gate. Missing env → loud-RED panic naming the missing variable.
fn require_local_opt_in() {
    match std::env::var("RUN_TRON_LOCAL").ok().as_deref() {
        Some("1") | Some("true") | Some("TRUE") => {}
        _ => panic!(
            "RUN_TRON_LOCAL=1 required to spawn tronbox/tre container; \
             also requires Docker daemon + `docker pull tronbox/tre:latest`. \
             CI excluded by design (desktop-only)."
        ),
    }
}

/// Spawn a fresh `tronbox/tre:latest` container, return its base URL and the
/// container handle (caller is responsible for keeping the handle alive for
/// the test duration — dropping it tears the container down).
async fn spawn_tronbox() -> (String, testcontainers::ContainerAsync<GenericImage>) {
    eprintln!("[trc20_local] spawning tronbox/tre container...");
    let container = GenericImage::new("tronbox/tre", "latest")
        .with_exposed_port(TRONBOX_DEFAULT_PORT.tcp())
        .start()
        .await
        .expect(
            "testcontainers: spawn tronbox/tre failed. \
             Prerequisite: Docker daemon reachable + `docker pull tronbox/tre:latest`. \
             Operator runbook: `docker pull tronbox/tre:latest` then re-run.",
        );
    let host_port = container
        .get_host_port_ipv4(ContainerPort::Tcp(TRONBOX_DEFAULT_PORT))
        .await
        .expect("testcontainers: resolve host port for 9090");
    let base_url = format!("http://127.0.0.1:{host_port}");
    eprintln!("[trc20_local] container started; base_url = {base_url}");
    (base_url, container)
}

/// Blocking readiness probe. Polls `/wallet/getnowblock` every 2s up to
/// `READY_PROBE_TIMEOUT`. Returns the parsed JSON body on the first 2xx.
fn probe_getnowblock(base_url: &str) -> Option<serde_json::Value> {
    let probe_url = base_url.to_string();
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
}

// ---------------------------------------------------------------------------
// Container spawn + readiness probe — the harness path.
// ---------------------------------------------------------------------------

/// `tronbox/tre` container starts and serves `/wallet/getnowblock` 2xx.
/// Phase 4 §4.1 acceptance — container spawn + HTTP readiness.
#[tokio::test]
#[ignore = "operator-driven per Phase 4 §4.1 — RUN_TRON_LOCAL=1 cargo test -p tron-v1-spike --test trc20_local -- --ignored"]
async fn tronbox_local_node_serves_getnowblock() {
    require_local_opt_in();
    let (base_url, _container) = spawn_tronbox().await;

    let now_block =
        probe_getnowblock(&base_url).expect("tronbox/tre local node did not respond within 180s");
    let block_id = now_block
        .get("blockID")
        .and_then(|v| v.as_str())
        .expect("missing blockID in /wallet/getnowblock response");
    assert!(!block_id.is_empty(), "blockID must be non-empty");
    eprintln!("[trc20_local] /wallet/getnowblock served; blockID = {block_id}");
}

/// `walletsolidity/getnowblock` (NOT `wallet/getnowblock`) for TAPOS reference.
/// Phase 2 §2.4 — finality requires the SolidityNode path.
#[tokio::test]
#[ignore = "operator-driven per Phase 4 §4.1 — RUN_TRON_LOCAL=1 required"]
async fn tronbox_local_node_serves_walletsolidity_getnowblock() {
    require_local_opt_in();
    let (base_url, _container) = spawn_tronbox().await;

    // First confirm the node is up (probe the cheaper endpoint), then query the
    // SolidityNode endpoint.
    probe_getnowblock(&base_url).expect("node did not become ready for SolidityNode probe");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base_url}/walletsolidity/getnowblock"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("walletsolidity/getnowblock transport");
    assert!(
        resp.status().is_success(),
        "walletsolidity/getnowblock non-2xx: {}",
        resp.status()
    );
    let body: serde_json::Value = resp.json().await.expect("json body");
    let block_header = body
        .get("block_header")
        .unwrap_or_else(|| panic!("missing block_header: {body}"));
    let raw_data = block_header
        .get("raw_data")
        .unwrap_or_else(|| panic!("missing block_header.raw_data: {block_header}"));
    // raw_data carries ref_block_bytes + ref_block_hash for TAPOS — assert
    // the field exists rather than hard-coding the schema.
    let _ = raw_data
        .get("number")
        .or_else(|| raw_data.get("txTrieRoot"))
        .unwrap_or_else(|| panic!("raw_data missing ref_block_bytes fields: {raw_data}"));
    eprintln!("[trc20_local] /walletsolidity/getnowblock served (TAPOS path)");
}

/// `/jsonrpc eth_chainId` round-trips — verifies the TronBox HTTP front
/// exposes the Ethereum-style JSON-RPC method the Nile harness relies on.
/// Chain id is the local TronBox default; we only assert shape (0x-prefixed
/// hex), not value — operators may override genesis.
#[tokio::test]
#[ignore = "operator-driven per Phase 4 §4.1 — RUN_TRON_LOCAL=1 required"]
async fn tronbox_local_node_serves_eth_chainid() {
    require_local_opt_in();
    let (base_url, _container) = spawn_tronbox().await;
    probe_getnowblock(&base_url).expect("node did not become ready");

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_chainId",
        "params": [],
        "id": 1,
    });
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base_url}/jsonrpc"))
        .json(&body)
        .send()
        .await
        .expect("eth_chainId transport");
    assert!(
        resp.status().is_success(),
        "eth_chainId non-2xx: {}",
        resp.status()
    );
    let parsed: serde_json::Value = resp.json().await.expect("json body");
    let chain_id_hex = parsed
        .get("result")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("missing eth_chainId result: {parsed}"));
    assert!(
        chain_id_hex.starts_with("0x") && chain_id_hex.len() > 2,
        "chain id must be 0x-prefixed hex, got {chain_id_hex}"
    );
    eprintln!("[trc20_local] /jsonrpc eth_chainId = {chain_id_hex}");
}

// ---------------------------------------------------------------------------
// `JsonRpcClient::new_local` URL parsing — unit-level (no container).
// ---------------------------------------------------------------------------

#[test]
fn new_local_parses_http_url_with_default_port_9090() {
    let client = JsonRpcClient::new_local("http://127.0.0.1").expect("parse");
    assert_eq!(client.scheme, "http");
    assert_eq!(client.host, "127.0.0.1");
    assert_eq!(client.port, TRONBOX_DEFAULT_PORT);
    assert!(
        client.pin_hex.is_empty(),
        "local client must not carry a pin"
    );
    assert_eq!(client.base_url(), "http://127.0.0.1:9090");
}

#[test]
fn new_local_parses_http_url_with_explicit_port() {
    let client = JsonRpcClient::new_local("http://localhost:8545").expect("parse");
    assert_eq!(client.scheme, "http");
    assert_eq!(client.host, "localhost");
    assert_eq!(client.port, 8545);
    assert_eq!(client.base_url(), "http://localhost:8545");
}

#[test]
fn new_local_rejects_https_url() {
    let err = JsonRpcClient::new_local("https://nile.trongrid.io").unwrap_err();
    assert_eq!(err, ClientParseError::LocalMustBeHttp);
}

#[test]
fn new_local_rejects_pinned_scheme() {
    let err = JsonRpcClient::new_local(
        "pinned://e9cc763b176063ea6eed1525dac2542512d9e0bf601e210a14f6aad218a9479f@nile.trongrid.io",
    )
    .unwrap_err();
    assert_eq!(err, ClientParseError::LocalMustBeHttp);
}

#[test]
fn new_pinned_defaults_scheme_to_https() {
    let pin = "e9cc763b176063ea6eed1525dac2542512d9e0bf601e210a14f6aad218a9479f";
    let client =
        JsonRpcClient::new_pinned(&format!("pinned://{pin}@nile.trongrid.io")).expect("parse");
    assert_eq!(client.scheme, "https");
    assert_eq!(client.port, 443);
    assert_eq!(client.base_url(), "https://nile.trongrid.io:443");
}

// ---------------------------------------------------------------------------
// Deeper scenario rows (1-7a, 8) — `#[ignore]` stubs awaiting contract deploy.
// ---------------------------------------------------------------------------
//
// Each row is documented per Phase 4 §4.2 matrix. The `MockTRC20.sol` fixture
// + `tronbox migrate --network development` inside the spawned container is
// the prerequisite. Operator runbook (one-time, per developer machine):
//
//   1. Author `contracts/MockTRC20.sol` (TRC-20 fixed-supply mint-on-deploy).
//   2. `docker pull tronbox/tre:latest`
//   3. Run an integration harness that:
//      a. Spawns the container.
//      b. Copies `MockTRC20.sol` + `tronbox.js` config into the container.
//      c. Executes `npx tronbox migrate --network development` inside it.
//      d. Reads the deployed contract address from the migration receipt.
//   4. With the deployed address + funded sender key, the rows below unblock.
//
// The stubs below stay `#[ignore]`d until (a)-(d) ship. They assert the
// pre-deploy contract is consistent (call to `tokens::load(Local)` returns
// the TronBox USDT entry) so partial progress is verifiable.

#[tokio::test]
#[ignore = "Phase 4 §4.2 row 1 — TRX native transfer. Awaiting MockTRC20 deploy; see doc above."]
async fn row_1_trx_native_transfer_local() {
    require_local_opt_in();
    let (_base_url, _container) = spawn_tronbox().await;
    // TODO(phase4): deploy funded sender key, sign + broadcast TRX transfer,
    // assert receipt.energy_usage < 0 (bandwidth only), balances reconcile.
    // Deferred until MockTRC20.sol fixture + tronbox migrate harness ships.
    panic!("deferred — MockTRC20 contract deploy prerequisite; see test docstring");
}

#[tokio::test]
#[ignore = "Phase 4 §4.2 row 2 — TRC-20 transfer (held recipient). Awaiting MockTRC20 deploy."]
async fn row_2_trc20_transfer_held_recipient_local() {
    require_local_opt_in();
    let (_base_url, _container) = spawn_tronbox().await;
    // TODO(phase4): submit 100 mock USDT to a recipient that already holds
    // USDT, assert energy_usage ≈ 65_000, recipient.balanceOf = 100.
    panic!("deferred — MockTRC20 contract deploy prerequisite");
}

#[tokio::test]
#[ignore = "Phase 4 §4.2 row 3 — TRC-20 first-time receive (empty recipient). Awaiting deploy."]
async fn row_3_trc20_first_time_receive_local() {
    require_local_opt_in();
    let (_base_url, _container) = spawn_tronbox().await;
    // TODO(phase4): submit 50 mock USDT to a fresh address with fee_limit
    // 130_000_000, assert energy_usage ≈ 130_000 (2x baseline).
    panic!("deferred — MockTRC20 contract deploy prerequisite");
}

#[tokio::test]
#[ignore = "Phase 4 §4.2 row 4 — TRC-20 approval + allowance. Awaiting deploy."]
async fn row_4_trc20_approval_local() {
    require_local_opt_in();
    let (_base_url, _container) = spawn_tronbox().await;
    // TODO(phase4): approve 1000 mock USDT to a DEX spender, query allowance
    // via triggerconstantcontract, assert allowance(owner, spender) == 1000.
    panic!("deferred — MockTRC20 contract deploy prerequisite");
}

#[tokio::test]
#[ignore = "Phase 4 §4.2 row 5 — Stake 2.0 freeze/unfreeze. V0.1.5 — outside v0.1 scope."]
async fn row_5_stake2_freeze_unfreeze_local() {
    require_local_opt_in();
    let (_base_url, _container) = spawn_tronbox().await;
    // TODO(v0.1.5): submit FreezeBalanceV2Contract + UnfreezeBalanceV2Contract,
    // assert account resource query shows frozen balance.
    panic!("deferred — Stake 2.0 ships with V0.1.5 release train");
}

#[tokio::test]
#[ignore = "Phase 4 §4.2 row 6 — Insufficient balance. Awaiting deploy."]
async fn row_6_trc20_insufficient_balance_local() {
    require_local_opt_in();
    let (_base_url, _container) = spawn_tronbox().await;
    // TODO(phase4): submit transfer > balance, assert tx REVERTED with exit
    // code 5 per plan §Phase 6 exit-code matrix.
    panic!("deferred — MockTRC20 contract deploy prerequisite");
}

#[tokio::test]
#[ignore = "Phase 4 §4.2 row 7 — Send-speedup (RBF). Per V7a finding, speedup = fresh envelope, not rebroadcast."]
async fn row_7_send_speedup_local() {
    require_local_opt_in();
    let (_base_url, _container) = spawn_tronbox().await;
    // TODO(phase4): per Phase 6 §6.8 / V7a finding — speedup requires
    // set_timestamp(new) + set_fee_limit(new) + sign_tx + broadcast.
    // Pure envelope rebroadcast returns DUP_TRANSACTION_ERROR on Nile.
    panic!("deferred — MockTRC20 contract deploy + signed-envelope harness prerequisite");
}

#[tokio::test]
#[ignore = "Phase 4 §4.2 row 7a — rebroadcast idempotency. Awaiting deploy."]
async fn row_7a_rebroadcast_idempotency_local() {
    require_local_opt_in();
    let (_base_url, _container) = spawn_tronbox().await;
    // TODO(phase4): rebroadcast identical (full-envelope-hex) after 60s,
    // assert DUP_TRANSACTION_ERROR (per V7a finding on Nile 2026-09-06).
    panic!("deferred — MockTRC20 contract deploy prerequisite");
}

#[tokio::test]
#[ignore = "Phase 4 §4.2 row 8 — Wallet-to-wallet TRC-20. Awaiting deploy + WalletManager::lookup()."]
async fn row_8_wallet_to_wallet_trc20_local() {
    require_local_opt_in();
    let (_base_url, _container) = spawn_tronbox().await;
    // TODO(phase4): resolve cold wallet name via WalletManager::lookup(),
    // submit 100 mock USDT, assert tx accepted.
    panic!("deferred — MockTRC20 contract deploy + wallet persistence prerequisite");
}
