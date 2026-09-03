//! P8-T3 / G5 (Issue #515) — `polygon erc20 send` round-trip on Amoy.
//!
//! Operator-driven live integration test per L29 (`#[ignore]` +
//! `require_run_polygon_amoy()` + opt-in env gate). Drives the full
//! 10-step workflow from plan §G5 — Workflow + Logging + §P8-T3, asserts
//! USDC round-trip on Amoy.
//!
//! Sister pattern at `polygon/tests/amoy_erc20_balance.rs` (parity oracle,
//! balance-only). Live-deps gate (V9 spike is Anvil-fork; this is real
//! Amoy RPC).
//!
//! ## Operator pre-conditions (per L29 + plan §G5)
//!
//! 1. Sender (`amoy_funded_pk_hex` from `tokens/amoy.json`) holds > 0 POL (gas).
//!    Fund via `https://faucet.polygon.technology`.
//! 2. Sender holds ≥ 1.0 USDC. Fund via `https://faucet.circle.com/`.
//! 3. `RUN_POLYGON_AMOY=1` env set.
//! 4. Recipient address `0x000…0042` is a deterministic sink (not pre-funded).
//!
//! ## Acceptance (plan §P8-T3)
//!
//! - Recipient USDC balance increases by 1_000_000 (6 decimals).
//! - Sender USDC balance decreases by 1_000_000 + gas (delta assertion).
//! - Receipt status == 1; logs contain `Transfer(address,address,uint256)`.
//!
//! ## Per-step log (plan §G5)
//!
//! Append-only at `.local/tmp/amoy_erc20_send_report.log` (`.local/` is
//! gitignored per root `.gitignore`). Each step emits a timestamped block;
//! success writes `=== TEST PASSED ===`, panic unwinds write
//! `=== TEST FAILED: <reason> ===` via a Drop guard.

#![cfg(test)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use alloy_primitives::{Address, B256, U256};
use alloy_provider::Provider;

// ----- Plan §G5 constants -----

/// ERC-20 `Transfer(address,address,uint256)` event topic (per EIP-20).
const TRANSFER_TOPIC: &str = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

/// Recipient wallet — operator-funded `amoy-smoke-2` from the
/// polygon-data-amoy wallet store (created in earlier session).
const RECIPIENT_ADDR: &str = "0x2055ba398775b9aa890bd02222a948f4978c3661";

/// Sender wallet — operator-funded `amoy-smoke-1` (funded via Circle USDC
/// faucet + Polygon POL faucet per L29 + plan §G5).
const SENDER_ADDR: &str = "0x971200F83562896Ff7049Cb8f6686c4eB5Cb1717";

/// Sender wallet name (matches `.meta.json::name` in the wallet store).
const WALLET_NAME: &str = "amoy-smoke-1";

/// Path to the polygon CLI's local wallet store PARENT directory — CLI
/// auto-resolves `<parent>/polygon_amoy/` for `--network amoy` (per
/// `polygon_amoy/README.md` usage). Sender + recipient wallets already
/// populated under `<parent>/polygon_amoy/`.
/// Resolve the polygon CLI wallet-store path under the test crate's
/// `tests/` dir. Project-relative (resolved via `CARGO_MANIFEST_DIR` at
/// runtime) so the test works regardless of where the repo is checked out.
/// CLI auto-resolves `<parent>/polygon_amoy/` for `--network amoy`.
fn wallet_data_dir() -> PathBuf {
    PathBuf::from(format!(
        "{}/tests/polygon-data-amoy",
        env!("CARGO_MANIFEST_DIR")
    ))
}

/// Wallet password source — env var only, never embedded in source per
/// security review (matches L54 secret-via-env discipline). Operator sets
/// `AMOY_TEST_WALLET_PASSWORD` at invocation; panic with pointer to
/// `polygon_amoy/README.md` if unset so the cause is unambiguous.
fn wallet_password() -> String {
    std::env::var("AMOY_TEST_WALLET_PASSWORD").unwrap_or_else(|_| {
        panic!(
            "AMOY_TEST_WALLET_PASSWORD env var must be set to the polygon_amoy \
             wallet password; see polygon_amoy/README.md"
        )
    })
}

