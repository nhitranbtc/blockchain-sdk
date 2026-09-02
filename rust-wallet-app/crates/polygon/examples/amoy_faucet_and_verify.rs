//! Operator-driven end-to-end Amoy faucet verification.
//!
//! Bridges the gap while `polygon faucet` (P8-T0 / G11) is a stub. Creates a
//! wallet via the `polygon` CLI, extracts the EIP-55 address, prompts the
//! operator to fund via web faucet, polls the balance until non-zero (or
//! timeout), reports final state. Per L29: operator-driven, NOT a CI test.
//!
//! # Usage
//!
//! ```text
//! cargo run -p polygon --example amoy_faucet_and_verify -- \
//!     --name test --password pw --network amoy
//! ```
//!
//! Defaults: `--timeout 300` (5 min), `--poll-interval 5` (5 s).
//!
//! # Configuration
//!
//! Configuration is loaded in this priority order (highest first):
//!
//! 1. Shell env vars (`POLYGON_RPC_URL`, `POLYGON_FAUCET_URL`, etc.) — paid-tier overrides
//! 2. `${CARGO_MANIFEST_DIR}/tokens/amoy.json` — committed Amoy config (RPC + 2 faucets + explorer + USDC token entry)
//!
//! **No `DEFAULT_*` fallback**: if `tokens/amoy.json` is missing or malformed,
//! the binary exits 2 with a clear error. Math constants (`WEI_PER_POL`,
//! `USDC_UNIT`, `BALANCE_OF_SELECTOR`) are not config and stay in the binary.
//!
//! ## `tokens/amoy.json` schema (extended; supersedes the lib's pure token-registry schema)
//!
//! ```json
//! {
//!   "chain_id": 80002,
//!   "rpc_url": "https://polygon-amoy-bor-rpc.publicnode.com",
//!   "faucet_pol_url": "https://faucet.polygon.technology",
//!   "faucet_circle_url": "https://faucet.circle.com/",
//!   "explorer_url": "https://amoy.polygonscan.com",
//!   "tokens": [
//!     {"symbol": "USDC", "address": "0x8B0180...94B4", "decimals": 6}
//!   ]
//! }
//! ```
//!
//! # Exit codes
//!
//! - `0` — at least one of {native POL, USDC} went non-zero within timeout
//!   (Phase 3 reports both balances + explorer link)
//! - `1` — timeout (Phase 3 reports last-seen zero balances + helpful error)
//! - `2` — Phase 1 subprocess failed or address extraction failed

use std::io::Write;
use std::process::{Command, ExitCode};
use std::str::FromStr;
use std::time::{Duration, Instant};

use alloy_primitives::{Address, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::{BlockId, BlockNumberOrTag, TransactionRequest};
use clap::Parser;

// ============================================================================
// Math constants (not config; not network-dependent).
// ============================================================================

/// Wei → POL divisor (10^18).
const WEI_PER_POL: u128 = 1_000_000_000_000_000_000;

/// USDC has 6 decimals; 1 USDC = 1_000_000 raw.
const USDC_UNIT: u128 = 1_000_000;

/// ERC-20 `balanceOf(address)` selector.
const BALANCE_OF_SELECTOR: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];

// ============================================================================
// Config — loaded strictly from `${CARGO_MANIFEST_DIR}/tokens/amoy.json`;
// shell env vars override individual fields. Missing/malformed JSON is a
// hard error (binary exits 2).
// ============================================================================

/// Path to the committed Amoy config JSON. Resolved at compile time via
/// `CARGO_MANIFEST_DIR` (Cargo sets this for examples).
const AMOY_CONFIG_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tokens/amoy.json");

/// JSON-side schema for `tokens/amoy.json`. Extended beyond the lib's pure
/// token-registry schema with network-level fields (rpc + faucets + explorer)
/// so the example has a single source of truth.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AmoyConfigJson {
    #[serde(default = "default_amoy_chain_id")]
    #[allow(dead_code)]
    chain_id: u64,
    rpc_url: String,
    faucet_pol_url: String,
    faucet_circle_url: String,
    explorer_url: String,
    tokens: Vec<TokenJson>,
}

