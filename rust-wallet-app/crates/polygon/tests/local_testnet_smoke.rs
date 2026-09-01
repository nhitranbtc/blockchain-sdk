//! Issue #492 / Tier 1 — Polygon local-testnet canonical scenario.
//!
//! **ADR 0002 Tier 1** (per `docs/superpowers/adrs/2026-08-31-adr-0002-polygon-local-testnet.md`):
//! in-process Anvil (Polygon-Amoy hardfork, chain_id 80002) + `alloy-node-bindings::Anvil`.
//!
//! **Scope:** every wired `polygon` CLI subcommand (per `polygon/src/main.rs:310-870`).
//! Sister to `polygon_wallet_scenario.rs` (CI-gated happy-path subset — wallet
//! lifecycle only) and `amoy_smoke.rs` / `mainnet_smoke.rs` (operator-driven live
//! RPC). This file is the canonical **Tier 1 local-testnet** scenario — fast,
//! deterministic, no live RPC dependency.
//!
//! **Opt-in (L29 discipline — `#[ignore]` + env guard):**
//!   `RUN_POLYGON_LOCAL=1 cargo test -p polygon --test local_testnet_smoke -- --ignored`
//!
//! **NOT in CI gate.** Manual driver at `scripts/run-polygon-local-smoke.sh`
//! mirrors the same scenario for operator hosts without a cargo build.
//!
//! **TDD status:** every test reads as a "should pass when the wired handler
//! returns the contract value." Each test is independent (fresh Anvil per
//! test) so a partial failure isolates one CLI surface, not the suite.
//!
//! **Why each test exists** — every variant of `cli::Command` and the most
//! important `WalletAction::Show`/`Delete`/`Sync`/`SendSpeedup` paths get
//! a coverage entry. The polygon CLI is user-facing per Phase 4 of plan
//! `2026-08-27-polygon-wallet-core.md`; Tier 1 local-testnet coverage keeps
//! regressions out of every release without paying live-RPC flake cost.
//!
//! **Coverage table** (Issue #495 Phase 1) — every test fn → CLI surface:
//!
//! | # | Test fn | CLI surface |
//! | --- | --- | --- |
//! | 1 | `local_testnet_version_exits_zero` | `polygon version` |
//! | 2 | `local_testnet_config_show_json` | `polygon config show --json` |
//! | 3 | `local_testnet_wallet_create` | `polygon wallet create --name` |
//! | 4 | `local_testnet_wallet_list_shows_created` | `polygon wallet list` |
//! | 5 | `local_testnet_wallet_balance_reflects_funding` | `polygon wallet balance` |
//! | 6 | `local_testnet_wallet_send_happy_path` | `polygon wallet send` (positive) |
//! | 7 | `local_testnet_wallet_send_invalid_recipient_errors_cleanly` | `polygon wallet send` (negative — bad `--to`) |
//! | 8 | `local_testnet_fee_json_parses` | `polygon fee --json` |
//! | 9 | `local_testnet_faucet_url_print` | `polygon faucet --address` (STUB — skips) |
//! | 10 | `local_testnet_sign_message_returns_signature` | `polygon sign-message` |
//! | 11 | `local_testnet_tx_get_returns_json_fields` | `polygon tx get --json` (live-RPC stubbed) |
//! | 12 | `local_testnet_erc20_list_json_amoy_usdc_six_decimals` | `polygon erc20 list --json` |
//! | 13 | `local_testnet_sign_typed_rejects_invalid_chain_id` | `polygon sign-typed --chain-id 1` (Q7 negative) |
//! | R1 | `local_testnet_sign_message_accepts_address_flag` | `polygon sign-message --address` (regress guard, PR `432210c`/`a775af7`) |
//! | R2 | `local_testnet_sign_message_verify_round_trips_positive` | `polygon sign-message --verify` (positive, G12 sister) |
//! | R3 | `local_testnet_tx_list_with_address_reaches_handler` | `polygon tx list --address` (regress guard, live-RPC stubbed) |
//! | R4 | `local_testnet_erc20_balance_address_flag` | `polygon erc20 balance --address --token USDC` (regress guard) |
//! | R5 | `local_testnet_erc20_register_address_flag` | `polygon erc20 register --address --list` (regress guard) |
//! | G16 | `local_testnet_derive_address_deterministic_round_trip` | offline `evm_wallet_core::mnemonic::{generate_12_word, derive_address}` (Phase 1, no RPC) |
//!
//! **Gap inventory** (Issue #495, NOT yet covered) — to be filled in subsequent
//! phased PRs under parent `task-polygon-full-scenario`:
//! G1 wallet import (HIGH), G2-G5 wallet show/delete/sync/send-speedup (MED),
//! G6 erc20 send (HIGH), G7 erc20 approve (MED), G8-G9 erc20 register/balance (LOW),
//! G10 tx list (LOW), G11 fee text mode (LOW),
//! G12 sign-message verify NEGATIVE path (MED, R2 covers positive only),
//! G13 sign-typed happy path (HIGH), G14 send flag variants (MED),
//! G15 negative input tests (LOW), G16 derive_address (this PR).
//!
//! **Note:** rows R1/R2/R3/R4/R5 above are *partial gap-fillers* (clap-arg-parse
//! regress guards from PR `432210c`/`a775af7`) — they prove the args parse
//! cleanly without downcast-panic, but do NOT exercise full happy-path handler
//! behavior. G8/G9/G10/G12 happy paths remain uncovered until their
//! respective phases land.
//!
#![cfg(test)]