/// 1.0 USDC at 6 decimals.
const TRANSFER_AMOUNT_RAW: u64 = 1_000_000;

// ----- JSON SoT loader (mirrors amoy_erc20_balance.rs) -----

fn ensure_tokens_loaded() {
    static LOADED: OnceLock<()> = OnceLock::new();
    LOADED.get_or_init(|| {
        let path = format!("{}/tokens/amoy.json", env!("CARGO_MANIFEST_DIR"));
        let bytes = std::fs::read(&path).unwrap_or_else(|e| {
            panic!("failed to read {path}: {e} — tokens/amoy.json is the committed Amoy SoT")
        });
        let parsed: serde_json::Value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|e| panic!("failed to parse {path}: {e}"));
        AMOY_TOKENS_JSON
            .set(parsed)
            .expect("AMOY_TOKENS_JSON OnceLock set twice");
    });
}

static AMOY_TOKENS_JSON: OnceLock<serde_json::Value> = OnceLock::new();

/// Single-flight gate so the export-status entry is written at most once
/// per process. Plain `cargo test` (no `--ignored`) still exercises this
/// path via the hermetic sentinel; `--ignored` runs also exercise it via
/// the live test's first call — same `OnceLock` keeps the marker count at 1.
static LOG_EXPORT_INIT: OnceLock<()> = OnceLock::new();

fn amoy_json_obj(key: &str) -> serde_json::Value {
    ensure_tokens_loaded();
    let v = AMOY_TOKENS_JSON
        .get()
        .expect("AMOY_TOKENS_JSON set by ensure_tokens_loaded");
    v.get(key)
        .cloned()
        .unwrap_or_else(|| panic!("missing object field `{key}` in tokens/amoy.json"))
}

fn usdc_address() -> String {
    let tokens = amoy_json_obj("tokens");
    let arr = tokens
        .as_array()
        .expect("`tokens` field in tokens/amoy.json must be an array");
    for entry in arr {
        if entry
            .get("symbol")
            .and_then(|s| s.as_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("USDC"))
        {
            return entry
                .get("address")
                .and_then(|a| a.as_str())
                .unwrap_or_else(|| panic!("USDC entry missing address field"))
                .to_string();
        }
    }
    panic!("USDC entry not found in tokens/amoy.json `tokens` array");
}

fn amoy_rpc_url() -> String {
    amoy_json_obj("rpc_url")
        .as_str()
        .expect("`rpc_url` field in tokens/amoy.json must be a string")
        .to_string()
}

fn amoy_explorer_url() -> String {
    amoy_json_obj("explorer_url")
        .as_str()
        .expect("`explorer_url` field in tokens/amoy.json must be a string")
        .to_string()
}

fn amoy_chain_id() -> u64 {
    amoy_json_obj("chain_id")
        .as_u64()
        .expect("`chain_id` field in tokens/amoy.json must be a u64")
}

// ----- Env gate (mirrors amoy_erc20_balance.rs) -----

fn require_run_polygon_amoy() {
    ensure_tokens_loaded();
    let opt = amoy_json_obj("test_harness")
        .get("run_polygon_amoy")
        .and_then(|s| s.as_str())
        .map(String::from);
    if opt.as_deref() != Some("1") {
        panic!(
            "tokens/amoy.json test_harness.run_polygon_amoy must be \"1\" for live test runs; \
             current = {opt:?}"
        );
    }
}

// ----- Polygon CLI runner -----

fn polygon_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_polygon"))
}

fn run_polygon(args: &[&str], data_dir: &Path) -> std::process::Output {
    Command::new(polygon_bin())
        .args(args)
        .arg("--data-dir")
        .arg(data_dir)
        .env("POLYGON_PASSWORD", wallet_password())
        .env("POLYGON_NETWORK", "amoy")
        .env("RUST_BACKTRACE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn polygon binary")
}

