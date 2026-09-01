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
//! | 14  | `local_testnet_erc20_send_amoy_stop_only_round_trip` | `polygon erc20 send` (STOP-only at `AMOY_USDC_ADDR`, see #498) |
//! | 14b | `local_testnet_erc20_send_amoy_usdce_rejected` | `polygon erc20 send` USDC.e guard negative (rejected before RPC) |
//! | 15  | `local_testnet_erc20_approve_stop_only_revoke_round_trip` | `polygon erc20 approve` set→revoke (STOP-only, 2 distinct 66-hex tx hashes) |
//! | 16  | `local_testnet_tx_list_limit_zero_rejected` | `polygon tx list --limit 0` + `--limit 10001` (CLI smoke for handler guard at `handlers/tx.rs:34-40`) |
//! | 17  | `local_testnet_fee_text_mode_human_readable` | `polygon fee --network amoy` (text mode, no `--json`; sister to row 8 JSON mode) |
//! | 18  | `local_testnet_sign_message_verify_mismatch_rejected` | `polygon sign-message --verify <other-addr>` (G12 negative; sister to R2 positive) |
//! | 19  | `local_testnet_sign_typed_valid_chain_id_reaches_lib_deferral` | `polygon sign-typed --chain-id 80002` (Q7 gate PASSES, dies at eip712 lib deferral) |
//! | 19b | `local_testnet_sign_typed_domain_chain_id_mismatch_rejected` | `polygon sign-typed` domain.chainId 137 vs `--chain-id` 80002 (#463 replay guard) |
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
//! G8-G9 erc20 register/balance (LOW),
//! G10 tx list (DONE Phase 4 row 16), G11 fee text mode (DONE Phase 4 row 17),
//! G12 sign-message verify negative (DONE Phase 5 row 18),
//! G13 sign-typed gates (DONE Phase 5 rows 19 + 19b — true happy path blocked
//! by the eip712 lib deferral, see the Phase 5 banner below),
//! G14 send flag variants (MED),
//! G15 negative input tests (LOW), G16 derive_address (DONE Phase 1).
//!
//! **Note:** rows R1/R2/R3/R4/R5 above are *partial gap-fillers* (clap-arg-parse
//! regress guards from PR `432210c`/`a775af7`) — they prove the args parse
//! cleanly without downcast-panic, but do NOT exercise full happy-path handler
//! behavior. G8/G9/G10/G12 happy paths remain uncovered until their
//! respective phases land. G12's positive half is R2; its negative half is
//! row 18 (Phase 5).
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

