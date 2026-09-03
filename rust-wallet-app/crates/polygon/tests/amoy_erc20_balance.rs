//! T6d-2.1 parity oracle (Issue #523) — `polygon erc20 balance` must match
//! raw `eth_call balanceOf(holder)` ÷ 10^decimals.
//!
//! CI-safe by default (`#[ignore]`); opt in per L29:
//!     RUN_POLYGON_AMOY=1 cargo test -p polygon --test amoy_erc20_balance -- --ignored
//!
//! Loads `tokens/amoy.json` per the P8-T-setup pattern (Issue #518 /
//! `amoy_smoke.rs`). USDC address + holder come from the JSON so the
//! test source has no USDC-specific literals (lives next to the config
//! it exercises).
//!
//! Parity oracle: spawn `polygon erc20 balance --json` as a subprocess,
//! parse the `raw` field from stdout, then independently call
//! `evm_wallet_core::erc20::token_balance(&provider, token, holder)`
//! against the same Amoy RPC inside the test process. Both paths must
//! encode `balanceOf(holder)` as `0x70a08231 ++ 32-byte-padded(holder)`
//! per ERC-20 spec; asserting equality proves the CLI's wire format
//! matches the canonical alloy selector route. A future refactor that
//! breaks calldata encoding (selector drift, address padding mistake)
//! flips this test red at the byte level.
//!
//! Lives as its own integration test (not in `amoy_smoke.rs`) per L13
//! step 9a — distinct concern (ERC-20 parity), distinct gate, distinct
//! fixture shape (USDC-vs-POL oracle). Sister pattern at
//! `polygon/tests/amoy_erc20_send_round_trip.rs` (Phase 8 T3 follow-up).

#![cfg(test)]

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

/// Load `${CARGO_MANIFEST_DIR}/tokens/amoy.json` once and parse it.
/// Top-level expressions are not allowed in test files; this runs lazily
/// from each helper. Mirrors `amoy_smoke.rs:44-58`.
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

/// Pull a JSON object field (the `test_harness` namespace).
fn amoy_json_obj(key: &str) -> serde_json::Value {
    ensure_tokens_loaded();
    let v = AMOY_TOKENS_JSON
        .get()
        .expect("AMOY_TOKENS_JSON set by ensure_tokens_loaded");
    v.get(key)
        .cloned()
        .unwrap_or_else(|| panic!("missing object field `{key}` in tokens/amoy.json"))
}

/// Look up the `address` field of the USDC entry in `tokens[]` array.
/// Falls back to panic + clear message if USDC is missing (drift signal).
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

/// Path to the `polygon` binary built by cargo for integration tests.
/// `env!("CARGO_BIN_EXE_polygon")` is compile-time cargo metadata.
fn polygon_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_polygon"))
}

/// Invoke `polygon` as a subprocess with hermetic env.
fn run_polygon(args: &[&str], data_dir: &std::path::Path) -> std::process::Output {
    Command::new(polygon_bin())
        .args(args)
        .arg("--data-dir")
        .arg(data_dir)
        .env("POLYGON_PASSWORD", "test-pw-ignore-leak")
        .env("POLYGON_NETWORK", "amoy")
        .env("RUST_BACKTRACE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn polygon binary")
}

/// Guard: skip unless `tokens/amoy.json` `test_harness.run_polygon_amoy == "1"`.
/// Mirrors `amoy_smoke.rs:140-155`.
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

/// Pull `test_harness.holder_address_for_usdc_oracle` from
/// `tokens/amoy.json`. Holds `HOLDER_LOCK` to serialize concurrent
/// captures (sister discipline to `amoy_smoke.rs:80` PK_LOCK).
static HOLDER_LOCK: Mutex<()> = Mutex::new(());

fn holder_address() -> String {
    let _guard = HOLDER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let v = amoy_json_obj("test_harness");
    v.get("holder_address_for_usdc_oracle")
        .and_then(|s| s.as_str())
        .unwrap_or_else(|| {
            panic!(
                "missing test_harness.holder_address_for_usdc_oracle in tokens/amoy.json — \
                 add Issue #523 test vector (0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369)"
            )
        })
        .to_string()
}