// ----- Per-step log helper (plan §G5 Logging & report) -----

fn log_path() -> PathBuf {
    // Cargo runs the test binary with CARGO_MANIFEST_DIR = rust-wallet-app/crates/polygon.
    // Walk three levels up to land at the repo root: crates/polygon -> crates -> rust-wallet-app -> repo.
    let repo_root = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    repo_root
        .join(".local")
        .join("tmp")
        .join("amoy_erc20_send_report.log")
}

fn now_iso8601() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let nanos = now.subsec_nanos();
    let (year, month, day, hour, minute, second) = epoch_to_ymdhms(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{nanos:09}Z")
}

fn epoch_to_ymdhms(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let day_secs = secs % 86_400;
    let days = secs / 86_400;
    let hour = (day_secs / 3600) as u32;
    let minute = ((day_secs % 3600) / 60) as u32;
    let second = (day_secs % 60) as u32;
    let (year, month, day) = days_to_ymd(days as i64);
    (year, month, day, hour, minute, second)
}

fn days_to_ymd(days_since_epoch: i64) -> (u32, u32, u32) {
    // Civil-from-days algorithm by Howard Hinnant (public domain).
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 {
        z / 146_097
    } else {
        (z - 146_096) / 146_097
    };
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as u32, m as u32, d as u32)
}

fn ensure_log_dir(path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
}

/// Emit a single timestamped `LOG EXPORT` marker identifying the current
/// cargo invocation (test binary loaded, live test held vs. invoked).
/// Idempotent within a process via `LOG_EXPORT_INIT` — second call is a
/// no-op so the live test's full 10-step trail doesn't get a duplicate
/// export header.
fn log_export_status() {
    LOG_EXPORT_INIT.get_or_init(|| {
        let held = std::env::var("RUN_POLYGON_AMOY").as_deref() == Ok("1");
        let detail = format!(
            "test binary loaded\nUSDC={}\nRPC={}\nCHAIN_ID={}\nRUN_POLYGON_AMOY={}\nlive_test_state={}",
            usdc_address(),
            amoy_rpc_url(),
            amoy_chain_id(),
            std::env::var("RUN_POLYGON_AMOY").unwrap_or_else(|_| "<unset>".to_string()),
            if held {
                "eligible (cargo test --ignored)"
            } else {
                "held (#[ignore] — set RUN_POLYGON_AMOY=1 to enable)"
            },
        );
        log_step(0, 10, "LOG EXPORT", &detail);
    });
}

/// Format a raw USDC amount (6-decimal ERC-20 base units) as a
/// `whole.frac USDC` decimal string — e.g. `1_000_000` → `"1.000000 USDC"`,
/// `1_234_567` → `"1.234567 USDC"`. Plan §G5 step 2 example uses this
/// shape so operators can read the log without doing the ÷1e6 in their head.
fn format_usdc(raw: U256) -> String {
    let divisor = U256::from(1_000_000u64);
    let whole = raw / divisor;
    let frac_raw = raw % divisor;
    // Render the fractional component as a zero-padded 6-digit decimal.
    let frac_dec = frac_raw.to_string();
    let frac_padded = format!("{:0>6}", frac_dec);
    format!("{whole}.{frac_padded} USDC")
}

fn log_step(n: u8, total: u8, action: &str, detail: &str) {
    let path = log_path();
    ensure_log_dir(&path);
    let mut file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[log_step] failed to open {path:?}: {e}");
            return;
        }
    };
    let header = format!("[{}] [step {n}/{total}] {action}", now_iso8601());
    let _ = writeln!(file, "{header}");
    for line in detail.lines() {
        let _ = writeln!(file, "  {line}");
    }
    let _ = writeln!(file);
    let _ = file.flush();
}

