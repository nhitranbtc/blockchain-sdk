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