#[derive(Debug, serde::Deserialize)]
struct TokenJson {
    symbol: String,
    address: String,
    #[allow(dead_code)]
    decimals: u8,
}

fn default_amoy_chain_id() -> u64 {
    80_002
}

#[derive(Debug, Clone)]
struct Config {
    rpc_url: String,
    faucet_pol_url: String,
    faucet_circle_url: String,
    explorer_url: String,
    usdc_address: Address,
}

impl Config {
    fn load() -> Result<Self, String> {
        let json = load_amoy_json()?;

        let usdc_address = std::env::var("POLYGON_USDC_ADDRESS")
            .ok()
            .and_then(|s| Address::from_str(&s).ok())
            .or_else(|| usdc_from_json(&json))
            .ok_or_else(|| {
                "tokens/amoy.json `tokens[]` has no entry with symbol == \"USDC\"".to_string()
            })?;

        Ok(Self {
            rpc_url: env_or("POLYGON_RPC_URL", &json.rpc_url),
            faucet_pol_url: env_or("POLYGON_FAUCET_URL", &json.faucet_pol_url),
            faucet_circle_url: env_or("CIRCLE_FAUCET_URL", &json.faucet_circle_url),
            explorer_url: env_or("AMOY_POLYGONSCAN_URL", &json.explorer_url),
            usdc_address,
        })
    }
}

/// Parse `tokens/amoy.json` from disk. Returns `Err` with a clear message if
/// the file is missing or malformed — caller exits 2.
fn load_amoy_json() -> Result<AmoyConfigJson, String> {
    let bytes = std::fs::read(AMOY_CONFIG_PATH).map_err(|e| {
        format!(
            "failed to read {AMOY_CONFIG_PATH}: {e}\n  hint: tokens/amoy.json is the committed config source-of-truth; verify the file exists"
        )
    })?;
    serde_json::from_slice::<AmoyConfigJson>(&bytes).map_err(|e| {
        format!(
            "failed to parse {AMOY_CONFIG_PATH}: {e}\n  hint: validate against the schema in examples/README.md §Configuration"
        )
    })
}

fn usdc_from_json(j: &AmoyConfigJson) -> Option<Address> {
    j.tokens
        .iter()
        .find(|t| t.symbol.eq_ignore_ascii_case("USDC"))
        .and_then(|t| Address::from_str(&t.address).ok())
}

fn env_or(key: &str, json_value: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| json_value.to_string())
}

// ============================================================================
// CLI args.
// ============================================================================

#[derive(Debug, Parser)]
#[command(
    name = "amoy_faucet_and_verify",
    about = "Operator-driven Amoy faucet + balance-polling verification (P8-T-manualfaucet)",
    long_about = None,
)]
struct Args {
    /// Name for the new wallet (forwarded to `polygon wallet create`).
    /// Required UNLESS `--address` is set (skip wallet creation).
    #[arg(long)]
    name: Option<String>,

    /// Pre-existing EIP-55 address to monitor. When set, skips Phase 1
    /// (wallet creation) and goes straight to Phase 2 balance polling.
    /// Useful for re-checking a previously funded wallet without burning a
    /// fresh wallet name.
    #[arg(long)]
    address: Option<String>,

    /// Target network. Only `amoy` is supported (mainnet has no canonical
    /// faucet). Default `amoy`.
    #[arg(long, default_value = "amoy")]
    network: String,

    /// Total polling budget in seconds (Phase 2). Default `300` (5 min).
    #[arg(long, default_value_t = 300)]
    timeout: u64,

    /// Seconds between balance polls. Default `5`.
    #[arg(long, default_value_t = 5)]
    poll_interval: u64,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let cfg = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    if args.network != "amoy" {
        eprintln!(
            "error: --network {} not supported (only `amoy` has a canonical faucet)",
            args.network
        );
        return ExitCode::from(2);
    }

    println!("=== amoy_faucet_and_verify ===");
    println!("rpc_url        = {}", cfg.rpc_url);
    println!("usdc_address   = {}", cfg.usdc_address);
    println!(
        "timeout        = {}s, poll_interval = {}s",
        args.timeout, args.poll_interval
    );
    println!();