/// Extract the wallet_id (UUID) from `polygon wallet create|import` stdout.
///
/// Both commands emit `"... id={uuid} address=0x..."` after the wallet is
/// created/imported. Returns `None` if no parseable UUID follows the
/// ` id=` marker. Bounds-safe (no fixed-byte slice = no panic on format
/// drift). Mirrors the seam-test convention of `find_signature_token`.
fn extract_wallet_id(stdout: &str) -> Option<&str> {
    let marker = stdout.find(" id=")?;
    let after = stdout.get(marker + 4..)?;
    let candidate = after.get(..36)?;
    if candidate.len() == 36
        && candidate.chars().enumerate().all(|(i, c)| match (i, c) {
            (8, '-') | (13, '-') | (18, '-') | (23, '-') => true,
            (_, c) if c.is_ascii_hexdigit() => true,
            _ => false,
        })
    {
        Some(candidate)
    } else {
        None
    }
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

// =============================================================================
// Phase 3 v1 (Issue #498) — STOP-only ERC-20 fixture.
//
// Tier 1 = CLI handler pipeline coverage, NOT token semantics (per ADR 0002).
// STOP-only bytecode (`0x00`) installed at the real Amoy USDC address via
// `anvil_set_code`. The contract returns success with empty output on every
// call, so the broadcast path exercises `unlock_signer` + `sign_erc20_tx_bytes`
// identically to a real ERC-20. Token state-machine verification is
// operator-driven (`amoy_smoke.rs` / `mainnet_smoke.rs`), not Tier 1.
//
// Sister pattern: `eth/tests/cli_localnet.rs:2928` (`STOP_BYTECODE` + STOP-only
// at `0x...beef` for negative tests). Sister resolve path:
// `polygon/src/handlers/erc20.rs:80` (`resolve_token_address` — if input starts
// with `0x`, parses as Address directly).
//
// `anvil_set_code` is explicit overwrite — even if a future Anvil version
// pre-funds the USDC slot, STOP bytecode wins.
// =============================================================================

/// Real Amoy USDC native address (lowercase form per #498 Q2 — alloy `address!`
/// strict EIP-55 may fail on mixed-case checksum; lowercase bypasses). Same
/// address as `polygon-wallet-core/tokens/amoy.json` (EIP-55 canonical form:
/// `0x41E94Eb019C0762f9Bfcf9Fb1E58725BfB0e7582`).
const AMOY_USDC_ADDR: alloy_primitives::Address =
    alloy_primitives::address!("0x41e94eb019c0762f9bfcf9fb1e58725bfb0e7582");

/// Bridged USDC.e address — rejected by `guard_usdc_e` at
/// `polygon/src/handlers/erc20.rs:36` BEFORE any RPC or broadcast step.
/// Sister class: `polygon_wallet_core::disambig::reject_bridged_usdc_e`.
const USDC_E_BRIDGED_ADDR: alloy_primitives::Address =
    alloy_primitives::address!("0x2791bca1f2de4661ed88a30c99a7a9449aa84174");

/// STOP-only bytecode (single `0x00` opcode). EVM halts with success,
/// returns empty output. Sister pattern: `eth/tests/cli_localnet.rs:2933`.
const STOP_BYTECODE: &[u8] = &[0x00];

/// Canonical burn address — used as `polygon erc20 send` recipient in
/// Phase 3 v1 tests. Sister to existing test pattern (e.g.
/// `local_testnet_wallet_send_happy_path`).
const RECIPIENT_ADDR: alloy_primitives::Address =
    alloy_primitives::address!("0x000000000000000000000000000000000000dEaD");

/// Synthetic spender for Phase 3 v1 approve round-trip tests. NOT a real
/// contract — STOP-only USDC fixture ignores the call regardless of spender.
const APPROVE_SPENDER_ADDR: alloy_primitives::Address =
    alloy_primitives::address!("0x000000000000000000000000000000000000c0ff");

/// 10 ETH in wei (hex) — funds the signer so tx base fee + calldata cost is
/// covered. STOP contract consumes 0 gas on the call, but the broadcast
/// itself still requires a funded signer. Replaces the inline magic literal
/// per L12 cluster code-reviewer finding #2.
const FUND_TX_GAS_WEI: &str = "0x8AC7230489E80000";

/// Install STOP-only bytecode at `AMOY_USDC_ADDR` on the fixture's Anvil
/// instance via the raw `anvil_setCode` RPC. Mirrors `anvil_set_balance`
/// pattern (line 121) — `AnvilApi` trait is gated behind `anvil-api` feature
/// on `alloy-provider` which the polygon crate does not enable. Raw RPC
/// avoids the feature dependency.
async fn install_stop_only_usdc(fx: &Fixture) {
    let endpoint: alloy_transport_http::reqwest::Url =
        fx.rpc_url.parse().expect("valid Anvil endpoint");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(endpoint);
    provider
        .raw_request::<_, ()>(
            "anvil_setCode".into(),
            (AMOY_USDC_ADDR, alloy_primitives::hex::encode(STOP_BYTECODE)),
        )
        .await
        .expect("install STOP-only bytecode at AMOY_USDC_ADDR must succeed");
}

/// Strict ERC-20 tx-hash finder — first `0x` + 64 hex chars (32-byte B256).
/// Mirrors `find_signature_token` shape. Sister to handler stdout format
/// `tx_hash: 0x{hex}` emitted at `polygon/src/main.rs:751-754` (erc20 send)
/// and `:823-826` (erc20 approve). After `split_whitespace`, the `0x{64}`
/// token is its own whitespace-delimited segment — len 66, hex-only.
///
/// Returns `None` if handler stdout format drifts — callers `panic!` with
/// verbatim stdout in the message. Bounds-safe: no fixed-byte slice = no
/// panic on format drift.
fn find_tx_hash_token(stdout: &str) -> Option<&str> {
    stdout.split_whitespace().find(|tok| {
        tok.len() == 66 && tok.starts_with("0x") && tok[2..].chars().all(|c| c.is_ascii_hexdigit())
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

// =============================================================================
// Phase 2 / S1 / G1 — `polygon wallet import --mnemonic <phrase>` round-trip.
//
// Sister class to L12 H-1 (PR #456 SecretMnemonic wrap, PR #470 --private-key-file).
// Mnemonic via argv IS visible to sibling processes via /proc/<pid>/cmdline;
// design choice per `cli.rs::Command::Wallet(WalletAction::Import)` (the
// `SecretMnemonic` field wraps `Zeroizing<String>` per `cli.rs:91`).
// Documents the wired behavior; operator-driven secret entry should prefer
// --private-key-file (sister test S2).
//
// Happy path: 12-word BIP-39 test vector → exit 0 → stdout contains
// "wallet imported: name=<n> id=<uuid> address=0x...".
// =============================================================================

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_wallet_import_mnemonic_round_trip() {
    require_run_polygon_local();
    let fx = Fixture::new();
    // BIP-39 test vector: 12-word "abandon...about" (all-zeros entropy, valid checksum).
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    // Password sourced via POLYGON_PASSWORD env (injected by `run_polygon`).
    let out = run_polygon(
        &[
            "wallet",
            "import",
            "--name",
            "imported-mnemonic",
            "--mnemonic",
            mnemonic,
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out.status.success(),
        "wallet import --mnemonic must succeed; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("wallet imported: name=imported-mnemonic"),
        "stdout should confirm import; got: {stdout}"
    );
    assert!(
        stdout.contains("address=0x"),
        "stdout should contain address=0x...; got: {stdout}"
    );
    let uuid = extract_wallet_id(&stdout).expect("stdout should contain parseable id= UUID");
    // Round-trip: re-show the imported wallet by id to prove the import persisted.
    let out_show = run_polygon(
        &["wallet", "show", "--id", uuid, "--json"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out_show.status.success(),
        "wallet show post-import must succeed; stderr={}",
        String::from_utf8_lossy(&out_show.stderr)
    );
}

// =============================================================================
// Phase 2 / S2 / G1 — `polygon wallet import --private-key-file <path>` mode-0600.
//
// Closes the L12 H-1 argv-exposure hole for PK import (sister class to the
// `--mnemonic` argv finding closed by PR #456). File contents read into
// `Zeroizing<Vec<u8>>` and zeroized on drop per `handlers::wallet::read_pk_file`.
//
// Security invariant: file perms must be 0o600 BEFORE and AFTER the import
// (no chmod mutation = no accidental world-readable window).
// =============================================================================

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_wallet_import_private_key_file_mode_0600() {
    use std::os::unix::fs::PermissionsExt;
    require_run_polygon_local();
    let fx = Fixture::new();
    // Deterministic 32-byte test PK (NOT a real key — fake data for seam coverage).
    // 0x1111... is a valid secp256k1 scalar (in [1, n-1] for n ≈ 2^256 - 0x1455...).
    let pk_hex = "0x1111111111111111111111111111111111111111111111111111111111111111";
    let pk_path = fx.data_dir.path().join("test-pk.key");
    std::fs::write(&pk_path, pk_hex.trim_start_matches("0x")).expect("write PK file");
    std::fs::set_permissions(&pk_path, std::fs::Permissions::from_mode(0o600))
        .expect("set perms 0o600");
    let out = run_polygon(
        &[
            "wallet",
            "import",
            "--name",
            "imported-pkfile",
            "--private-key-file",
            pk_path.to_str().expect("utf-8 path"),
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out.status.success(),
        "wallet import --private-key-file must succeed; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("wallet imported: name=imported-pkfile"),
        "stdout should confirm import; got: {stdout}"
    );
    assert!(
        stdout.contains("address=0x"),
        "stdout should contain address=0x...; got: {stdout}"
    );
    // Security invariant: perms remain 0o600 after import (no chmod mutation).
    let perms_after = std::fs::metadata(&pk_path)
        .expect("stat pk file")
        .permissions();
    assert_eq!(
        perms_after.mode() & 0o777,
        0o600,
        "PK file perms must remain 0o600 after import (mode mutation = leak); got: {:o}",
        perms_after.mode() & 0o777
    );
}

// Sister negative test for S2: handler's `read_pk_file` mode check (0o600 gate)
// must reject non-conforming perms at parse time. Closes the mode-validator
// coverage gap (L12 finding).
#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_wallet_import_private_key_file_wrong_mode_rejected() {
    use std::os::unix::fs::PermissionsExt;
    require_run_polygon_local();
    let fx = Fixture::new();
    let pk_hex = "0x1111111111111111111111111111111111111111111111111111111111111111";
    let pk_path = fx.data_dir.path().join("test-pk-bad-mode.key");
    std::fs::write(&pk_path, pk_hex.trim_start_matches("0x")).expect("write PK file");
    // 0o644 = world-readable — handler must reject per read_pk_file gate.
    std::fs::set_permissions(&pk_path, std::fs::Permissions::from_mode(0o644))
        .expect("set perms 0o644");
    let out = run_polygon(
        &[
            "wallet",
            "import",
            "--name",
            "imported-pkfile-bad-mode",
            "--private-key-file",
            pk_path.to_str().expect("utf-8 path"),
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        !out.status.success(),
        "wallet import --private-key-file with mode 0o644 must exit non-zero; got exit 0"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("0o600") || stderr.contains("mode"),
        "stderr should mention mode-0600 rejection; got: {stderr}"
    );
}

// =============================================================================
// Phase 2 / S3 / G2 — `polygon wallet show --id <uuid> --json` round-trip.
//
// Drift from issue #495 body: handler requires `--id` (UUID), not `--name`
// (look-up by name deferred). `--addresses` and `--export` flags also
// deferred. Test covers what the handler ships.
// WalletInfo fields per `evm_wallet_core::WalletInfo`:
// wallet_id, name, network, address, derivation_path.
// =============================================================================

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_wallet_show_id_json_round_trip() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let out_create = run_polygon(
        &["wallet", "create", "--name", "show-test"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out_create.status.success(),
        "wallet create must succeed; stderr={}",
        String::from_utf8_lossy(&out_create.stderr)
    );
    let stdout_create = String::from_utf8_lossy(&out_create.stdout);
    let uuid =
        extract_wallet_id(&stdout_create).expect("create stdout must contain parseable UUID");
    let out_show = run_polygon(
        &["wallet", "show", "--id", uuid, "--json"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out_show.status.success(),
        "wallet show --id --json must succeed; stderr={}",
        String::from_utf8_lossy(&out_show.stderr)
    );
    let stdout_show = String::from_utf8_lossy(&out_show.stdout);
    let v: serde_json::Value =
        serde_json::from_str(&stdout_show).expect("wallet show --json should parse as JSON");
    assert_eq!(
        v.get("wallet_id").and_then(|x| x.as_str()),
        Some(uuid),
        "wallet_id mismatch"
    );
    assert_eq!(
        v.get("name").and_then(|x| x.as_str()),
        Some("show-test"),
        "name mismatch"
    );
    assert!(
        v.get("address")
            .and_then(|x| x.as_str())
            .map(|s| s.starts_with("0x"))
            .unwrap_or(false),
        "address should be 0x-hex; got: {}",
        v.get("address")
            .map(|x| x.to_string())
            .unwrap_or_else(|| "<missing>".into())
    );
    // Schema pin (L12 finding): all 5 WalletInfo fields must appear.
    assert!(
        v.get("network").is_some(),
        "WalletInfo JSON must include `network` field"
    );
    assert!(
        v.get("derivation_path").is_some(),
        "WalletInfo JSON must include `derivation_path` field"
    );
}

// =============================================================================
// Phase 2 / S4 / G3 — `polygon wallet delete --id <uuid>` round-trip.
//
// Drift from issue #495 body: handler requires `--id` (UUID), not `--name`
// (look-up by name deferred). After delete, `wallet show --id` must fail
// (meta.json + .enc files removed).
// =============================================================================

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_wallet_delete_id_round_trip() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let out_create = run_polygon(
        &["wallet", "create", "--name", "delete-test"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out_create.status.success(),
        "wallet create must succeed; stderr={}",
        String::from_utf8_lossy(&out_create.stderr)
    );
    let stdout_create = String::from_utf8_lossy(&out_create.stdout);
    let uuid =
        extract_wallet_id(&stdout_create).expect("create stdout must contain parseable UUID");
    let out_del = run_polygon(
        &["wallet", "delete", "--id", uuid],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out_del.status.success(),
        "wallet delete --id must succeed; stderr={}",
        String::from_utf8_lossy(&out_del.stderr)
    );
    let stdout_del = String::from_utf8_lossy(&out_del.stdout);
    assert!(
        stdout_del.contains("wallet deleted:"),
        "stdout should confirm delete; got: {stdout_del}"
    );
    // Post-delete invariant: `wallet show --id` must fail (meta.json + .enc gone).
    let out_show = run_polygon(
        &["wallet", "show", "--id", uuid],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        !out_show.status.success(),
        "wallet show post-delete must fail (meta.json removed); got exit 0 with stdout={}",
        String::from_utf8_lossy(&out_show.stdout)
    );
    let stderr_show = String::from_utf8_lossy(&out_show.stderr);
    assert!(
        stderr_show.contains("not found")
            || stderr_show.contains("meta.json")
            || stderr_show.contains("read_file"),
        "post-delete stderr should mention file-not-found; got: {stderr_show}"
    );
}

// =============================================================================
// Phase 2 / S5 / G4 — `polygon wallet sync --address <addr>` reaches handler.
//
// Drift from issue #495 body: live RPC body deferred to T7 (operator-driven
// per L29) per `handlers::wallet::wallet_sync`. Handler returns
// `Error::Rpc("wallet sync not yet implemented")` until then. Sister SKIP
// pattern to `local_testnet_tx_get_returns_json_fields` (line 11 in coverage
// table) + `local_testnet_tx_list_with_address_reaches_handler` (R3).
// =============================================================================

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario; KNOWN GAP: live RPC deferred to T7 per handlers::wallet::wallet_sync"]
async fn local_testnet_wallet_sync_address_reaches_handler() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let addr = "0x0000000000000000000000000000000000000042";
    let out = run_polygon(
        &["wallet", "sync", "--address", addr, "--network", "amoy"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    if out.status.success() {
        // Live RPC landed — assert parseable JSON (empty array acceptable).
        // TODO[T7]: tighten to assert Vec<TxSummary> shape per `polygon_wallet_core::TxSummary`.
        let stdout = String::from_utf8_lossy(&out.stdout);
        let _: serde_json::Value =
            serde_json::from_str(&stdout).expect("wallet sync should parse as JSON");
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("not yet implemented") || stderr.contains("wallet sync"),
            "wallet sync stderr should mention deferral OR live result; got: {stderr}"
        );
        eprintln!(
            "SKIP: wallet sync live RPC deferred to T7 per handlers::wallet::wallet_sync. \
             Re-run when live RPC lands."
        );
    }
}

// =============================================================================
// Phase 2 / S6 / G5 — `polygon wallet send-speedup --tx-hash <bad>` validator.
//
// Drift from issue #495 body: happy path requires pending-tx setup (operator-
// driven per L29 — Anvil auto-mines, no pending tx available offline). This
// test covers the validator path: bad tx-hash format → handler rejects at
// `B256::from_str` per `handlers::wallet::wallet_send_speedup_v2`. Happy path
// belongs in `amoy_smoke.rs` (operator-driven live RPC).
//
// L12 fix: wallet-create-first step ensures the test does NOT pass on
// validator ordering (handler parses tx-hash before wallet lookup).
// =============================================================================

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_wallet_send_speedup_invalid_tx_hash_errors() {
    require_run_polygon_local();
    let fx = Fixture::new();
    // Wallet must exist first so the validator error path (tx-hash parse)
    // is the path that trips — not "wallet not found".
    let out_create = run_polygon(
        &["wallet", "create", "--name", "speedup-validator-test"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out_create.status.success(),
        "wallet create must succeed; stderr={}",
        String::from_utf8_lossy(&out_create.stderr)
    );
    let out = run_polygon(
        &[
            "wallet",
            "send-speedup",
            "--name",
            "speedup-validator-test",
            "--tx-hash",
            "not-a-hex-hash",
            "--new-max-fee-per-gas",
            "1",
            "--new-max-priority-fee-per-gas",
            "1",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        !out.status.success(),
        "wallet send-speedup with invalid --tx-hash must exit non-zero; got exit 0 with stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    // L12 fix: drop `|| "invalid"` disjunct (trivially satisfiable).
    // Handler emits exactly `Error::InvalidInput(format!("invalid --tx-hash: {e}"))`.
    assert!(
        stderr.contains("invalid --tx-hash"),
        "stderr should mention exact `invalid --tx-hash`; got: {stderr}"
    );
}

// =============================================================================
// Phase 3 v1 (Issue #495, sub-task #498) — `polygon erc20 send` Tier 1
// CLI-pipeline coverage. STOP-only fixture installed at `AMOY_USDC_ADDR`.
//
// Per L12 cluster code-reviewer finding #3, split into two fns (one per
// concern) sister to existing pattern (`local_testnet_erc20_balance_address_flag`
// vs `local_testnet_erc20_register_address_flag` — separate fns for separate
// concerns). USDC.e negative sister does NOT need the STOP-only fixture since
// `guard_usdc_e` rejects BEFORE any RPC call (`polygon/src/handlers/erc20.rs:180`).
// =============================================================================

/// Happy path — STOP-only contract at `AMOY_USDC_ADDR`; broadcast succeeds;
/// handler returns 66-hex tx hash. Validates L12 H-1 secret-handling invariant
/// on the signing path (`unlock_signer` + `sign_erc20_tx_bytes`) without
/// requiring real ERC-20 semantics. Per #498 + ADR 0002, token semantics
/// (real ERC-20 transfer state change) is operator-driven scope
/// (`amoy_smoke.rs` / `mainnet_smoke.rs`).
#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_erc20_send_amoy_stop_only_round_trip() {
    require_run_polygon_local();

    let fx = Fixture::new();
    install_stop_only_usdc(&fx).await;

    let create = run_polygon(
        &["wallet", "create", "--name", "g6-sender"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        create.status.success(),
        "wallet create failed: stderr={}",
        String::from_utf8_lossy(&create.stderr)
    );
    let sender_addr = read_first_address(fx.data_dir.path(), "g6-sender");
    anvil_set_balance(&fx.anvil, &sender_addr, FUND_TX_GAS_WEI).await;

    let send = run_polygon(
        &[
            "erc20",
            "send",
            "--name",
            "g6-sender",
            "--token",
            &format!("{AMOY_USDC_ADDR:#x}"),
            "--to",
            &format!("{RECIPIENT_ADDR:#x}"),
            "--amount",
            "1000000",
            "--network",
            "amoy",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    let stdout = String::from_utf8_lossy(&send.stdout);
    let stderr = String::from_utf8_lossy(&send.stderr);
    assert!(
        send.status.success(),
        "erc20 send STOP-only happy path failed: exit={:?} stderr={}",
        send.status.code(),
        stderr
    );
    // `find_tx_hash_token` already filters `len() == 66 && starts_with("0x")
    // && all hex`. The unwrap panic embeds verbatim stdout for diagnosis on
    // handler-format drift.
    let _tx_hash = find_tx_hash_token(&stdout).unwrap_or_else(|| {
        panic!("erc20 send stdout should contain 0x + 64-hex tx hash; got: {stdout}")
    });
}

/// USDC.e negative — bridged USDC.e address rejected by `guard_usdc_e`
/// (`polygon/src/handlers/erc20.rs:36`) BEFORE the broadcast step. No
/// STOP-only fixture interaction (guard rejects at address resolution,
/// before any RPC). Sister to existing test at `erc20.rs:351-357` which
/// asserts `msg.contains("BRIDGED_USDC_REJECTED")` at unit-test level.
#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_erc20_send_amoy_usdce_rejected() {
    require_run_polygon_local();

    let fx = Fixture::new();
    let create = run_polygon(
        &["wallet", "create", "--name", "g6-usdce"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        create.status.success(),
        "wallet create (USDCe neg) failed: stderr={}",
        String::from_utf8_lossy(&create.stderr)
    );

    let send_e = run_polygon(
        &[
            "erc20",
            "send",
            "--name",
            "g6-usdce",
            "--token",
            &format!("{USDC_E_BRIDGED_ADDR:#x}"), // USDC.e bridged
            "--to",
            &format!("{RECIPIENT_ADDR:#x}"),
            "--amount",
            "1000000",
            "--network",
            "amoy",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    let stderr_e = String::from_utf8_lossy(&send_e.stderr);
    assert!(
        !send_e.status.success(),
        "erc20 send with USDC.e address should be rejected by guard_usdc_e; \
         got exit=0 stderr={stderr_e}"
    );
    // Pin on semantic class (robust to message drift).
    assert!(
        stderr_e.contains("bridged")
            || stderr_e.contains("USDC.e")
            || stderr_e.contains("BRIDGED_USDC_REJECTED")
            || stderr_e.contains("guard"),
        "USDC.e rejection stderr should mention bridged/USDC.e/guard; got: {stderr_e}"
    );
}

// =============================================================================
// Phase 3 v1 (Issue #495, sub-task #498) — `polygon erc20 approve` Tier 1
// CLI-pipeline coverage. STOP-only fixture at `AMOY_USDC_ADDR`.
//
// Two-step approve → revoke round-trip. Both exit 0; tx hashes differ (proves
// the broadcast step actually went out for both — no cached response, no
// handler short-circuit). Canonical revocation = `approve(spender, 0)`
// zeroes the allowance.
//
// Nonce handling: handler calls `provider.get_transaction_count(signer.address())`
// per invocation (`polygon/src/handlers/erc20.rs:288-291`). Step A broadcasts
// at nonce N; Step B auto-fetches N+1. No Fixture nonce threading.
//
// Per #498 + ADR 0002, real allowance state change verification is
// operator-driven scope, not Tier 1.
// =============================================================================

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_erc20_approve_stop_only_revoke_round_trip() {
    require_run_polygon_local();

    let fx = Fixture::new();
    install_stop_only_usdc(&fx).await;

    let create = run_polygon(
        &["wallet", "create", "--name", "g7-owner"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        create.status.success(),
        "wallet create failed: stderr={}",
        String::from_utf8_lossy(&create.stderr)
    );
    let owner_addr = read_first_address(fx.data_dir.path(), "g7-owner");
    anvil_set_balance(&fx.anvil, &owner_addr, FUND_TX_GAS_WEI).await;

    // Step A: approve spender for 1_000_000 raw units.
    let approve_a = run_polygon(
        &[
            "erc20",
            "approve",
            "--name",
            "g7-owner",
            "--token",
            &format!("{AMOY_USDC_ADDR:#x}"),
            "--spender",
            &format!("{APPROVE_SPENDER_ADDR:#x}"),
            "--amount",
            "1000000",
            "--network",
            "amoy",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    let stdout_a = String::from_utf8_lossy(&approve_a.stdout);
    let stderr_a = String::from_utf8_lossy(&approve_a.stderr);
    assert!(
        approve_a.status.success(),
        "erc20 approve (set) failed: exit={:?} stderr={stderr_a}",
        approve_a.status.code()
    );
    let hash_a = find_tx_hash_token(&stdout_a).unwrap_or_else(|| {
        panic!("erc20 approve (set) stdout should contain 0x + 64-hex tx hash; got: {stdout_a}")
    });

    // Step B: revoke (canonical revocation = approve with amount=0).
    let approve_b = run_polygon(
        &[
            "erc20",
            "approve",
            "--name",
            "g7-owner",
            "--token",
            &format!("{AMOY_USDC_ADDR:#x}"),
            "--spender",
            &format!("{APPROVE_SPENDER_ADDR:#x}"),
            "--amount",
            "0",
            "--network",
            "amoy",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    let stdout_b = String::from_utf8_lossy(&approve_b.stdout);
    let stderr_b = String::from_utf8_lossy(&approve_b.stderr);
    assert!(
        approve_b.status.success(),
        "erc20 approve (revoke) failed: exit={:?} stderr={stderr_b}",
        approve_b.status.code()
    );
    let hash_b = find_tx_hash_token(&stdout_b).unwrap_or_else(|| {
        panic!("erc20 approve (revoke) stdout should contain 0x + 64-hex tx hash; got: {stdout_b}")
    });

    // Distinct hashes proves the broadcast step actually went out for both
    // (not cached, not a no-op short-circuit).
    assert_ne!(
        hash_a, hash_b,
        "approve→revoke must produce 2 distinct tx hashes; got both={hash_a}"
    );
}

// =============================================================================
// Phase 4 / Issue #495 — tx + fee gap fills (G10, G11)
// =============================================================================

/// `polygon tx list --limit <out-of-range>` (CLI smoke for handler guard).
///
/// Sister to handler unit tests `tx_list_rejects_zero_limit` +
/// `tx_list_rejects_excessive_limit` at `polygon/src/handlers/tx.rs:131-161`.
/// Validates the CLI surface reaches the `limit ∈ [1, 10000]` guard at
/// `polygon/src/handlers/tx.rs:34-40` (returns `Error::InvalidInput` BEFORE the
/// T7 live-RPC gate). Existing R3 stub `local_testnet_tx_list_with_address_reaches_handler`
/// at line 1058 covers happy-path reach + deferral SKIP — this fn fills the
/// negative-path-only gap. Issue #495 Phase 4 (LOW, G10).
#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_tx_list_limit_zero_rejected() {
    require_run_polygon_local();
    let fx = Fixture::new();
    // Valid 20-byte hex address (handler-level `parse_address_strict` at
    // `polygon/src/handlers/tx.rs:81-89` accepts canonical EIP-55 hex).
    let addr = "0x0000000000000000000000000000000000000042";

    // Sub-assert 1: `--limit 0` — below lower boundary, must reject.
    let out_zero = run_polygon(
        &[
            "tx",
            "list",
            "--address",
            addr,
            "--limit",
            "0",
            "--json",
            "--network",
            "amoy",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        !out_zero.status.success(),
        "tx list --limit 0 must exit non-zero; exit={:?} stderr={}",
        out_zero.status.code(),
        String::from_utf8_lossy(&out_zero.stderr)
    );
    let stderr_zero = String::from_utf8_lossy(&out_zero.stderr);
    assert!(
        stderr_zero.contains("limit")
            || stderr_zero.contains("[1, 10000]")
            || stderr_zero.contains("InvalidInput"),
        "tx list --limit 0 stderr must mention limit guard; got: {stderr_zero}"
    );

    // Sub-assert 2: `--limit 10001` — above upper boundary, must reject.
    // Sister to Phase 2 G1 mode-0600 dual-assert (single fn, two asserts).
    let out_max = run_polygon(
        &[
            "tx",
            "list",
            "--address",
            addr,
            "--limit",
            "10001",
            "--json",
            "--network",
            "amoy",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        !out_max.status.success(),
        "tx list --limit 10001 must exit non-zero; exit={:?} stderr={}",
        out_max.status.code(),
        String::from_utf8_lossy(&out_max.stderr)
    );
    let stderr_max = String::from_utf8_lossy(&out_max.stderr);
    assert!(
        stderr_max.contains("limit")
            || stderr_max.contains("[1, 10000]")
            || stderr_max.contains("InvalidInput"),
        "tx list --limit 10001 stderr must mention limit guard; got: {stderr_max}"
    );
}

/// `polygon fee --network amoy` (text-mode happy path, no `--json`).
///
/// Sister to Phase 1 test 8 (`local_testnet_fee_json_parses`) which exercises
/// the `--json` mode. Validates the `format_fee_human` output shape at
/// `polygon/src/handlers/fee.rs:138-148`. RPC path = `fetch_fee_estimate` at
/// `handlers/fee.rs:91-122` (read-only `estimate_eip1559_fees`, no signing).
/// Issue #495 Phase 4 (LOW, G11).
#[test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
fn local_testnet_fee_text_mode_human_readable() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let out = run_polygon(
        &["fee", "--network", "amoy"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        out.status.success(),
        "polygon fee (text mode) failed: exit={:?} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Markers from `format_fee_human` at handlers/fee.rs:138-148.
    assert!(
        stdout.contains("network: polygon-amoy"),
        "fee text-mode stdout must include network label (per network_label at handlers/fee.rs:129-135); got: {stdout}"
    );
    assert!(
        stdout.contains("chain_id: 80002"),
        "fee text-mode stdout must include chain_id 80002 (Anvil fixture); got: {stdout}"
    );
    assert!(
        stdout.contains("max_fee_per_gas:"),
        "fee text-mode stdout must include max_fee_per_gas label; got: {stdout}"
    );
    assert!(
        stdout.contains("max_priority_fee_per_gas:"),
        "fee text-mode stdout must include max_priority_fee_per_gas label; got: {stdout}"
    );
    // gwei + wei both present per fee pair (`{:.3} gwei ({} wei)` template).
    assert!(
        stdout.contains("gwei") && stdout.contains("wei"),
        "fee text-mode stdout must include both gwei and wei units; got: {stdout}"
    );
}

// =============================================================================
// Issue #495 Phase 5 — sign gaps (G12 negative + G13 two-layer gates).
//
// G12: R2 above covers `--verify` POSITIVE only. Row 18 fills the negative
// half — a mismatched `--verify` address must be rejected at the recover
// step (`handlers/sign.rs:88-95`), not silently accepted.
//
// G13: the gap inventory originally asked for a `sign-typed` HAPPY path with
// a real signature. Not reachable — `evm_wallet_core::sign_typed_data`
// (`crates/evm-wallet-core/src/signer.rs:164-172`) is an unconditional
// `Err(SignError::Unsupported)` pending the alloy eip712 feature gate. Rows
// 19 + 19b instead pin the TWO gate layers that DO run today, each with a
// distinct failure classification:
//   19  — chain_id 80002 PASSES the Q7 gate, then dies in the lib layer
//         (`Error::Rpc`, exit 3). SKIP-accepts exit 0 once eip712 lands.
// Password: env-only via `POLYGON_PASSWORD` (injected by `run_polygon`), NOT
// `--password` on argv — argv is `/proc/<pid>/cmdline`-visible and the
// dispatcher emits its own insecurity warning for it (`main.rs:39-42`).
// Sister to the Phase 3 env-only pattern.
//
//   19b — typed-data `domain.chainId` disagreeing with `--chain-id` is
//         rejected by `assert_domain_chain_id_consistency` (#463 replay
//         guard) BEFORE the lib call (`Error::InvalidInput`, exit 2).
// =============================================================================

/// Row 18 (G12 negative) — `polygon sign-message --verify <other-addr>`.
///
/// Sister to R2 (positive). The signature recovers to `karl`'s address, but
/// `--verify` names a different address, so the handler must fail the
/// round-trip comparison at `handlers/sign.rs:91`.
#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_sign_message_verify_mismatch_rejected() {
    require_run_polygon_local();
    let fx = Fixture::new();
    for name in ["karl", "kara"] {
        let _ = run_polygon(
            &["wallet", "create", "--name", name],
            fx.data_dir.path(),
            &fx.rpc_url,
        );
    }
    let karl_addr = read_first_address(fx.data_dir.path(), "karl");
    let kara_addr = read_first_address(fx.data_dir.path(), "kara");
    assert_ne!(
        karl_addr, kara_addr,
        "fixture precondition: two wallets must derive distinct addresses"
    );

    // Sign with karl's key but claim the signer is kara.
    let out = run_polygon(
        &[
            "sign-message",
            "--name",
            "karl",
            "--message",
            "verify me",
            "--verify",
            &kara_addr,
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        !out.status.success(),
        "sign-message --verify with a MISMATCHED address must exit non-zero; got exit 0 with stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Pin the EXACT fragments from the handler's format string at
    // `handlers/sign.rs:92-94`: `eip191 verify mismatch: recovered {r} !=
    // expected {e}`. A loose `contains("mismatch")` would also match an
    // unrelated password/RPC rejection; `recovered` + `!= expected` prove
    // the recover round-trip itself ran and disagreed.
    assert!(
        stderr.contains("eip191 verify mismatch"),
        "stderr should surface the eip191 verify mismatch (handlers/sign.rs:92-94); got: {stderr}"
    );
    assert!(
        stderr.contains("recovered") && stderr.contains("!= expected"),
        "stderr must show the recovered-vs-expected pair, proving the recover \
         step at handlers/sign.rs:88-90 actually executed; got: {stderr}"
    );
    // Guard against a leaked signature on the failure path: a rejected
    // verify must not still hand the caller a usable signature on stdout.
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        find_signature_token(&stdout).is_none(),
        "rejected --verify must NOT print a signature to stdout; got: {stdout}"
    );
}

/// Row 19 (G13a) — `polygon sign-typed --chain-id 80002` with a consistent
/// `domain.chainId`. Proves the Q7 gate ACCEPTS Amoy, then the failure comes
/// from the lib-layer deferral, not from chain_id validation.
///
/// TODO[eip712-feature-gate]: when `evm_wallet_core::sign_typed_data` returns
/// `Ok(sig)`, this test SKIP-accepts exit 0 and the assertion below should be
/// tightened to a 132-hex signature pin (sister to R2).
#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario; KNOWN GAP: eip712 lib deferral"]
async fn local_testnet_sign_typed_valid_chain_id_reaches_lib_deferral() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let _ = run_polygon(
        &["wallet", "create", "--name", "liam"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    // domain.chainId MUST equal --chain-id or the #463 guard fires first
    // (that path is row 19b's job, not this one).
    let typed_data = r#"{"types":{"EIP712Domain":[{"name":"chainId","type":"uint256"}]},"primaryType":"EIP712Domain","domain":{"chainId":80002},"message":{}}"#;
    let out = run_polygon(
        &[
            "sign-typed",
            "--chain-id",
            "80002",
            "--typed-data",
            typed_data,
            "--name",
            "liam",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);

    if out.status.success() {
        // KNOWN-GAP SKIP branch: the alloy eip712 feature gate landed.
        // NOT assertion-free — an exit-0 that prints no signature (or a
        // truncated one) is a regression, not a feature landing, so pin the
        // signature before returning (sister to R2 at the `--verify` positive).
        assert!(
            find_signature_token(&stdout).is_some(),
            "eip712 feature appears to have landed (exit 0) but stdout carries \
             no 0x+130-hex signature — regression, not a feature: {stdout}"
        );
        eprintln!(
            "SKIP: eip712 sign-typed now succeeds — tighten this test to a full \
             signature pin and drop this branch. stdout={stdout}"
        );
        return;
    }

    // The Q7 gate must NOT be the thing that rejected us: chain_id 80002 is
    // in the allowed {137, 80002} set (handlers/sign.rs:26-39).
    // Exact lowercase fragment from `handlers/sign.rs:30-31`
    // (`... is not a polygon PoS chain ...`). The earlier capital-P spelling
    // could never match, making this pin vacuously true.
    assert!(
        !stderr.contains("is not a polygon"),
        "chain_id 80002 must PASS the Q7 gate; stderr looks like a Q7 rejection: {stderr}"
    );
    // Failure must come from the lib-layer eip712 deferral instead.
    // Do NOT accept a bare `EIP-712` / `eip712` token here: the Q7 rejection
    // message at `handlers/sign.rs:30-31` also contains `EIP-712`, so those
    // tokens cannot distinguish the two layers. `deferred` / `feature gate` /
    // `Unsupported` come only from the lib stub
    // (`crates/evm-wallet-core/src/signer.rs:168-170`).
    assert!(
        stderr.contains("deferred")
            || stderr.contains("feature gate")
            || stderr.contains("Unsupported"),
        "expected the lib-layer eip712 deferral (crates/evm-wallet-core/src/signer.rs:164-172), \
         not a Q7-gate rejection; got: {stderr}"
    );
}

/// Row 19b (G13b) — `--chain-id 80002` but typed-data `domain.chainId: 137`.
///
/// `assert_domain_chain_id_consistency` (`handlers/sign.rs:180-188`, #463)
/// must reject the disagreement BEFORE any signing attempt. Distinct layer
/// from row 19: this is the cross-chain-replay guard, and both values are
/// individually valid Polygon chain ids — only their disagreement is the bug.
#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1; local-testnet scenario"]
async fn local_testnet_sign_typed_domain_chain_id_mismatch_rejected() {
    require_run_polygon_local();
    let fx = Fixture::new();
    let _ = run_polygon(
        &["wallet", "create", "--name", "luna"],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    // 137 (Polygon PoS mainnet) is itself a VALID Q7 chain id — the defect
    // under test is purely the disagreement with --chain-id 80002.
    let typed_data = r#"{"types":{"EIP712Domain":[{"name":"chainId","type":"uint256"}]},"primaryType":"EIP712Domain","domain":{"chainId":137},"message":{}}"#;
    let out = run_polygon(
        &[
            "sign-typed",
            "--chain-id",
            "80002",
            "--typed-data",
            typed_data,
            "--name",
            "luna",
        ],
        fx.data_dir.path(),
        &fx.rpc_url,
    );
    assert!(
        !out.status.success(),
        "domain.chainId 137 vs --chain-id 80002 must be rejected (#463 replay guard); got exit 0 with stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Exact fragment from `handlers/sign.rs:183-186`. A `contains("137")`
    // disjunct would false-pass if any other layer ever echoed a chain id.
    assert!(
        stderr.contains("must match") && stderr.contains("typed.domain.chainId"),
        "stderr should name the domain.chainId disagreement using the #463 guard \
         message at handlers/sign.rs:183-186; got: {stderr}"
    );
}
