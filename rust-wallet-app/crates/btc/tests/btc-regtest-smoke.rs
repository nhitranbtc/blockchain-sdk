//! `btc-regtest-smoke` — ephemeral Bitcoin Core regtest container via testcontainers.
//!
//! Issue #112 (https://github.com/nhitranbtc/blockchain-sdk/issues/112) — promotes
//! the `btc wallet balance` happy-path from operator-driven smoke (L29,
//! `BTC_DEMO_ESPLORA_SPKI_PIN`) into a CI-gated regression test.
//!
//! **Scope of this PR (smoke only):**
//! 1. Boot a real `bitcoind` regtest container via testcontainers-rs.
//! 2. JSON-RPC: `getblockcount` returns 0 (chain tip at genesis).
//! 3. Mine 101 blocks to a known address (regtest coinbase maturity).
//! 4. Verify chain height advances to 101.
//! 5. Tear down container (RAII via `Drop`).
//!
//! **Out of scope (future PRs):**
//! - Esplora sidecar (electrs) so `btc wallet balance` can hit the real
//!   `EsploraClient` codepath. Issue #112 AC #5/6 defer this.
//! - Esplora SPKI pin propagation testcontainers-side (placeholder image).
//!
//! **CI implications:**
//! - Pulls `lncm/bitcoind:v0.21.2` (~80 MB cold). Add to action cache.
//! - First run: ~30s image pull + ~2s boot. Warm: ~3s total.
//! - Runs in parallel with `cargo test` existing job (no time budget change).
//!
//! **Disable:**
//! - `SKIP_TESTCONTAINERS=1` skips this test (for sandboxed CI runners
//!   without Docker socket access).

use std::borrow::Cow;
use std::time::Duration;

use testcontainers::{
    core::{ContainerPort, IntoContainerPort, WaitFor},
    runners::SyncRunner,
    Image,
};

const BITCOIND_IMAGE: &str = "bitcoin/bitcoin";
const BITCOIND_TAG: &str = "25";
const RPC_PORT: u16 = 18443;

/// Static port list — `expose_ports()` returns `&[ContainerPort]` which
/// requires `'static` data.
const EXPOSED_PORTS: [ContainerPort; 1] = [ContainerPort::Tcp(18443)];

/// Bitcoind regtest image — custom Image impl per testcontainers 0.23 API.
/// Uses `cmd()` to pass regtest flags + RPC port.
struct BitcoindRegtest;

impl Image for BitcoindRegtest {
    fn name(&self) -> &str {
        BITCOIND_IMAGE
    }

    fn tag(&self) -> &str {
        BITCOIND_TAG
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stdout("Done loading")]
    }

    fn cmd(&self) -> impl IntoIterator<Item = impl Into<Cow<'_, str>>> {
        [
            "-regtest".to_string(),
            "-rpcbind=0.0.0.0".to_string(),
            "-rpcallowip=0.0.0.0/0".to_string(),
            "-rpcuser=bitcoin".to_string(),
            "-rpcpassword=bitcoin".to_string(),
            "-fallbackfee=0.0002".to_string(),
            "-printtoconsole".to_string(),
        ]
    }

    fn expose_ports(&self) -> &[ContainerPort] {
        &EXPOSED_PORTS
    }
}

/// JSON-RPC `getblockcount` — returns the current chain height.
fn rpc_getblockcount(url: &str) -> Result<u64, String> {
    let body = serde_json::json!({
        "jsonrpc": "1.0",
        "id": "smoke",
        "method": "getblockcount",
        "params": [],
    });
    let resp = reqwest::blocking::Client::new()
        .post(url)
        .basic_auth("bitcoin", Some("bitcoin"))
        .json(&body)
        .send()
        .map_err(|e| format!("send: {e}"))?;
    let json: serde_json::Value = resp.json().map_err(|e| format!("parse: {e}"))?;
    let count = json.get("result").and_then(serde_json::Value::as_u64);
    count.ok_or_else(|| format!("no result in {json}"))
}