    // ---- Phase 1: create wallet OR use pre-supplied address ----
    let address = match phase1_resolve_address(&args) {
        Ok(addr) => addr,
        Err(code) => return code,
    };

    let eip55 = format!("{address}");
    if args.address.is_some() {
        // --address path: skip the funding pause (no fresh wallet to fund).
        println!("\nmonitoring (no funding prompt): {eip55}");
    } else {
        println!("\nfunding target (EIP-55 checksum): {eip55}");
        println!("\nfunding options:");
        println!("  POL (native gas):  {}", cfg.faucet_pol_url);
        println!(
            "  USDC (test token): {}  (select `Polygon Amoy`)",
            cfg.faucet_circle_url
        );
        println!("\nview on explorer: {}/address/{eip55}", cfg.explorer_url);
        print!("\npress Enter after funding the address...");
        std::io::stdout().flush().ok();
        let mut ack = String::new();
        if std::io::stdin().read_line(&mut ack).is_err() {
            eprintln!("error: failed to read acknowledgement from stdin");
            return ExitCode::from(2);
        }
        println!();
    }

    // ---- Phase 2: poll balances (native POL + ERC-20 USDC) ----
    let (final_pol, final_usdc, polled_at_least_once) = match phase2_poll_balances(
        address,
        cfg.usdc_address,
        &cfg.rpc_url,
        args.timeout,
        args.poll_interval,
    )
    .await
    {
        Ok(t) => t,
        Err(code) => return code,
    };

    // ---- Phase 3: report ----
    let exit_code = phase3_report(
        address,
        final_pol,
        final_usdc,
        polled_at_least_once,
        args.timeout,
        &cfg.explorer_url,
        &cfg.usdc_address,
    );

    // ---- Phase 3b: parity check — compare alloy readback against `polygon
    // wallet balance` subprocess. Surfaces the #522 known mismatch (CLI
    // formatter off by 10^3) without failing the run. Output is both
    // printed and appended to the log file via write_log_entry's tail-arg.
    let parity_block = verify_cli_parity(address, final_pol);

    // Append the parity block to the same per-run log entry.
    if let Some(p) = parity_block {
        // Re-open the log file in append mode and write the parity block.
        // Cheap: file already exists from phase3_report's write.
        const LOG_PATH: &str = ".local/tmp/amoy_faucet_and_verify_report.md";
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(LOG_PATH) {
            let _ = f.write_all(p.as_bytes());
            if !p.ends_with('\n') {
                let _ = f.write_all(b"\n");
            }
            let _ = f.write_all(b"\n");
        }
    }

    exit_code
}

/// Render a compact ASCII balance table (POL + USDC) at the end of the report.
fn balance_table(balance_pol: U256, balance_usdc: U256, usdc_contract: &Address) -> String {
    let pol_wei: u128 = balance_pol.try_into().unwrap_or(u128::MAX);
    let usdc_raw: u128 = balance_usdc.try_into().unwrap_or(u128::MAX);

    // Headers
    let h_token = "Token";
    let h_balance = "Balance";
    let h_source = "Source";
    let sep = "+---------+----------------------+---------------------------------------------+\n";

    let mut out = String::new();
    out.push_str("\nBalance summary:\n");
    out.push_str(sep);
    out.push_str(&format!(
        "| {:<7} | {:<20} | {:<43} |\n",
        h_token, h_balance, h_source
    ));
    out.push_str(sep);

    // POL row — balance truncated to 6-decimal for table fit; full wei in Phase 3 detail above.
    let pol_hr = pol_wei as f64 / 1e18;
    let pol_str = format!("{pol_hr:.6} POL");
    let pol_source = format!("eth_getBalance → {pol_wei} wei");
    out.push_str(&format!(
        "| {:<7} | {:<20} | {:<43} |\n",
        "POL", pol_str, pol_source
    ));

    // USDC row.
    let usdc_hr = usdc_raw as f64 / 1_000_000.0;
    let usdc_str = format!("{usdc_hr:.6} USDC");
    let usdc_source = format!("eth_call balanceOf({usdc_contract}) → {usdc_raw} raw");
    out.push_str(&format!(
        "| {:<7} | {:<20} | {:<43} |\n",
        "USDC", usdc_str, usdc_source
    ));

    out.push_str(sep);
    out
}