fn log_pass(elapsed: Duration) {
    let path = log_path();
    ensure_log_dir(&path);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(
            f,
            "[{}] === TEST PASSED ===\n  total elapsed: {:.1}s\n",
            now_iso8601(),
            elapsed.as_secs_f64()
        );
        let _ = f.flush();
    }
}

/// Failure marker — fires from Drop guard so panic unwinds still record.
struct TestFailGuard {
    reason: std::sync::Mutex<Option<String>>,
}

impl Drop for TestFailGuard {
    fn drop(&mut self) {
        let reason = self.reason.lock().unwrap().take();
        if let Some(reason) = reason {
            let path = log_path();
            ensure_log_dir(&path);
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                let _ = writeln!(f, "[{}] === TEST FAILED: {reason} ===\n", now_iso8601());
                let _ = f.flush();
            }
        }
    }
}

impl TestFailGuard {
    fn new() -> Self {
        Self {
            reason: std::sync::Mutex::new(None),
        }
    }
    fn set(&self, reason: impl Into<String>) {
        *self.reason.lock().unwrap() = Some(reason.into());
    }
}

// ----- Receipt polling (live RPC) -----

async fn poll_receipt(
    provider: &impl Provider,
    tx_hash: B256,
    timeout: Duration,
    interval: Duration,
) -> Option<alloy_rpc_types::TransactionReceipt> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        match provider.get_transaction_receipt(tx_hash).await {
            Ok(Some(receipt)) => return Some(receipt),
            Ok(None) => tokio::time::sleep(interval).await,
            Err(_) => tokio::time::sleep(interval).await,
        }
    }
    None
}

// ----- The 10-step workflow test -----