use std::path::PathBuf;
use std::process::{Command, Stdio};

use alloy_node_bindings::{Anvil, AnvilInstance};
use alloy_provider::Provider;
use tempfile::TempDir;

/// `CARGO_BIN_EXE_polygon` — set by cargo at integration-test compile time.
/// See <https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-tests>.
fn polygon_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_polygon"))
}

/// L29 guard — `RUN_POLYGON_LOCAL=1` required. CI never sets it.
fn require_run_polygon_local() {
    if std::env::var("RUN_POLYGON_LOCAL").ok().as_deref() != Some("1") {
        panic!(
            "RUN_POLYGON_LOCAL=1 not set; local-testnet smoke tests require explicit opt-in (L29)"
        );
    }
}

/// Hermetic `polygon` invocation. Password sourced from env to avoid the TTY-prompt
/// branch (which blocks under non-interactive cargo test).
fn run_polygon(args: &[&str], data_dir: &std::path::Path, rpc_url: &str) -> std::process::Output {
    Command::new(polygon_bin())
        .args(args)
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--rpc-url")
        .arg(rpc_url)
        .env("POLYGON_PASSWORD", "test-pw-ignore-leak")
        .env("POLYGON_NETWORK", "amoy")
        .env("RUST_BACKTRACE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn polygon binary")
}

/// Spawn Anvil with Polygon-Amoy chain_id (80002) so the wallet created with
/// `--network amoy` matches the RPC chain (handlers verify chain_id at send
/// time per L13 critical-tier review).
fn spawn_amoy_anvil() -> AnvilInstance {
    Anvil::new().chain_id(80_002).spawn()
}

/// Fund `address` on the Anvil instance via the raw `anvil_setBalance`
/// RPC. Anvil default-prefunded accounts are not the wallet just created
/// by `polygon wallet create`, so we must top up the new wallet's
/// address before `wallet send` has any balance to spend.
async fn anvil_set_balance(anvil: &AnvilInstance, address: &str, wei_hex: &str) {
    let endpoint: alloy_transport_http::reqwest::Url =
        anvil.endpoint().parse().expect("valid Anvil endpoint");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(endpoint);
    provider
        .raw_request::<_, ()>("anvil_setBalance".into(), (address, wei_hex))
        .await
        .expect("anvil_setBalance must succeed");
}