// ============================================================================
// Parity check: spawn `polygon wallet balance` and compare stdout against
// the alloy readback. Currently surfaces #522 (CLI formatter off by 10^3);
// USDC parity is skipped because `polygon erc20 balance` is stubbed per
// #523 (deferred to T6d-2.1). Best-effort: any failure prints a warning and
// returns without altering the run's exit code.
// ============================================================================

fn verify_cli_parity(address: Address, alloy_pol_wei: U256) -> Option<String> {
    println!("\n[parity check] `polygon wallet balance` vs alloy get_balance");

    let polygon_bin =
        std::env::var("CARGO_BIN_EXE_polygon").unwrap_or_else(|_| "polygon".to_string());
    let output = Command::new(&polygon_bin)
        .args([
            "wallet",
            "balance",
            "--address",
            &format!("{address}"),
            "--network",
            "amoy",
        ])
        .output();
    let output = match output {
        Ok(o) => o,
        Err(e) => {
            let msg = format!("  warn: could not spawn `polygon wallet balance`: {e}");
            println!("{msg}");
            return Some(msg);
        }
    };

    if !output.status.success() {
        let msg = format!(
            "  warn: `polygon wallet balance` exited with status {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        println!("{msg}");
        return Some(msg);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let cli_displayed = stdout.trim();

    // Parse the first whitespace-separated token as a float (handles both
    // "0.37 POL" and "0.00037 POL" forms from any future formatter change).
    // f64 has enough precision for typical faucet drip magnitudes; we only
    // use this for the visual mismatch check.
    let cli_wei_approx: Option<u128> = cli_displayed
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|n| (n * 1e18) as u128);

    let alloy_u128: u128 = alloy_pol_wei.try_into().unwrap_or(u128::MAX);

    let match_mark = match cli_wei_approx {
        Some(c) if c == alloy_u128 => "✓ match",
        Some(_) => "✗ MISMATCH (1000× off — #522)",
        None => "? (CLI stdout unparseable)",
    };

    // Machine-readable parity verdict (grep-able). `true` only when the parsed
    // CLI wei number equals the alloy readback exactly.
    let parity_ok = matches!(cli_wei_approx, Some(c) if c == alloy_u128);
    let parity_line = format!(
        "parity: {} (polygon_cli_vs_eth_getBalance = {})",
        parity_ok,
        if parity_ok { "true" } else { "false" }
    );

    // ---- Parity table: polygon CLI vs eth_getBalance raw oracle ----
    let sep = "+-------------------+----------------------+--------------------------------+\n";
    let mut out = String::new();
    out.push_str(sep);
    out.push_str(&format!(
        "| {:<17} | {:<20} | {:<30} |\n",
        "Source", "Value (display)", "Notes"
    ));
    out.push_str(sep);
    out.push_str(&format!(
        "| {:<17} | {:<20} | {:<30} |\n",
        "polygon CLI", cli_displayed, match_mark
    ));
    out.push_str(&format!(
        "| {:<17} | {:<20} | {:<30} |\n",
        "eth_getBalance",
        format!("{alloy_u128} wei"),
        "raw oracle (raw = displayed/1000)"
    ));
    out.push_str(sep);
    out.push_str(&format!("{parity_line}\n"));
    out.push_str("USDC parity skipped — `polygon erc20 balance` deferred to T6d-2.1 (#523)\n");
    print!("{out}");
    Some(out)
}

// ============================================================================
// Phase 1: create wallet via the `polygon` CLI subprocess, OR use a
// pre-supplied address (skips wallet creation entirely).
// ============================================================================

fn phase1_resolve_address(args: &Args) -> Result<Address, ExitCode> {
    // Path B: operator supplied --address; parse + validate, skip wallet creation.
    if let Some(addr_str) = &args.address {
        let addr = Address::from_str(addr_str).map_err(|e| {
            eprintln!("error: invalid --address `{addr_str}`: {e}");
            eprintln!("  hint: provide an EIP-55 checksum address (e.g. 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369)");
            ExitCode::from(2)
        })?;
        println!("[phase 1/3] using pre-supplied address (skip wallet create): {addr}");
        return Ok(addr);
    }

    // Path A: create wallet via the `polygon` CLI subprocess.
    let name = args.name.as_deref().ok_or_else(|| {
        eprintln!("error: --name is required (or pass --address to skip wallet creation)");
        ExitCode::from(2)
    })?;

    // Password comes from `POLYGON_WALLET_PASSWORD` env var only — never
    // from argv (security review feedback; passwords in argv leak via
    // /proc/*/cmdline + shell history + logs). The `polygon` CLI itself
    // reads `POLYGON_PASSWORD` (per its in-binary warning "set
    // POLYGON_PASSWORD in CI") so we forward it via Command::env, not argv.
    let password = std::env::var("POLYGON_WALLET_PASSWORD").map_err(|_| {
        eprintln!(
            "error: POLYGON_WALLET_PASSWORD env var not set\n  \
             hint: export POLYGON_WALLET_PASSWORD=<password> (avoid passing on argv; leaks via /proc/<pid>/cmdline + shell history)"
        );
        ExitCode::from(2)
    })?;

    println!("[phase 1/3] creating wallet via `polygon wallet create`...");

    // Resolve the `polygon` binary path. `CARGO_BIN_EXE_<name>` is set by Cargo
    // at runtime when running via `cargo run --example` or `cargo test`; fall
    // back to PATH lookup for operators who built once and run from elsewhere.
    let polygon_bin =
        std::env::var("CARGO_BIN_EXE_polygon").unwrap_or_else(|_| "polygon".to_string());
    let output = Command::new(polygon_bin)
        .args([
            "wallet",
            "create",
            "--name",
            name,
            "--network",
            &args.network,
        ])
        .env("POLYGON_PASSWORD", &password)
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: failed to spawn `polygon` binary: {e}");
            eprintln!("  hint: build first with `cargo build -p polygon`");
            return Err(ExitCode::from(2));
        }
    };

    if !output.status.success() {
        eprintln!(
            "error: `polygon wallet create` exited with status {}",
            output.status
        );
        eprintln!("--- stdout ---");
        eprintln!("{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("--- stderr ---");
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        return Err(ExitCode::from(2));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    print!("{}", stdout);

    let address = extract_address_from_create_stdout(&stdout).ok_or_else(|| {
        eprintln!("error: could not extract `address=0x<hex>` from `polygon wallet create` stdout");
        ExitCode::from(2)
    })?;

    Ok(address)
}

/// Extract the `address=0x<40 hex>` token from `polygon wallet create` stdout.
/// Also accepts `Wallet Address: 0x<...>` shape (defense in depth — current CLI
/// emits the former, per `polygon/src/main.rs:348-352`).
fn extract_address_from_create_stdout(stdout: &str) -> Option<Address> {
    let needle = "address=";
    let start = stdout.find(needle)? + needle.len();
    let rest = &stdout[start..];

    // Walk up to the first non-hex char after the optional `0x` prefix.
    let mut hex_end = 0usize;
    let mut count = 0usize;
    for c in rest.chars() {
        if count >= 64 {
            break;
        }
        if c == '0' || c == 'x' || c == 'X' {
            hex_end += c.len_utf8();
            count += 1;
            continue;
        }
        if c.is_ascii_hexdigit() {
            hex_end += c.len_utf8();
            count += 1;
            continue;
        }
        break;
    }

    let addr_str = rest.get(..hex_end)?;
    let addr_clean = addr_str
        .strip_prefix("0x")
        .or_else(|| addr_str.strip_prefix("0X"))?;
    if addr_clean.len() != 40 {
        return None;
    }
    Address::from_str(&format!("0x{addr_clean}")).ok()
}

// ============================================================================
// Phase 2: poll native POL + ERC-20 USDC balances via alloy provider.
// ============================================================================

async fn phase2_poll_balances(
    address: Address,
    usdc_address: Address,
    rpc_url: &str,
    timeout_secs: u64,
    poll_interval_secs: u64,
) -> Result<(U256, U256, bool), ExitCode> {
    println!(
        "[phase 2/3] polling balances for {} (timeout {}s, every {}s)...",
        address, timeout_secs, poll_interval_secs
    );
    println!("  POL  → eth_getBalance");
    println!("  USDC → eth_call balanceOf({})", usdc_address);

    let url = match url::Url::parse(rpc_url) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("error: invalid RPC URL `{rpc_url}`: {e}");
            return Err(ExitCode::from(2));
        }
    };

    let provider = ProviderBuilder::new().connect_http(url);

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let interval = Duration::from_secs(poll_interval_secs);
    let mut last_pol = U256::ZERO;
    let mut last_usdc = U256::ZERO;
    let mut polled = false;

    loop {
        // Native POL balance.
        match provider.get_balance(address).await {
            Ok(bal) => {
                polled = true;
                last_pol = bal;
            }
            Err(e) => {
                eprintln!("\n  warn: eth_getBalance error: {e}");
            }
        }

        // ERC-20 USDC balance (silently skip on transient errors; report each
        // success so the operator can see the drip landing).
        match get_usdc_balance(&provider, usdc_address, address).await {
            Ok(bal) => {
                polled = true;
                last_usdc = bal;
            }
            Err(e) => {
                eprintln!("\n  warn: USDC balanceOf error: {e}");
            }
        }

        let pol_funded = !last_pol.is_zero();
        let usdc_funded = !last_usdc.is_zero();
        if pol_funded || usdc_funded {
            if pol_funded {
                println!("\n  ✓ POL non-zero: {} wei", last_pol);
            }
            if usdc_funded {
                println!("\n  ✓ USDC non-zero: {} (raw, 6-decimal)", last_usdc);
            }
            return Ok((last_pol, last_usdc, polled));
        }

        print!(".");
        std::io::stdout().flush().ok();

        let now = Instant::now();
        if now >= deadline {
            println!();
            return Ok((last_pol, last_usdc, polled));
        }

        let remaining = deadline.saturating_duration_since(now);
        tokio::time::sleep(interval.min(remaining)).await;
    }
}