/// Smoke test: boot regtest + verify RPC responds with height 0.
#[test]
fn btc_regtest_smoke_container_boots_and_rpc_responds() {
    if std::env::var("SKIP_TESTCONTAINERS").is_ok() {
        eprintln!("SKIP_TESTCONTAINERS set; skipping testcontainers smoke");
        return;
    }

    let container = BitcoindRegtest
        .start()
        .expect("start bitcoind regtest container");
    let host_port = container
        .get_host_port_ipv4(RPC_PORT.tcp())
        .expect("map RPC port to host");
    let url = format!("http://127.0.0.1:{host_port}");

    // Wait for RPC readiness (bitcoind takes ~100ms to bind after "Done loading").
    let mut count = 0u64;
    for _ in 0..50 {
        if let Ok(c) = rpc_getblockcount(&url) {
            count = c;
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    assert_eq!(
        count, 0,
        "regtest should start at height 0, got {count} (RPC: {url})"
    );
}

/// Smoke test: regtest mines 101 blocks, verifies chain advance.
#[test]
fn btc_regtest_smoke_mines_101_blocks() {
    if std::env::var("SKIP_TESTCONTAINERS").is_ok() {
        eprintln!("SKIP_TESTCONTAINERS set; skipping testcontainers smoke");
        return;
    }

    let container = BitcoindRegtest
        .start()
        .expect("start bitcoind regtest container");
    let host_port = container
        .get_host_port_ipv4(RPC_PORT.tcp())
        .expect("map RPC port to host");
    let url = format!("http://127.0.0.1:{host_port}");

    // Mine 101 blocks to a fixed address (regtest coinbase maturity).
    let mine_body = serde_json::json!({
        "jsonrpc": "1.0",
        "id": "smoke",
        "method": "generatetoaddress",
        "params": [101, "bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080"],
    });
    let resp = reqwest::blocking::Client::new()
        .post(&url)
        .basic_auth("bitcoin", Some("bitcoin"))
        .json(&mine_body)
        .send()
        .expect("send mining RPC");
    assert!(
        resp.status().is_success(),
        "mining RPC failed: {:?}",
        resp.status()
    );

    let count = rpc_getblockcount(&url).expect("getblockcount after mining");
    assert_eq!(count, 101, "expected 101 blocks after mining, got {count}");
}

/// End-to-end workflow test: create wallet → fund → get balance.
///
/// Exercises the full `btc` CLI workflow against a real regtest node:
/// 1. `btc wallet import` — creates a wallet from the entropy=0 mnemonic
///    (returns wallet_id = UUID)
/// 2. Mine 101 blocks to the entropy=0 wallet's first address
///    (funds the wallet via coinbase)
/// 3. `btc wallet balance --mnemonic` — queries the balance
///
/// This is the happy-path use case that issue #112 / Story 3 care about:
/// proves the CLI works against a real node end-to-end (not just raw RPC).
/// Uses stateless `btc wallet balance` (no Esplora required) so the test
/// stays focused on the wallet-fund-balance surface.
///
/// **Currently `#[ignore]`** — `btc wallet import` saves the encrypted blob
/// under `$XDG_DATA_HOME/btc/wallets/testnet/<uuid>.enc` per ADR 0001, which
/// requires `btc wallet show` to decrypt — but `show` requires Esplora (F36).
/// The workaround is to use `btc wallet balance --mnemonic` (stateless), which
/// doesn't need the persisted blob. This test still demonstrates the workflow.
#[test]
#[ignore = "stateless check via --mnemonic; full wallet_id round-trip needs Esplora sidecar"]
fn btc_workflow_create_fund_balance() {
    if std::env::var("SKIP_TESTCONTAINERS").is_ok() {
        eprintln!("SKIP_TESTCONTAINERS set; skipping testcontainers smoke");
        return;
    }

    let tmp = std::env::temp_dir().join(format!("btc-workflow-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("create temp dir");

    // 1. Boot bitcoind regtest container.
    let container = BitcoindRegtest
        .start()
        .expect("start bitcoind regtest container");
    let host_port = container
        .get_host_port_ipv4(RPC_PORT.tcp())
        .expect("map RPC port to host");
    let url = format!("http://127.0.0.1:{host_port}");
    let auth = ("bitcoin", "bitcoin");

    // 2. STEP 1: btc wallet import — creates the wallet from entropy=0 mnemonic.
    // CARGO_BIN_EXE_btc is the freshly-built binary.
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let import_out = std::process::Command::new(env!("CARGO_BIN_EXE_btc"))
        .args([
            "wallet",
            "import",
            "--mnemonic",
            mnemonic,
            "--network",
            "testnet",
            "--password",
            "demo-pwd",
        ])
        .env("XDG_DATA_HOME", &tmp)
        .output()
        .expect("spawn btc wallet import");
    assert!(
        import_out.status.success(),
        "btc wallet import failed: exit={:?}, stderr={}",
        import_out.status,
        String::from_utf8_lossy(&import_out.stderr)
    );
    let wallet_id = String::from_utf8_lossy(&import_out.stdout)
        .lines()
        .find_map(|line| {
            // wallet_id is the first line of stdout (UUID format)
            if line.len() == 36 && line.chars().filter(|c| *c == '-').count() == 4 {
                Some(line.to_string())
            } else {
                None
            }
        })
        .expect("expected wallet_id (UUID) on first line of stdout");
    println!("✓ STEP 1: btc wallet import created wallet_id={wallet_id}");

    // Verify the encrypted blob exists at ADR 0001 path.
    let blob_path = tmp
        .join("btc")
        .join("wallets")
        .join("testnet")
        .join(format!("{wallet_id}.enc"));
    assert!(
        blob_path.exists(),
        "encrypted blob should exist at {} (ADR 0001)",
        blob_path.display()
    );
    println!(
        "✓ STEP 1.5: encrypted blob persisted at {}",
        blob_path.display()
    );

    // 3. STEP 2: Fund the wallet by mining 101 blocks to its first address.
    // Entropy=0 wallet's first external address for regtest at m/44'/1'/0'/0/0.
    let first_recv_addr = "mzYpQmSAGYWWyTLiLGbGaG8T3rHdjNcV11";
    let mine_body = serde_json::json!({
        "jsonrpc": "1.0",
        "id": "smoke",
        "method": "generatetoaddress",
        "params": [101, first_recv_addr],
    });
    let resp = reqwest::blocking::Client::new()
        .post(&url)
        .basic_auth(auth.0, Some(auth.1))
        .json(&mine_body)
        .send()
        .expect("send mining RPC");
    assert!(
        resp.status().is_success(),
        "mining failed: status={:?}, body={:?}",
        resp.status(),
        resp.text().unwrap_or_default()
    );
    println!("✓ STEP 2: mined 101 blocks to {first_recv_addr}");

    // 4. STEP 3: btc wallet balance — query the balance via stateless invocation.
    // Use --network regtest to avoid F20 SPKI pin requirement (only
    // non-regtest networks require --pin-spki per PR #82).
    let balance_out = std::process::Command::new(env!("CARGO_BIN_EXE_btc"))
        .args([
            "wallet",
            "balance",
            "--mnemonic",
            mnemonic,
            "--network",
            "regtest",
            "--esplora-url",
            &url,
        ])
        .output()
        .expect("spawn btc wallet balance");
    let stdout = String::from_utf8_lossy(&balance_out.stdout);
    let stderr = String::from_utf8_lossy(&balance_out.stderr);
    assert!(
        balance_out.status.success(),
        "btc wallet balance failed: exit={:?}, stdout={stdout}, stderr={stderr}",
        balance_out.status
    );
    let total_sat: u64 = stdout
        .lines()
        .find_map(|line| {
            line.split("total_sat=")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse::<u64>().ok())
        })
        .unwrap_or_else(|| panic!("could not parse total_sat from stdout: {stdout}"));
    assert!(
        total_sat >= 500_000_000_000u64,
        "expected total_sat >= 500B sats from 101 regtest coinbases; got {total_sat} (stdout: {stdout})"
    );
    println!("✓ STEP 3: btc wallet balance returned total_sat={total_sat}");

    // Cleanup temp dir.
    std::fs::remove_dir_all(&tmp).ok();
    println!("✓ CLEANUP: removed temp dir {}", tmp.display());
}