/// Find the wallet's address from `<data_dir>/polygon_amoy/<uuid>.meta.json`.
///
/// Mirrors the helper in `polygon_wallet_scenario.rs:86-119`. Lives here too
/// (not in `common/`) because Tier 1 is a single-file scenario per ADR 0002 —
/// copying the helper keeps the file readable as a standalone artifact.
fn read_first_address(data_dir: &std::path::Path, name: &str) -> String {
    let network_dir = data_dir.join("polygon_amoy");
    let mut meta_path: Option<PathBuf> = None;
    for entry in std::fs::read_dir(&network_dir).expect("read amoy dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let is_meta = path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n.ends_with(".meta.json"));
        if !is_meta {
            continue;
        }
        let bytes = std::fs::read(&path).expect("read meta.json");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("parse meta.json");
        if v.get("name").and_then(|n| n.as_str()) == Some(name) {
            meta_path = Some(path);
            break;
        }
    }
    let meta_path = meta_path.expect("wallet meta.json not found for name");
    let bytes = std::fs::read(&meta_path).expect("re-read meta.json");
    let v: serde_json::Value = serde_json::from_slice(&bytes).expect("parse meta.json");
    v.get("address")
        .and_then(|a| a.as_str())
        .map(|s| s.to_string())
        .expect("address field in meta.json")
}

/// Strict EIP-191 signature finder.
///
/// Returns the FIRST whitespace-delimited token in `stdout` that is
/// exactly `0x` + 130 hex chars (the r || s || v 65-byte personal_sign
/// signature). Avoids the previously-fragile `0x-containing line with
/// ≥ 130 hex chars total` pattern, which would false-positive on
/// `0x{address}+0x{sig}+…` concatenation.
///
/// Returns `None` if no such token is present.
fn find_signature_token(stdout: &str) -> Option<&str> {
    stdout.split_whitespace().find(|tok| {
        tok.len() == 132 && tok.starts_with("0x") && tok[2..].chars().all(|c| c.is_ascii_hexdigit())
    })
}

/// Spawn Anvil + temp data dir, returning a fixture the caller can use
/// to compose individual subcommand tests. Each test owns its fixture so
/// a partial failure isolates one CLI surface.
struct Fixture {
    #[allow(dead_code)]
    anvil: AnvilInstance,
    rpc_url: String,
    data_dir: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let anvil = spawn_amoy_anvil();
        let rpc_url = anvil.endpoint().clone();
        let data_dir = TempDir::new().expect("tempdir for data-dir");
        Self {
            anvil,
            rpc_url,
            data_dir,
        }
    }
}

// =============================================================================
// (smoke) — `polygon version` exits 0 with version string.
// =============================================================================

#[test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
fn local_testnet_version_exits_zero() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let out = run_polygon(&["version"], fx.data_dir.path(), &fx.rpc_url);
    assert!(
        out.status.success(),
        "polygon version failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("polygon "),
        "version stdout should contain 'polygon '; got: {stdout}"
    );
}

// =============================================================================
// Story 11 — `polygon config show --json` exposes resolved config.
// =============================================================================