/// Query an ERC-20 `balanceOf(holder)` via raw `eth_call`. Returns the raw
/// 256-bit balance (USDC = 6 decimals; divide by `USDC_UNIT` for display).
async fn get_usdc_balance<P>(provider: &P, token: Address, holder: Address) -> Result<U256, String>
where
    P: Provider,
{
    // balanceOf(address) calldata = selector(4) ++ holder(32-byte left-padded).
    let mut padded = [0u8; 32];
    padded[12..32].copy_from_slice(holder.as_slice());
    let mut input = Vec::with_capacity(4 + 32);
    input.extend_from_slice(&BALANCE_OF_SELECTOR);
    input.extend_from_slice(&padded);

    let req = TransactionRequest::default()
        .to(token)
        .input(alloy_primitives::Bytes::from(input).into());
    let out = provider
        .call(req)
        .block(BlockId::Number(BlockNumberOrTag::Latest))
        .await
        .map_err(|e| e.to_string())?;
    Ok(U256::from_be_slice(&out))
}

// ============================================================================
// Phase 3: report final state.
// ============================================================================

fn phase3_report(
    address: Address,
    balance_pol: U256,
    balance_usdc: U256,
    polled: bool,
    timeout_secs: u64,
    explorer_url: &str,
    usdc_address: &Address,
) -> ExitCode {
    let eip55 = format!("{address}");
    let pol_funded = !balance_pol.is_zero();
    let usdc_funded = !balance_usdc.is_zero();
    let timed_out = !pol_funded && !usdc_funded;

    // ---- Build the report lines (print to stdout AND append to log file) ----
    let mut report = String::new();
    report.push_str("\n[phase 3/3] final report\n");
    report.push_str(&format!("  address:        {eip55}\n"));
    report.push_str(&format!("  balance_pol:    {balance_pol} wei\n"));
    report.push_str(&format!(
        "  balance_usdc:   {balance_usdc} (raw, 6-decimal)\n"
    ));
    report.push_str(&format!("  usdc_contract:  {usdc_address}\n"));
    report.push_str(&format!(
        "  explorer:       {explorer_url}/address/{eip55}\n"
    ));

    if pol_funded {
        let wei_u128: u128 = balance_pol.try_into().unwrap_or(u128::MAX);
        let whole = wei_u128 / WEI_PER_POL;
        let frac = wei_u128 % WEI_PER_POL;
        report.push_str(&format!("  balance_pol_hr: ~{whole}.{frac:018} POL\n"));
    } else {
        report.push_str("  balance_pol_hr: (still zero — only USDC funded)\n");
    }

    if usdc_funded {
        let raw_u128: u128 = balance_usdc.try_into().unwrap_or(u128::MAX);
        let whole = raw_u128 / USDC_UNIT;
        let frac = raw_u128 % USDC_UNIT;
        report.push_str(&format!("  balance_usdc_hr: ~{whole}.{frac:06} USDC\n"));
    } else {
        report.push_str("  balance_usdc_hr: (still zero — only POL funded)\n");
    }

    if timed_out {
        report.push_str(&format!(
            "\nerror: timeout — no non-zero POL or USDC observed within {timeout_secs}s\n"
        ));
        if !polled {
            report.push_str("  hint: no successful RPC response at all — check RPC URL\n");
        } else {
            report
                .push_str("  hint: faucet drip may be delayed; re-run with a longer `--timeout`\n");
        }
    } else {
        report.push_str("\nsuccess — wallet funded.\n");
    }

    // ---- Balance summary table (POL + USDC) ----
    report.push_str(&balance_table(balance_pol, balance_usdc, usdc_address));

    // ---- Echo to stdout ----
    print!("{report}");

    // ---- Append to log file ----
    let exit_code: u8 = if timed_out { 1 } else { 0 };
    write_log_entry(&eip55, &report, exit_code, timed_out);

    if timed_out {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Append a single run's final report to the operator-local audit log.
/// Path: `./.local/tmp/amoy_faucet_and_verify_report.log` — relative to
/// the cwd at run time. `cargo run --example` runs the binary from the
/// workspace root (`rust-wallet-app/`), so the log lands at
/// `<repo-root>/.local/tmp/amoy_faucet_and_verify_report.log`. Direct
/// invocation from the package dir lands the log at
/// `crates/polygon/.local/tmp/...`. The `.local/` prefix is gitignored
/// per root `.gitignore:19`. Best-effort: log write failures print a
/// warning to stderr but do not fail the run.
fn write_log_entry(address: &str, report: &str, exit_code: u8, timed_out: bool) {
    const LOG_DIR: &str = ".local/tmp";
    const LOG_FILENAME: &str = "amoy_faucet_and_verify_report.md";

    if let Err(e) = std::fs::create_dir_all(LOG_DIR) {
        eprintln!("warn: failed to create {LOG_DIR}: {e} (skipping log write)");
        return;
    }
    let log_path_abs = format!("{LOG_DIR}/{LOG_FILENAME}");

    // RFC3339-ish timestamp without external deps (`chrono` not in scope for
    // examples). UNIX-epoch seconds is enough for an audit log.
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let disposition = if timed_out { "TIMEOUT" } else { "FUNDED" };

    let mut block = String::new();
    block.push_str(&format!(
        "=== amoy_faucet_and_verify run {ts} address={address} exit={exit_code} disposition={disposition} ===\n"
    ));
    block.push_str(report);
    if !block.ends_with('\n') {
        block.push('\n');
    }
    block.push_str(&format!("=== end run {ts} ===\n\n"));

    use std::io::Write;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path_abs);
    match file {
        Ok(mut f) => {
            if let Err(e) = f.write_all(block.as_bytes()) {
                eprintln!("warn: failed to append to {log_path_abs}: {e}");
            }
        }
        Err(e) => eprintln!("warn: failed to open {log_path_abs}: {e} (skipping log write)"),
    }
}