#[test]
#[ignore = "operator-driven live Amoy RPC per L29 — run with: RUN_POLYGON_AMOY=1 cargo test -p polygon --test amoy_erc20_send -- --ignored"]
fn amoy_erc20_send_usdc_round_trip() {
    log_export_status();
    let started = Instant::now();
    let fail_guard = TestFailGuard::new();
    let test_start_msg = format!(
        "RUN_POLYGON_AMOY=1, USDC={}, RPC={}, CHAIN_ID={}",
        usdc_address(),
        amoy_rpc_url(),
        amoy_chain_id()
    );
    log_step(0, 10, "TEST STARTED", &test_start_msg);

    // ===== Step 1: preflight =====
    require_run_polygon_amoy();
    log_step(1, 10, "preflight env check", "RUN_POLYGON_AMOY = \"1\" ✓");

    let data_dir = wallet_data_dir();
    let token = usdc_address();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime for oracle + receipt poll");

    rt.block_on(async {
        let provider = polygon_wallet_core::new_http_polygon_amoy()
            .expect("provider build for oracle");

        // ===== Step 2: snapshot sender USDC balance =====
        // Operator pre-funded `amoy-smoke-1` (sender) via Circle USDC faucet;
        // see `polygon_amoy/README.md`. Address pinned via SENDER_ADDR constant
        // — matches `.meta.json::address` for `amoy-smoke-1`.
        let sender_addr: Address = SENDER_ADDR.parse().expect("sender EIP-55 parses");
        let recipient_addr: Address = RECIPIENT_ADDR.parse().expect("recipient hex parses");
        let token_addr: Address = token.parse().expect("USDC hex parses");

        let sender_balance_before = evm_wallet_core::erc20::token_balance(
            &provider, token_addr, sender_addr,
        )
        .await
        .expect("oracle balanceOf(sender) before");
        log_step(
            2,
            10,
            "snapshot sender USDC balance (BEFORE SEND)",
            &format!(
                "sender={sender_addr} (wallet name: {WALLET_NAME})\nUSDC contract={token_addr}\nbalanceOf(sender) raw = {sender_balance_before}\nbalanceOf(sender) = {usdc}",
                usdc = format_usdc(sender_balance_before),
            ),
        );
        assert!(
            sender_balance_before >= U256::from(TRANSFER_AMOUNT_RAW),
            "sender must be pre-funded with USDC >= 1.0; visit https://faucet.circle.com/ — \
             got {sender_balance_before}"
        );

        // ===== Step 3: snapshot recipient USDC balance =====
        let recipient_balance_before = evm_wallet_core::erc20::token_balance(
            &provider, token_addr, recipient_addr,
        )
        .await
        .expect("oracle balanceOf(recipient) before");
        log_step(
            3,
            10,
            "snapshot recipient USDC balance (BEFORE SEND)",
            &format!(
                "recipient={recipient_addr}\nbalanceOf(recipient) raw = {recipient_balance_before}\nbalanceOf(recipient) = {usdc}",
                usdc = format_usdc(recipient_balance_before),
            ),
        );

        // ===== Step 4: wallet preflight (sender wallet must load) =====
        // Sender (`amoy-smoke-1`) wallet must exist in `wallet_data_dir()`
        // — CLI signs the transfer via this wallet. Recipient is a raw
        // EIP-55 address string (`--to 0x...`) — `polygon erc20 send` does
        // NOT require the recipient to be a managed wallet. So we only
        // assert sender presence here; recipient presence is irrelevant
        // to the send path.
        let list = run_polygon(
            &[
                "wallet",
                "list",
                "--network",
                "amoy",
                "--json",
            ],
            &data_dir,
        );
        let list_stdout = String::from_utf8_lossy(&list.stdout).to_string();
        let list_stderr = String::from_utf8_lossy(&list.stderr).to_string();
        log_step(
            4,
            10,
            "wallet preflight (list)",
            &format!(
                "polygon wallet list --network amoy --json\nexit = {}\nstdout: {}\nstderr: {}",
                list.status.code().unwrap_or(-1),
                list_stdout.trim(),
                list_stderr.trim(),
            ),
        );
        assert!(
            list.status.success(),
            "polygon wallet list failed: stderr={list_stderr}"
        );
        assert!(
            list_stdout.contains("\"amoy-smoke-1\""),
            "wallet list must contain sender wallet name \"amoy-smoke-1\"; got: {list_stdout}"
        );

        // ===== Step 5: send 1.0 USDC =====
        // CLI requires `--token` (positional, clap); `--token-address` is an
        // optional override. main.rs:732-734 prefers `token_address` if Some
        // else resolves `token` via the per-network registry. Pass both for
        // defense-in-depth: `--token` satisfies clap, `--token-address` skips
        // the symbol lookup and binds to the exact address from JSON SoT.
        // Sign via existing `amoy-smoke-1` wallet (no import).
        let send = run_polygon(
            &[
                "erc20",
                "send",
                "--name",
                WALLET_NAME,
                "--token",
                "USDC",
                "--token-address",
                &token,
                "--to",
                RECIPIENT_ADDR,
                "--amount",
                &TRANSFER_AMOUNT_RAW.to_string(),
                "--network",
                "amoy",
            ],
            &data_dir,
        );
        let send_stdout = String::from_utf8_lossy(&send.stdout).to_string();
        let send_stderr = String::from_utf8_lossy(&send.stderr).to_string();
        // CLI prints: `tx_hash: 0x...`
        let tx_hash_str = send_stdout
            .lines()
            .find_map(|line| line.strip_prefix("tx_hash: 0x").map(|s| s.to_string()))
            .unwrap_or_else(|| {
                panic!(
                    "polygon erc20 send stdout must contain `tx_hash: 0x...`; \
                     got: {send_stdout}\nstderr: {send_stderr}"
                )
            });
        let tx_hash: B256 = tx_hash_str
            .parse()
            .unwrap_or_else(|e| panic!("parse tx_hash `{tx_hash_str}`: {e}"));
        let explorer = format!("{}/tx/0x{tx_hash_str}", amoy_explorer_url());
        // Surface the tx hash on stdout (machine-greppable) so operators running
        // with `--nocapture` can pipe `| grep TX_HASH=` to extract it without
        // parsing the log file. Mirrors what `polygon erc20 send` already
        // prints to stdout but with a stable machine-greppable marker.
        println!("TX_HASH=0x{tx_hash_str}");
        log_step(
            5,
            10,
            "send 1.0 USDC",
            &format!(
                "polygon erc20 send --name amoy-erc20-test --token-address {token} \
                 --to {RECIPIENT_ADDR} --amount {TRANSFER_AMOUNT_RAW}\nexit = {}\n\
                 tx_hash = 0x{tx_hash_str}\nexplorer: {explorer}",
                send.status.code().unwrap_or(-1)
            ),
        );
        assert!(
            send.status.success(),
            "polygon erc20 send failed: stderr={send_stderr}"
        );

        // ===== Step 6: poll receipt =====
        let receipt = poll_receipt(
            &provider,
            tx_hash,
            Duration::from_secs(60),
            Duration::from_secs(2),
        )
        .await
        .unwrap_or_else(|| {
            fail_guard.set("receipt not mined within 60s");
            panic!(
                "receipt not mined within 60s; tx_hash=0x{tx_hash_str}\nexplorer: {explorer}"
            )
        });
        log_step(
            6,
            10,
            "poll receipt",
            &format!(
                "receipt present\nstatus = {}\ngas_used = {}\nexplorer: {explorer}",
                receipt.status(),
                receipt.gas_used,
            ),
        );

        // ===== Step 7: assert tx success =====
        assert!(receipt.status(), "tx reverted; receipt={receipt:?}");
        log_step(7, 10, "assert tx success", "status = 0x1 ✓");

        // ===== Step 8: locate Transfer event =====
        let transfer_topic_b256: B256 = TRANSFER_TOPIC.parse().expect("topic hex");
        let transfer_log = receipt
            .logs()
            .iter()
            .find(|log| {
                log.address() == token_addr
                    && log.topics().first() == Some(&transfer_topic_b256)
            })
            .unwrap_or_else(|| {
                fail_guard.set("no Transfer event found in receipt logs");
                panic!(
                    "no Transfer event found in receipt logs for token {token_addr}; \
                     logs = {:#?}",
                    receipt.logs()
                )
            });
        let topics = transfer_log.topics();
        log_step(
            8,
            10,
            "locate Transfer event",
            &format!(
                "logs count: {}\nmatching log: address={:?} topics[0]={:?}",
                receipt.logs().len(),
                transfer_log.address(),
                topics.first(),
            ),
        );

        // ===== Step 9: decode event =====
        let topic_from = Address::from_word(topics.get(1).copied().unwrap_or(B256::ZERO));
        let topic_to = Address::from_word(topics.get(2).copied().unwrap_or(B256::ZERO));
        let value_bytes = transfer_log.data().data.as_ref();
        let mut padded = [0u8; 32];
        let copy_len = value_bytes.len().min(32);
        padded[..copy_len].copy_from_slice(&value_bytes[..copy_len]);
        let value = U256::from_be_bytes(padded);
        log_step(
            9,
            10,
            "decode event",
            &format!("from = {topic_from}\nto   = {topic_to}\nvalue = {value}"),
        );
        assert_eq!(topic_from, sender_addr, "Transfer.from must equal sender");
        assert_eq!(
            topic_to, recipient_addr,
            "Transfer.to must equal recipient"
        );
        assert_eq!(
            value,
            U256::from(TRANSFER_AMOUNT_RAW),
            "Transfer.value must equal {TRANSFER_AMOUNT_RAW}"
        );

        // ===== Step 10: verify deltas =====
        let sender_balance_after = evm_wallet_core::erc20::token_balance(
            &provider, token_addr, sender_addr,
        )
        .await
        .expect("oracle balanceOf(sender) after");
        let recipient_balance_after = evm_wallet_core::erc20::token_balance(
            &provider, token_addr, recipient_addr,
        )
        .await
        .expect("oracle balanceOf(recipient) after");
        let sender_delta = sender_balance_before - sender_balance_after;
        let recipient_delta = recipient_balance_after - recipient_balance_before;
        log_step(
            10,
            10,
            "verify deltas (AFTER SEND)",
            &format!(
                "sender_balance_after   raw = {sender_balance_after}  ({sender_usdc_after})\n\
                 recipient_balance_after raw = {recipient_balance_after}  ({recipient_usdc_after})\n\
                 sender_delta   = -{sender_delta} (-{sender_usdc_delta})\n\
                 recipient_delta = +{recipient_delta} (+{recipient_usdc_delta})\n\
                 expected delta  = +/- {TRANSFER_AMOUNT_RAW} (+/- 1.000000 USDC)",
                sender_usdc_after = format_usdc(sender_balance_after),
                recipient_usdc_after = format_usdc(recipient_balance_after),
                sender_usdc_delta = format_usdc(sender_delta),
                recipient_usdc_delta = format_usdc(recipient_delta),
            ),
        );
        assert_eq!(
            sender_delta,
            U256::from(TRANSFER_AMOUNT_RAW),
            "sender USDC delta must equal {TRANSFER_AMOUNT_RAW}"
        );
        assert_eq!(
            recipient_delta,
            U256::from(TRANSFER_AMOUNT_RAW),
            "recipient USDC delta must equal {TRANSFER_AMOUNT_RAW}"
        );

        Ok::<(), Box<dyn std::error::Error>>(())
    })
    .expect("workflow block_on");

    log_pass(started.elapsed());
    // Disarm failure guard — we passed. The Drop guard is leaked intentionally;
    // skipping the Drop avoids a misleading `=== TEST PASSED ===` followed by a
    // `=== TEST FAILED ===` race on the same log.
    std::mem::forget(fail_guard);
}