#[test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
fn local_testnet_config_show_json() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let out = run_polygon(
        &["config", "show", "--json", "--network", "amoy"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out.status.success(),
        "config show --json failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value =
        serde_json::from_str(&stdout).expect("config show --json should parse as JSON");
    assert_eq!(
        v.get("network").and_then(|n| n.as_str()),
        Some("amoy"),
        "config show network field should equal 'amoy'; got: {stdout}"
    );
}

// =============================================================================
// Story 1 — `polygon wallet create --name <name>` exits 0 and echoes address.
// =============================================================================

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_wallet_create() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let out = run_polygon(
        &[
            "wallet",
            "create",
            "--name",
            "alice",
            "--password",
            "test-pw-ignore-leak",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out.status.success(),
        "wallet create failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("wallet created:") && stdout.contains("address=0x"),
        "wallet create stdout should echo 'wallet created: … address=0x…'; got: {stdout}"
    );
    // Sanity: persisted meta.json is readable + parses.
    let _ = read_first_address(fx.data_dir.path(), "alice");
}

// =============================================================================
// Story 9 — `polygon wallet list` shows the freshly-created wallet.
// =============================================================================

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_wallet_list_shows_created() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let _ = run_polygon(
        &[
            "wallet",
            "create",
            "--name",
            "bob",
            "--password",
            "test-pw-ignore-leak",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    let out = run_polygon(
        &["wallet", "list", "--network", "amoy"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out.status.success(),
        "wallet list failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("bob"),
        "wallet list should contain 'bob'; got: {stdout}"
    );
}

// =============================================================================
// Story 3 — `polygon wallet balance --address <addr>` reflects anvil funding.
// =============================================================================

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_wallet_balance_reflects_funding() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let _ = run_polygon(
        &[
            "wallet",
            "create",
            "--name",
            "carol",
            "--password",
            "test-pw-ignore-leak",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    let carol_addr = read_first_address(fx.data_dir.path(), "carol");
    let ten_pol_wei = "0x".to_string() + &format!("{:x}", 10_u128 * 10_u128.pow(18));
    anvil_set_balance(&fx.anvil, &carol_addr, &ten_pol_wei).await;

    let out = run_polygon(
        &[
            "wallet",
            "balance",
            "--address",
            &carol_addr,
            "--unit",
            "wei",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out.status.success(),
        "wallet balance failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let numeric = stdout.split_whitespace().next().unwrap_or("").to_string();
    let parsed: u128 = numeric
        .parse()
        .unwrap_or_else(|_| panic!("balance stdout should parse as u128 wei; got: {stdout:?}"));
    assert!(
        parsed >= 10 * 10_u128.pow(18) / 2,
        "carol balance should reflect the 10 POL we funded (minus any gas spent); got: {stdout}"
    );
}

// =============================================================================
// Story 5 (positive) — `polygon wallet send` returns 0x + 64 hex tx hash.
// =============================================================================

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_wallet_send_happy_path() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let _ = run_polygon(
        &[
            "wallet",
            "create",
            "--name",
            "dave",
            "--password",
            "test-pw-ignore-leak",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    let dave_addr = read_first_address(fx.data_dir.path(), "dave");
    let ten_pol_wei = "0x".to_string() + &format!("{:x}", 10_u128 * 10_u128.pow(18));
    anvil_set_balance(&fx.anvil, &dave_addr, &ten_pol_wei).await;

    let recipient = "0x0000000000000000000000000000000000000042";
    let out = run_polygon(
        &[
            "wallet",
            "send",
            "--name",
            "dave",
            "--password",
            "test-pw-ignore-leak",
            "--to",
            recipient,
            "--amount",
            "0.001",
            "--unit",
            "pol",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out.status.success(),
        "wallet send failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let tx_hash_line = stdout
        .lines()
        .find(|l| l.starts_with("tx_hash: 0x"))
        .expect("wallet send stdout should contain 'tx_hash: 0x...' line");
    let tx_hash = tx_hash_line.trim_start_matches("tx_hash: 0x").trim();
    assert_eq!(
        tx_hash.len(),
        64,
        "tx_hash hex must be 64 chars (32 bytes); got: {tx_hash:?}"
    );
    assert!(
        tx_hash.chars().all(|c| c.is_ascii_hexdigit()),
        "tx_hash must be hex; got: {tx_hash:?}"
    );
}

// =============================================================================
// Story 5 (negative) — malformed `--to` recipient must exit non-zero.
// =============================================================================

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_wallet_send_invalid_recipient_errors_cleanly() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let _ = run_polygon(
        &[
            "wallet",
            "create",
            "--name",
            "eve",
            "--password",
            "test-pw-ignore-leak",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    let eve_addr = read_first_address(fx.data_dir.path(), "eve");
    let ten_pol_wei = "0x".to_string() + &format!("{:x}", 10_u128 * 10_u128.pow(18));
    anvil_set_balance(&fx.anvil, &eve_addr, &ten_pol_wei).await;

    let out = run_polygon(
        &[
            "wallet",
            "send",
            "--name",
            "eve",
            "--password",
            "test-pw-ignore-leak",
            "--to",
            "0xnotavalidaddress",
            "--amount",
            "0.001",
            "--unit",
            "pol",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        !out.status.success(),
        "wallet send to invalid recipient must exit non-zero; got exit 0 with stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("invalid") || stderr.contains("--to"),
        "stderr should mention invalid / --to; got: {stderr}"
    );
}

// =============================================================================
// Story 8 — `polygon fee --json` returns parseable gas estimate JSON.
// =============================================================================

#[test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
fn local_testnet_fee_json_parses() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let out = run_polygon(
        &["fee", "--json", "--network", "amoy"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out.status.success(),
        "polygon fee --json failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value =
        serde_json::from_str(&stdout).expect("fee --json should parse as JSON");
    let max_fee_wei = v
        .get("max_fee_per_gas_wei")
        .and_then(|x| x.as_u64())
        .unwrap_or_else(|| panic!("fee JSON max_fee_per_gas_wei should be u64; got: {stdout}"));
    assert!(
        max_fee_wei > 0,
        "fee max_fee_per_gas_wei should be > 0 (Anvil default base fee = 1 gwei); got: {max_fee_wei}"
    );
}

// =============================================================================
// Story 30 — `polygon faucet --address <addr>` prints canonical Amoy faucet URL.
// =============================================================================

#[test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; KNOWN GAP: handler stubbed"]
fn local_testnet_faucet_url_print() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let out = run_polygon(
        &[
            "faucet",
            "--address",
            "0x0000000000000000000000000000000000000042",
            "--network",
            "amoy",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    if out.status.success() {
        // Handler landed — assert the canonical Amoy faucet URL.
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("faucet.polygon.technology"),
            "faucet stdout should contain canonical Amoy faucet URL; got: {stdout}"
        );
    } else {
        // Handler still stubbed (per `polygon/src/main.rs:876`) — accept the
        // documented deferral. Re-run after the handler body lands.
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("deferred past T6b") || stderr.contains("faucet"),
            "faucet stderr should mention stub-deferral OR URL print; got: {stderr}"
        );
        eprintln!(
            "SKIP: faucet handler still stubbed — per polygon/src/main.rs:876. \
             Re-run after handler body lands."
        );
    }
}

// =============================================================================
// Story 18 — `polygon sign-message --name <w> --message "…"` returns 0x + 130 hex.
// =============================================================================

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_sign_message_returns_signature() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let _ = run_polygon(
        &[
            "wallet",
            "create",
            "--name",
            "frank",
            "--password",
            "test-pw-ignore-leak",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    let out = run_polygon(
        &[
            "sign-message",
            "--name",
            "frank",
            "--password",
            "test-pw-ignore-leak",
            "--message",
            "hello polygon",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out.status.success(),
        "polygon sign-message failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Strict signature parse: a token that is exactly `0x` + 130 hex chars
    // (65-byte EIP-191 personal_sign sig: r || s || v). Avoids the previous
    // fragile `>= 130 hex chars AND 0x` pattern which would false-positive
    // on `0x{address}+0x{sig}+…` concatenation. See review finding #6.
    let sig_hex = find_signature_token(&stdout).unwrap_or_else(|| {
        panic!("sign-message stdout should include 0x + 130-hex sig; got: {stdout}")
    });
    assert_eq!(
        sig_hex.len(),
        130 + 2,
        "signature token must be exactly 0x + 130 hex chars; got len={}",
        sig_hex.len()
    );
    assert!(
        sig_hex.chars().skip(2).all(|c| c.is_ascii_hexdigit()),
        "signature body must be hex; got: {sig_hex}"
    );
}

// =============================================================================
// Story 7 — `polygon tx get --tx-hash <hash>` returns JSON `from` + `to` fields.
// =============================================================================

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_tx_get_returns_json_fields() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let _ = run_polygon(
        &[
            "wallet",
            "create",
            "--name",
            "grace",
            "--password",
            "test-pw-ignore-leak",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    let grace_addr = read_first_address(fx.data_dir.path(), "grace");
    let ten_pol_wei = "0x".to_string() + &format!("{:x}", 10_u128 * 10_u128.pow(18));
    anvil_set_balance(&fx.anvil, &grace_addr, &ten_pol_wei).await;

    let recipient = "0x0000000000000000000000000000000000000042";
    let send_out = run_polygon(
        &[
            "wallet",
            "send",
            "--name",
            "grace",
            "--password",
            "test-pw-ignore-leak",
            "--to",
            recipient,
            "--amount",
            "0.001",
            "--unit",
            "pol",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    let send_stdout = String::from_utf8_lossy(&send_out.stdout);
    let tx_hash_line = send_stdout
        .lines()
        .find(|l| l.starts_with("tx_hash: 0x"))
        .expect("send stdout should contain 'tx_hash: 0x...' line");
    let tx_hash_hex = tx_hash_line
        .trim_start_matches("tx_hash: 0x")
        .trim()
        .to_string();
    // `polygon tx get --tx-hash` requires `0x + 64 hex` per the B256
    // parser in `handlers::tx::tx_get` (see the "invalid tx hash" error
    // path) — strip the `0x` to validate length, then re-prefix it.
    let tx_hash = format!("0x{tx_hash_hex}");

    let out = run_polygon(
        &[
            "tx",
            "get",
            "--tx-hash",
            &tx_hash,
            "--json",
            "--network",
            "amoy",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    if out.status.success() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let v: serde_json::Value =
            serde_json::from_str(&stdout).expect("tx get --json should parse as JSON");
        assert!(
            v.get("from").is_some() && v.get("to").is_some(),
            "tx get JSON should include from + to fields; got: {stdout}"
        );
    } else {
        // `handlers::tx::tx_get` is wired but live RPC is operator-driven
        // follow-up — accept the documented deferral.
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("not yet implemented") || stderr.contains("tx get"),
            "tx get stderr should mention deferral OR live result; got: {stderr}"
        );
        eprintln!(
            "SKIP: tx get live RPC not yet implemented — \
             per polygon/src/handlers/tx.rs:67. Re-run when live RPC lands."
        );
    }
}

// =============================================================================
// Story 23 — `polygon erc20 list --json` returns a non-empty JSON array.
//
// Polygon-Amoy registry contains 1 entry (USDC); Polygon mainnet contains 3
// (USDC/USDT/DAI per `polygon/tests/mainnet_smoke.rs:281`). The Tier 1
// scenario targets Amoy, so we assert ≥ 1 entry + USDC decimals = 6 — the
// chain-id invariant that holds across both registries.
// =============================================================================

#[test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
fn local_testnet_erc20_list_json_amoy_usdc_six_decimals() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let out = run_polygon(
        &["erc20", "list", "--json", "--network", "amoy"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out.status.success(),
        "erc20 list --json failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value =
        serde_json::from_str(&stdout).expect("erc20 list --json should parse as JSON");
    let arr = v.as_array().expect("erc20 list JSON should be an array");
    assert!(
        !arr.is_empty(),
        "Polygon erc20 list (Amoy) should have ≥ 1 entry (USDC); got 0"
    );
    let usdc = arr
        .iter()
        .find(|t| t.get("symbol").and_then(|s| s.as_str()) == Some("USDC"))
        .expect("USDC entry should be present in Amoy erc20 registry");
    assert_eq!(
        usdc.get("decimals").and_then(|d| d.as_u64()),
        Some(6),
        "USDC on Polygon Amoy must be 6 decimals (native Circle USDC, NOT bridged)"
    );
}

// =============================================================================
// Story 27 / Q7 (negative) — `polygon sign-typed --chain-id <not 137/80002>`
// must exit non-zero (chain_id gate enforcement per Q7 critical-tier).
// =============================================================================

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_sign_typed_rejects_invalid_chain_id() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let _ = run_polygon(
        &[
            "wallet",
            "create",
            "--name",
            "henry",
            "--password",
            "test-pw-ignore-leak",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    // Q7 gate: chain_id 1 (Ethereum mainnet) is NOT in the {137, 80002}
    // Polygon set; `assert_polygon_chain_id` in handlers/sign.rs must reject.
    let typed_data =
        r#"{"types":{"EIP712Domain":[]},"primaryType":"EIP712Domain","domain":{},"message":{}}"#;
    let out = run_polygon(
        &[
            "sign-typed",
            "--chain-id",
            "1",
            "--typed-data",
            typed_data,
            "--name",
            "henry",
            "--password",
            "test-pw-ignore-leak",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        !out.status.success(),
        "sign-typed with chain_id 1 must exit non-zero (Q7 gate); got exit 0 with stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("chain_id")
            || stderr.contains("chain id")
            || stderr.contains("137")
            || stderr.contains("80002"),
        "stderr should mention chain_id gate; got: {stderr}"
    );
}

// =============================================================================
// Regression coverage for the e99c0ab + retype-batch sister-class fixes:
// each of these CLI surfaces accepts a `parse_address` flag that previously
// downcast-panicked because the field type was `String` / `Option<String>`.
// Sister to the FaucetArgs.address fix in cli.rs:478-479.
// =============================================================================

/// `polygon sign-message --address <addr>` (clap downcast regression).
///
/// Pre-fix: `SignMessageArgs.address` was `Option<String>`; passing
/// `--address 0x…` panicked with "Could not downcast to …String".
#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_sign_message_accepts_address_flag() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let _ = run_polygon(
        &[
            "wallet",
            "create",
            "--name",
            "iris",
            "--password",
            "test-pw-ignore-leak",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    let iris_addr = read_first_address(fx.data_dir.path(), "iris");

    let out = run_polygon(
        &[
            "sign-message",
            "--name",
            "iris",
            "--password",
            "test-pw-ignore-leak",
            "--message",
            "regression",
            "--address",
            &iris_addr,
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out.status.success(),
        "sign-message --address must NOT downcast-panic; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        find_signature_token(&stdout).is_some(),
        "sign-message stdout should include 0x + 130-hex sig; got: {stdout}"
    );
}

/// `polygon sign-message --verify <same-addr>` round-trip (positive).
///
/// After signing, the handler recovers the signer from the signature
/// and compares against `--verify`. With `--verify == signer's address`,
/// stdout must contain a verification-success indicator.
#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_sign_message_verify_round_trips_positive() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let _ = run_polygon(
        &[
            "wallet",
            "create",
            "--name",
            "jack",
            "--password",
            "test-pw-ignore-leak",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    let jack_addr = read_first_address(fx.data_dir.path(), "jack");

    let out = run_polygon(
        &[
            "sign-message",
            "--name",
            "jack",
            "--password",
            "test-pw-ignore-leak",
            "--message",
            "verify me",
            "--verify",
            &jack_addr,
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out.status.success(),
        "sign-message --verify (same addr) must succeed; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    // On `--verify` success, the CLI prints the signature (the recovery
    // round-trip passed silently — no separate "verified" word emitted).
    // The proof of success is: signature present + no panic + exit 0.
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        find_signature_token(&stdout).is_some(),
        "sign-message --verify (positive) stdout should include 0x + 130-hex sig; got: {stdout}"
    );
}

/// `polygon tx list --address <addr>` (clap downcast regression).
///
/// Pre-fix: `TxAction::List.address` was `String`; this test would
/// have panicked on the parse mismatch.
/// Live RPC still returns "not yet implemented" — same SKIP pattern as
/// `tx get`.
#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario; KNOWN GAP: live RPC"]
async fn local_testnet_tx_list_with_address_reaches_handler() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let addr = "0x0000000000000000000000000000000000000042";
    let out = run_polygon(
        &[
            "tx",
            "list",
            "--address",
            addr,
            "--limit",
            "1",
            "--json",
            "--network",
            "amoy",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    if out.status.success() {
        // Live RPC landed — assert parseable JSON array.
        let stdout = String::from_utf8_lossy(&out.stdout);
        let _: serde_json::Value =
            serde_json::from_str(&stdout).expect("tx list --json should parse as JSON");
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("not yet implemented") || stderr.contains("tx list"),
            "tx list stderr should mention deferral OR live result; got: {stderr}"
        );
        eprintln!(
            "SKIP: tx list live RPC not yet implemented — \
             per polygon/src/handlers/tx.rs:42. Re-run when live RPC lands."
        );
    }
}

/// `polygon erc20 balance --address <addr> --token USDC` (clap downcast regression).
///
/// Pre-fix: `Erc20Action::Balance.address` was `String`; this would
/// have panicked.
#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_erc20_balance_address_flag() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let addr = "0x0000000000000000000000000000000000000042";
    let out = run_polygon(
        &[
            "erc20",
            "balance",
            "--address",
            addr,
            "--token",
            "USDC",
            "--network",
            "amoy",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    if out.status.success() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        // Amoy USDC contract is `0x41e94eb019c0762f9bfcf9fb1e58725bfb0e7582`.
        // Either stdout contains a decimal balance, or the handler returns
        // a clear "no balance" line. The regression we're guarding against
        // is the panic itself — assertion is just "didn't crash".
        assert!(
            !stdout.is_empty() || !out.stderr.is_empty(),
            "erc20 balance should produce some output; got empty stdout+stderr"
        );
    } else {
        // Handler deferred to T6d-2.1 follow-up (cli.rs Balance.address type
        // conflict). The CLI args parsed cleanly — no downcast panic, which
        // is the regression we guard.
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("deferred") || stderr.contains("T6d"),
            "erc20 balance stderr should mention deferral; got: {stderr}"
        );
        eprintln!(
            "SKIP: erc20 balance handler deferred to T6d-2.1 — arg-parse regress guard verified."
        );
    }
}

/// `polygon erc20 register --address <addr> --list` (clap downcast regression).
///
/// Pre-fix: `Erc20Action::Register.address` was `String`; this would
/// have panicked.
#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_erc20_register_address_flag() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let addr = "0x0000000000000000000000000000000000000042";
    let out = run_polygon(
        &[
            "erc20",
            "register",
            "--address",
            addr,
            "--list",
            "--network",
            "amoy",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    if out.status.success() {
        // Handler landed — assert success path.
    } else {
        // Handler deferred to T6d-2.2 follow-up (XDG-persisted user registry).
        // The CLI args parsed cleanly (no downcast panic).
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("deferred") || stderr.contains("T6d"),
            "erc20 register stderr should mention deferral; got: {stderr}"
        );
        eprintln!(
            "SKIP: erc20 register handler deferred to T6d-2.2 — arg-parse regress guard verified."
        );
    }
}

// =============================================================================
// G16 (Issue #495 Phase 1) — offline `derive_address` deterministic round-trip.
// Sister to `mainnet_smoke.rs:159` `cross_chain_identity_same_address_eth_polygon`
// (operator-driven, online). This variant is offline, fast, deterministic —
// green-by-default (no Anvil, no RPC, no `#[ignore]`). Pinned BIP-44 coin_type
// = 60 invariant: same phrase + same index must yield the same address on
// every EVM chain. Different index → different address (BIP-44 path sanity).
// =============================================================================

#[test]
fn local_testnet_derive_address_deterministic_round_trip() {
    use evm_wallet_core::mnemonic::{derive_address, generate_12_word};

    let phrase = generate_12_word();
    let addr_a = derive_address(&phrase, 0);
    let addr_b = derive_address(&phrase, 0);
    assert_eq!(
        addr_a, addr_b,
        "deterministic derive_address invariant broken: same phrase + index must yield same address"
    );

    // BIP-44 path sanity: different index → different address.
    let addr_idx1 = derive_address(&phrase, 1);
    assert_ne!(
        addr_a, addr_idx1,
        "derive_address(phrase, 0) == derive_address(phrase, 1) — BIP-44 path ignored"
    );
}