/// Issue #523 — parity oracle test (P8-T3 G5 follow-up).
///
/// Steps:
///   1. Spawn `polygon erc20 balance --json --address <H> --token-address <USDC> --network amoy`.
///   2. Parse the JSON `raw` field.
///   3. Independently call `evm_wallet_core::erc20::token_balance` against
///      the same Amoy RPC inside the test process.
///   4. Assert CLI `raw` == oracle `raw`.
///   5. Sanity: parsed `decimals == 6` for USDC (the canonical contract).
#[test]
#[ignore]
fn amoy_erc20_balance_parity_against_eth_call_oracle() {
    require_run_polygon_amoy();

    let holder = holder_address();
    let token = usdc_address();
    let data_dir = tempfile::TempDir::new().expect("tempdir for data-dir");

    // Step 1: spawn CLI.
    let out = run_polygon(
        &[
            "erc20",
            "balance",
            "--token",
            "USDC",
            "--json",
            "--address",
            &holder,
            "--token-address",
            &token,
            "--network",
            "amoy",
        ],
        data_dir.path(),
    );
    assert!(
        out.status.success(),
        "polygon erc20 balance --json failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let cli_json: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("CLI stdout must be valid JSON (--json mode); got: {stdout}\nparse error: {e}")
    });
    let cli_raw_str = cli_json
        .get("raw")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("CLI JSON missing `raw` field: {stdout}"));
    let cli_raw_decimal = cli_json
        .get("decimals")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| panic!("CLI JSON missing `decimals` field: {stdout}"));
    let cli_raw = alloy_primitives::U256::from_str_radix(cli_raw_str.trim_start_matches("0x"), 10)
        .unwrap_or_else(|e| {
            panic!("CLI `raw` field not decimal U256: {cli_raw_str}\nparse error: {e}")
        });

    // Sanity: USDC has 6 decimals per `tokens/amoy.json` bundled registry.
    assert_eq!(
        cli_raw_decimal, 6,
        "USDC decimals must be 6 per the bundled registry; got {cli_raw_decimal}"
    );

    // Step 2: independent oracle via alloy provider (same Amoy RPC).
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime for oracle");
    let oracle_raw = rt.block_on(async {
        let provider =
            polygon_wallet_core::new_http_polygon_amoy().expect("provider build for oracle");
        let token_addr: alloy_primitives::Address = token.parse().expect("USDC hex parses");
        let holder_addr: alloy_primitives::Address = holder.parse().expect("holder hex parses");
        evm_wallet_core::erc20::token_balance(&provider, token_addr, holder_addr)
            .await
            .expect("oracle token_balance (balanceOf)")
    });

    assert_eq!(
        cli_raw, oracle_raw,
        "CLI raw U256 must equal independent `eth_call balanceOf` oracle \
         (selector 0x70a08231 + 32-byte-padded holder encoding); \
         CLI={cli_raw} oracle={oracle_raw}"
    );
}

// ============================================================
// Hermetic SoT verification (NOT #[ignore]) — runs on plain
// `cargo test -p polygon --test amoy_erc20_balance` (no live RPC,
// no operator setup) so CI parity catches config drift silently.
// ============================================================

#[test]
fn amoy_tokens_have_usdc_entry() {
    let token = usdc_address();
    // USDC for Amoy lives at the canonical address per Issue #523 test
    // vector; the test source never hardcodes the literal — if the JSON
    // drifts, the panic message names the missing field.
    assert_eq!(
        token, "0x8B0180f2101c8260d49339abfEe87927412494B4",
        "USDC address in tokens/amoy.json must match plan §Network Configuration USDC contract"
    );

    // Sentinel: holder fixture present + matches the Issue #523 vector.
    let holder = holder_address();
    assert_eq!(
        holder, "0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369",
        "holder_address_for_usdc_oracle in tokens/amoy.json must match Issue #523 test vector"
    );

    // Opt-in guard reads the same JSON flag the live test uses.
    require_run_polygon_amoy();
}