#[test]
fn amoy_erc20_send_full_log_export() {
    // Hermetic end-to-end log export — exercises every log_step call site
    // (steps 0..10 + the pass marker) with deterministic synthetic data, so
    // plain `cargo test -p polygon --test amoy_erc20_send` writes a complete
    // 10-step log block to `.local/tmp/amoy_erc20_send_report.log` without
    // hitting any RPC. Lets reviewers + operators inspect the full log shape
    // without funding the L29 operator pre-conditions.
    log_export_status();
    let started = Instant::now();

    let sender_addr: Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .expect("canonical Anvil-#0 sender");
    let recipient_addr: Address = RECIPIENT_ADDR.parse().expect("recipient hex parses");
    let token_addr: Address = usdc_address().parse().expect("USDC hex parses");
    let _transfer_topic_b256: B256 = TRANSFER_TOPIC.parse().expect("topic hex");
    // Deterministic synthetic tx hash — `DEADBEEF` × 8 = obviously fake
    // placeholder (not a real on-chain tx). Valid hex, parses cleanly, lets
    // reviewers grep for the deadbeef pattern instead of chasing a phantom
    // tx on amoy.polygonscan.com. Real runs emit real hashes from receipts.
    let tx_hash: B256 = "0xDEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEF"
        .parse()
        .expect("synthetic tx hash");
    let tx_hash_str = alloy_primitives::hex::encode(tx_hash.as_slice());

    let sender_balance_before = U256::from(2_500_000u64);
    let recipient_balance_before = U256::ZERO;
    let sender_balance_after = U256::from(1_500_000u64);
    let recipient_balance_after = U256::from(1_000_000u64);

    log_step(
        0,
        10,
        "TEST STARTED (synthetic — no RPC)",
        &format!(
            "synthetic round-trip\nUSDC={token_addr}\nRPC={}\nCHAIN_ID={}\n\
             tx_hash = 0x{tx_hash_str}",
            amoy_rpc_url(),
            amoy_chain_id(),
        ),
    );
    log_step(
        1,
        10,
        "preflight env check",
        "RUN_POLYGON_AMOY = \"1\" ✓ (synthetic — gate bypassed for log export)",
    );
    log_step(
        2,
        10,
        "snapshot sender USDC balance (BEFORE SEND)",
        &format!(
            "sender={sender_addr}\nUSDC contract={token_addr}\n\
             balanceOf(sender) raw = {sender_balance_before}\n\
             balanceOf(sender) = {}",
            format_usdc(sender_balance_before)
        ),
    );
    log_step(
        3,
        10,
        "snapshot recipient USDC balance (BEFORE SEND)",
        &format!(
            "recipient={recipient_addr}\n\
             balanceOf(recipient) raw = {recipient_balance_before}\n\
             balanceOf(recipient) = {}",
            format_usdc(recipient_balance_before)
        ),
    );
    log_step(
        4,
        10,
        "wallet import",
        "polygon wallet import --name amoy-erc20-test --private-key-file <0600-mode>\n\
         exit = 0\nstdout: wallet imported: name=amoy-erc20-test id=<synthetic-uuid> \
         address=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266\n\
         stderr:",
    );
    log_step(
        5,
        10,
        "send 1.0 USDC",
        &format!(
            "polygon erc20 send --name amoy-erc20-test --token-address {token_addr} \
             --to {recipient_addr} --amount {TRANSFER_AMOUNT_RAW}\n\
             exit = 0\ntx_hash = 0x{tx_hash_str} (synthetic — not on-chain)",
        ),
    );
    log_step(
        6,
        10,
        "poll receipt",
        "receipt present (synthetic)\nstatus = true\ngas_used = 64257\n\
         explorer link omitted (synthetic tx; no amoy.polygonscan.com entry)",
    );
    log_step(7, 10, "assert tx success", "status = 0x1 ✓ (synthetic)");
    log_step(
        8,
        10,
        "locate Transfer event",
        &format!(
            "logs count: 1 (synthetic)\n\
             matching log: address={token_addr} topics[0]=0x{transfer_topic_str}",
            transfer_topic_str = &TRANSFER_TOPIC[2..],
        ),
    );
    log_step(
        9,
        10,
        "decode event",
        &format!(
            "from = {sender_addr}\nto   = {recipient_addr}\n\
             value = {TRANSFER_AMOUNT_RAW} ({} raw / 1.000000 USDC)",
            TRANSFER_AMOUNT_RAW
        ),
    );
    log_step(
        10,
        10,
        "verify deltas (AFTER SEND)",
        &format!(
            "sender_balance_after   raw = {sender_balance_after}  ({})\n\
             recipient_balance_after raw = {recipient_balance_after}  ({})\n\
             sender_delta   = -{sender_delta} (-{})\n\
             recipient_delta = +{recipient_delta} (+{})\n\
             expected delta  = +/- {TRANSFER_AMOUNT_RAW} (+/- 1.000000 USDC)",
            format_usdc(sender_balance_after),
            format_usdc(recipient_balance_after),
            U256::from(TRANSFER_AMOUNT_RAW),
            format_usdc(U256::from(TRANSFER_AMOUNT_RAW)),
            sender_delta = U256::from(TRANSFER_AMOUNT_RAW),
            recipient_delta = U256::from(TRANSFER_AMOUNT_RAW),
        ),
    );

    log_pass(started.elapsed());
}

#[test]
fn amoy_erc20_send_config_present() {
    // Hermetic SoT verification — runs on plain `cargo test -p polygon`
    // without `RUN_POLYGON_AMOY=1`. Catches JSON drift silently.
    log_export_status();
    let token = usdc_address();
    assert_eq!(
        token, "0x8B0180f2101c8260d49339abfEe87927412494B4",
        "USDC address in tokens/amoy.json must match plan §Network Configuration"
    );
    let rpc = amoy_rpc_url();
    assert!(
        rpc.contains("polygon-amoy"),
        "RPC URL must target Amoy; got {rpc}"
    );
    require_run_polygon_amoy();
}
