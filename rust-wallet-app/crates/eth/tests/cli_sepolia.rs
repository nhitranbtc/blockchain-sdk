//! Binary integration tests for the `eth` CLI against live Sepolia testnet.
//!
//! Per L29 (lessons.md): live-testnet smoke is operator-driven, not CI.
//! Each test is `#[ignore]`-marked + opt-in via `SEPOLIA_E2E=1`. CI never
//! runs these; the operator runs them manually with real creds + funded
//! wallets. Complements `cli_localnet.rs` (Anvil-fork suite, deterministic,
//! CI-per-commit) by exercising the same code path against a live RPC + a
//! real ERC-20 contract on Sepolia — different risk surface (gas estimation,
//! block inclusion timing, ERC-20 contract quirks, RPC latency).
//!
//! Required env (operator sets these before run):
//! - `SEPOLIA_E2E=1` — gate (mandatory)
//! - `SEPOLIA_RPC_URL` — e.g. `https://sepolia.infura.io/v3/<KEY>`
//! - `SEPOLIA_USDT_ADDRESS` — deployed OpenZeppelin ERC-20 mock address
//!   (6 decimals, see Issue #352 "Setup" item 2). NOT in
//!   `tokens/sepolia.json` because that file ships Circle's official USDC
//!   only; this mock is operator-specific per Sepolia deploy.
//!
//! Test convention: `async fn` + `#[tokio::test]` (matches alloy-provider).
//! Sync wallet ops use `#[test]` per #333 exemption. Wallet store isolated
//! via fresh `TempDir` so no state bleeds across tests.
//!
//! Operator-driven acceptance gate: PR-merge box flips `[ ]`→`[x]` only
//! after the operator confirms a successful Sepolia run (L13 step 14
//! "external gate acceptance").

use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

// `Provider` trait — provides `get_balance` (L-1 pre-state ETH check) +
// `get_transaction_receipt` (M-2 post-broadcast wait for mined tx).
// Without this import, calls like `provider.get_balance(addr)` fail with
// E0599 (method exists but trait not in scope).
use alloy_provider::Provider;

alloy_sol_macro::sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    contract IERC20 {
        function balanceOf(address target) returns (uint256);
    }
}

/// Skip helper for Sepolia-gated tests (L29). Requires three env vars —
/// `SEPOLIA_E2E=1`, `SEPOLIA_RPC_URL`, and `SEPOLIA_USDT_ADDRESS`. All
/// three MUST be set; missing any one logs and returns early (test
/// compiles + passes as ignored, never hits the network).
macro_rules! sepolia_or_skip {
    () => {
        if std::env::var("SEPOLIA_E2E").ok().as_deref() != Some("1") {
            eprintln!("[cli_sepolia] SKIP — set SEPOLIA_E2E=1 to run");
            return;
        }
        if std::env::var("SEPOLIA_RPC_URL").is_err() {
            eprintln!("[cli_sepolia] SKIP — set SEPOLIA_RPC_URL to run");
            return;
        }
        if std::env::var("SEPOLIA_USDT_ADDRESS").is_err() {
            eprintln!("[cli_sepolia] SKIP — set SEPOLIA_USDT_ADDRESS to run");
            return;
        }
    };
}

/// USDT has 6 decimals (matches Tether mainnet USDT + Circle USDC). 100
/// USDT = 100 × 10^6 raw units. Operator's mock MUST also use 6 decimals
/// (per #352 spec).
const USDT_TRANSFER_AMOUNT_RAW: u64 = 100_000_000;

/// Circle USDC on Sepolia — official testnet deployment. 6 decimals
/// (same as USDT). Faucet: https://faucet.circle.com. No contract
/// deploy required (Tether does NOT deploy official testnet USDT, so
/// USDC is the path of least resistance for L29 acceptance runs).
const USDC_SEPOLIA: alloy_primitives::Address =
    alloy_primitives::address!("1c7D4B196Cb0F7BB1D82a98fE3bfD0BfE4aEb287");

/// USDC also 6 decimals. 100 USDC = 100 × 10^6 raw units.
const USDC_TRANSFER_AMOUNT_RAW: u64 = 100_000_000;

/// Resolve the path to the `eth` binary under test. Cargo provides this
/// via `CARGO_BIN_EXE_<name>` for integration tests (same as
/// `cli_localnet.rs`).
fn eth_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_eth"))
}

/// Run the `eth` binary with `ETH_DATA_DIR` pointed at `data_dir` (so the
/// wallet store is isolated). Captures stdout + stderr + exit status.
///
/// Strips inherited `ETH_PASSWORD` so tests stay hermetic. Tests that
/// exercise the env-var code path must call `Command::new` directly and
/// set `ETH_PASSWORD` explicitly.
///
/// IMPORTANT: `SEPOLIA_RPC_URL` contains an operator API key. The CLI
/// must NOT echo it to stderr (cycle 7 redaction per
/// `redact_rpc_error` in `handlers.rs`). If a test fails and the stderr
/// dump includes the URL, that's a redaction regression — file an
/// issue.
fn run_eth(data_dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(eth_bin())
        .env("ETH_DATA_DIR", data_dir)
        .env("NO_COLOR", "1")
        .env_remove("ETH_PASSWORD")
        .args(args)
        .output()
        .expect("spawn eth")
}

// ---------------------------------------------------------------------------
// Sepolia-gated live-testnet test (L29 opt-in)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "L29 operator smoke — set SEPOLIA_E2E=1 + live creds to run"]
async fn alpha_send_beta_100_usdt_against_sepolia() {
    // Issue #352 acceptance test: alpha sends beta 100 USDT against live
    // Sepolia testnet (real RPC + real ERC-20 mock contract).
    //
    // Flow:
    // 1. Operator pre-funds alpha's address with Sepolia ETH (gas) +
    //    100+ USDT from the mock contract. Alpha's address is
    //    deterministic (derived from Anvil's well-known "test test ...
    //    junk" mnemonic at index 0 →
    //    0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266). Operator does
    //    this BEFORE running the test — see #352 "Setup".
    // 2. Test creates temp ETH_DATA_DIR, imports alpha (deterministic
    //    mnemonic), creates beta (random mnemonic, address captured
    //    from CLI stdout).
    // 3. Test asserts pre-state: alpha USDT balance >= 100 USDT (sanity
    //    check that operator-funded correctly).
    // 4. Test runs `eth erc20 send --name alpha --password test
    //    --token $SEPOLIA_USDT_ADDRESS --to beta_address --amount 100
    //    --network sepolia --rpc-url $SEPOLIA_RPC_URL`.
    // 5. Test asserts exit 0 + tx hash in stdout.
    // 6. Test queries post-state: beta's USDT balance == 100 USDT.
    // 7. TempDir drops → wallet files cleaned up.
    sepolia_or_skip!();

    let rpc_url = std::env::var("SEPOLIA_RPC_URL").expect("SEPOLIA_RPC_URL");
    let usdt_address_str = std::env::var("SEPOLIA_USDT_ADDRESS").expect("SEPOLIA_USDT_ADDRESS");
    let usdt_address: alloy_primitives::Address = usdt_address_str
        .parse()
        .expect("SEPOLIA_USDT_ADDRESS parse");

    let parsed_url = alloy_transport_http::reqwest::Url::parse(&rpc_url).expect("rpc url");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(parsed_url);

    let alpha_addr: alloy_primitives::Address =
        alloy_primitives::address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

    // Pre-state sanity (L-1 from PR review): alpha must have >= 100 USDT
    // AND >= 0.01 Sepolia ETH (gas). Both pre-funds are operator's
    // responsibility — fail fast with clear messages if either missing.
    let usdt = IERC20::new(usdt_address, &provider);
    let alpha_usdt_before = usdt
        .balanceOf(alpha_addr)
        .call()
        .await
        .expect("alpha balanceOf pre-state (USDT)");
    assert!(
        alpha_usdt_before >= alloy_primitives::U256::from(USDT_TRANSFER_AMOUNT_RAW),
        "pre-fund missing: alpha has {alpha_usdt_before} raw USDT, need >= {USDT_TRANSFER_AMOUNT_RAW}. \
         Operator must pre-fund alpha with at least 100 USDT from the mock contract.",
    );
    let alpha_eth_before = provider
        .get_balance(alpha_addr)
        .await
        .expect("alpha get_balance pre-state (Sepolia ETH for gas)");
    let min_gas_eth: alloy_primitives::U256 =
        alloy_primitives::U256::from(10_000_000_000_000_000u128); // 0.01 ETH
    assert!(
        alpha_eth_before >= min_gas_eth,
        "gas pre-fund missing: alpha has {alpha_eth_before} wei Sepolia ETH, need >= {min_gas_eth} wei (= 0.01 ETH). \
         Operator must pre-fund alpha with Sepolia ETH via https://cloudflare-eth.com/faucet or https://sepoliafaucet.com.",
    );

    // Import alpha wallet (deterministic mnemonic — same as Anvil #0).
    // Assert exit 0 (M-1 from PR review): same pattern as the L12
    // `wallet create beta` check above. A silent alpha-import failure
    // would otherwise surface at `erc20 send` as a confusing "wallet
    // not found" instead of pinpointing the import step.
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();
    let phrase = "test test test test test test test test test test test junk";
    let alpha_import = run_eth(
        &data_dir,
        &[
            "wallet",
            "import",
            "--name",
            "alpha",
            "--mnemonic",
            phrase,
            "--password",
            "test-password",
            "--network",
            "sepolia",
        ],
    );
    let alpha_import_stdout = String::from_utf8_lossy(&alpha_import.stdout);
    let alpha_import_stderr = String::from_utf8_lossy(&alpha_import.stderr);
    assert_eq!(
        alpha_import.status.code(),
        Some(0),
        "wallet import alpha must succeed\nstdout: {alpha_import_stdout}\nstderr: {alpha_import_stderr}",
    );

    // Create beta wallet (random mnemonic). Capture address from stdout.
    // Assert exit 0 first — if `wallet create` fails, the stdout parse
    // would silently return None and surface as an unrelated "beta addr
    // parse" panic. Explicit success check gives the failure a clear
    // pointer to the wallet-create path (M-2 from L12 review).
    let beta_create = run_eth(
        &data_dir,
        &[
            "wallet",
            "create",
            "--name",
            "beta",
            "--password",
            "test-password",
            "--network",
            "sepolia",
        ],
    );
    let beta_stdout = String::from_utf8_lossy(&beta_create.stdout);
    let beta_stderr = String::from_utf8_lossy(&beta_create.stderr);
    assert_eq!(
        beta_create.status.code(),
        Some(0),
        "wallet create beta must succeed\nstdout: {beta_stdout}\nstderr: {beta_stderr}",
    );
    let beta_addr_str = beta_stdout
        .lines()
        .find_map(|line| {
            // wallet_create prints `address:    0x<40 hex>` per
            // `print_wallet_created` in handlers.rs. Parse the line
            // starting with `address:` and extract the hex part.
            // Brittle: if the format changes, the assertion above
            // catches the empty stdout, but the find_map returns None
            // and panics with "beta wallet stdout must contain
            // `address:` line" — that message is the regression
            // signal.
            line.strip_prefix("address:").map(|s| s.trim().to_string())
        })
        .expect("beta wallet stdout must contain `address:` line");
    let beta_addr: alloy_primitives::Address = beta_addr_str.parse().expect("beta addr parse");

    // Run the CLI's erc20 send path: alpha -> beta, 100 USDT raw units.
    let out = run_eth(
        &data_dir,
        &[
            "erc20",
            "send",
            "--name",
            "alpha",
            "--password",
            "test-password",
            "--token",
            &usdt_address_str,
            "--to",
            &beta_addr_str,
            "--amount",
            &USDT_TRANSFER_AMOUNT_RAW.to_string(),
            "--network",
            "sepolia",
            "--rpc-url",
            &rpc_url,
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "erc20 send must succeed against Sepolia\nstdout: {stdout}\nstderr: {stderr}",
    );
    let hex_count = stdout
        .chars()
        .filter(|c| c.is_ascii_hexdigit() && *c != '\n')
        .count();
    assert!(
        stdout.contains("0x") && hex_count >= 64,
        "expected tx hash (0x + 64 hex chars) in stdout: {stdout}",
    );

    // M-2 from PR review: wait for the tx to be mined before asserting
    // beta's balance. send_raw_transaction returns once the tx is in
    // the mempool; the on-chain `balanceOf` won't reflect it until the
    // tx is mined (Sepolia block time ~12s, plus mempool queue). Without
    // this wait, the balance assertion fails with "beta has 0, expected
    // 100000000" — a confusing message that masks the actual cause
    // (tx still in mempool). Poll get_transaction_receipt up to 60s.
    let tx_hash_hex = stdout
        .lines()
        .find_map(|line| {
            // The CLI's `wallet_send_erc20` prints `pending.tx_hash()`
            // (handlers.rs:470) which serialises as `0x<64 hex>`. The
            // tx hash is the only 0x+64-hex string on stdout, but be
            // robust: find any line containing exactly 66 hex chars
            // starting with 0x.
            if line.starts_with("0x") && line.len() == 66 {
                Some(line.to_string())
            } else {
                None
            }
        })
        .expect("stdout must contain tx hash (0x + 64 hex chars)");
    let tx_hash: alloy_primitives::B256 = tx_hash_hex.parse().expect("tx hash parse");
    // Print the tx hash to stdout so the operator can copy it into
    // Etherscan Sepolia (https://sepolia.etherscan.io/tx/<hash>) without
    // digging through the test runner's buffered stderr.
    println!("[cli_sepolia] tx_hash: {tx_hash_hex}");
    let receipt = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        provider.get_transaction_receipt(tx_hash),
    )
    .await
    .expect("timed out waiting for tx receipt after 60s")
    .expect("get_transaction_receipt error")
    .expect("tx not mined within 60s — Sepolia mempool stuck or RPC unreliable");
    assert!(
        receipt.status(),
        "tx mined but reverted on-chain — check mock contract mint/transfer semantics",
    );

    // Post-state assert: beta's USDT balance == exactly 100 USDT. With
    // the receipt wait above, the tx is guaranteed mined; `balanceOf`
    // reflects post-state.
    let beta_balance_after = usdt
        .balanceOf(beta_addr)
        .call()
        .await
        .expect("beta balanceOf post-state");
    assert_eq!(
        beta_balance_after,
        alloy_primitives::U256::from(USDT_TRANSFER_AMOUNT_RAW),
        "beta should have exactly {USDT_TRANSFER_AMOUNT_RAW} raw USDT (= 100 USDT) after receiving from alpha, got {beta_balance_after}",
    );

    // Cleanup: TempDir drops here, removing ETH_DATA_DIR + all wallets.
    // No explicit `wallet delete` needed — the directory is gone.
}

// ---------------------------------------------------------------------------
// USDC variant — same alpha→beta 100 transfer pattern, but uses Circle's
// official Sepolia USDC (0x1c7D4B...Eb287, 6 decimals) instead of an
// operator-deployed USDT mock. Faucet: https://faucet.circle.com.
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "L29 operator smoke — set SEPOLIA_E2E=1 + live creds + SEPOLIA_USDC_ADDRESS to run"]
async fn alpha_send_beta_100_usdc_against_sepolia() {
    // Issue #352 acceptance test, USDC variant. Same flow as USDT test
    // above, but token = Circle's official Sepolia USDC (already
    // deployed at USDC_SEPOLIA, no contract deploy needed). Operator
    // pre-funds alpha with Sepolia ETH (gas) + 100 USDC via
    // https://faucet.circle.com BEFORE running.
    //
    // Required env (in addition to the 3 in `sepolia_or_skip!()`):
    //   SEPOLIA_USDC_ADDRESS — Circle USDC on Sepolia. Hardcoded as
    //     USDC_SEPOLIA below; the env var is checked for symmetry with
    //     the USDT fn + lets operator override if Circle ever redeploys.
    sepolia_or_skip!();
    if std::env::var("SEPOLIA_USDC_ADDRESS").is_err() {
        eprintln!("[cli_sepolia] SKIP — set SEPOLIA_USDC_ADDRESS to run");
        return;
    }
    let usdc_address_str = std::env::var("SEPOLIA_USDC_ADDRESS").expect("SEPOLIA_USDC_ADDRESS");
    let usdc_address_from_env: alloy_primitives::Address = usdc_address_str
        .parse()
        .expect("SEPOLIA_USDC_ADDRESS parse");
    // Prefer the hardcoded constant (Circle's official address); only
    // fall back to env var if it differs (allows override if Circle
    // ever redeploys USDC on Sepolia).
    let usdc_address = if usdc_address_from_env == USDC_SEPOLIA {
        USDC_SEPOLIA
    } else {
        usdc_address_from_env
    };

    let rpc_url = std::env::var("SEPOLIA_RPC_URL").expect("SEPOLIA_RPC_URL");
    let parsed_url = alloy_transport_http::reqwest::Url::parse(&rpc_url).expect("rpc url");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(parsed_url);

    let alpha_addr: alloy_primitives::Address =
        alloy_primitives::address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

    // Pre-state: alpha must have >= 100 USDC AND >= 0.01 Sepolia ETH.
    let usdc = IERC20::new(usdc_address, &provider);
    let alpha_usdc_before = usdc
        .balanceOf(alpha_addr)
        .call()
        .await
        .expect("alpha balanceOf pre-state (USDC)");
    assert!(
        alpha_usdc_before >= alloy_primitives::U256::from(USDC_TRANSFER_AMOUNT_RAW),
        "pre-fund missing: alpha has {alpha_usdc_before} raw USDC, need >= {USDC_TRANSFER_AMOUNT_RAW}. \
         Faucet alpha via https://faucet.circle.com (Circle USDC on Sepolia).",
    );
    let alpha_eth_before = provider
        .get_balance(alpha_addr)
        .await
        .expect("alpha get_balance pre-state (Sepolia ETH for gas)");
    let min_gas_eth: alloy_primitives::U256 =
        alloy_primitives::U256::from(10_000_000_000_000_000u128); // 0.01 ETH
    assert!(
        alpha_eth_before >= min_gas_eth,
        "gas pre-fund missing: alpha has {alpha_eth_before} wei Sepolia ETH, need >= {min_gas_eth} wei (= 0.01 ETH). \
         Operator must pre-fund alpha with Sepolia ETH via https://cloudflare-eth.com/faucet or https://sepoliafaucet.com.",
    );

    // Import alpha wallet (deterministic mnemonic — same as Anvil #0).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();
    let phrase = "test test test test test test test test test test test junk";
    let alpha_import = run_eth(
        &data_dir,
        &[
            "wallet",
            "import",
            "--name",
            "alpha",
            "--mnemonic",
            phrase,
            "--password",
            "test-password",
            "--network",
            "sepolia",
        ],
    );
    let alpha_import_stdout = String::from_utf8_lossy(&alpha_import.stdout);
    let alpha_import_stderr = String::from_utf8_lossy(&alpha_import.stderr);
    assert_eq!(
        alpha_import.status.code(),
        Some(0),
        "wallet import alpha must succeed\nstdout: {alpha_import_stdout}\nstderr: {alpha_import_stderr}",
    );

    // Create beta wallet (random mnemonic). Capture address from stdout.
    let beta_create = run_eth(
        &data_dir,
        &[
            "wallet",
            "create",
            "--name",
            "beta",
            "--password",
            "test-password",
            "--network",
            "sepolia",
        ],
    );
    let beta_stdout = String::from_utf8_lossy(&beta_create.stdout);
    let beta_stderr = String::from_utf8_lossy(&beta_create.stderr);
    assert_eq!(
        beta_create.status.code(),
        Some(0),
        "wallet create beta must succeed\nstdout: {beta_stdout}\nstderr: {beta_stderr}",
    );
    let beta_addr_str = beta_stdout
        .lines()
        .find_map(|line| line.strip_prefix("address:").map(|s| s.trim().to_string()))
        .expect("beta wallet stdout must contain `address:` line");
    let beta_addr: alloy_primitives::Address = beta_addr_str.parse().expect("beta addr parse");

    // Run the CLI's erc20 send path: alpha -> beta, 100 USDC raw units.
    let out = run_eth(
        &data_dir,
        &[
            "erc20",
            "send",
            "--name",
            "alpha",
            "--password",
            "test-password",
            "--token",
            &usdc_address_str,
            "--to",
            &beta_addr_str,
            "--amount",
            &USDC_TRANSFER_AMOUNT_RAW.to_string(),
            "--network",
            "sepolia",
            "--rpc-url",
            &rpc_url,
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "erc20 send must succeed against Sepolia\nstdout: {stdout}\nstderr: {stderr}",
    );
    let hex_count = stdout
        .chars()
        .filter(|c| c.is_ascii_hexdigit() && *c != '\n')
        .count();
    assert!(
        stdout.contains("0x") && hex_count >= 64,
        "expected tx hash (0x + 64 hex chars) in stdout: {stdout}",
    );

    let tx_hash_hex = stdout
        .lines()
        .find_map(|line| {
            if line.starts_with("0x") && line.len() == 66 {
                Some(line.to_string())
            } else {
                None
            }
        })
        .expect("stdout must contain tx hash (0x + 64 hex chars)");
    let tx_hash: alloy_primitives::B256 = tx_hash_hex.parse().expect("tx hash parse");
    println!("[cli_sepolia] tx_hash: {tx_hash_hex}");
    let receipt = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        provider.get_transaction_receipt(tx_hash),
    )
    .await
    .expect("timed out waiting for tx receipt after 60s")
    .expect("get_transaction_receipt error")
    .expect("tx not mined within 60s — Sepolia mempool stuck or RPC unreliable");
    assert!(
        receipt.status(),
        "tx mined but reverted on-chain — check USDC contract semantics",
    );

    // Post-state: beta's USDC balance == exactly 100 USDC.
    let beta_balance_after = usdc
        .balanceOf(beta_addr)
        .call()
        .await
        .expect("beta balanceOf post-state");
    assert_eq!(
        beta_balance_after,
        alloy_primitives::U256::from(USDC_TRANSFER_AMOUNT_RAW),
        "beta should have exactly {USDC_TRANSFER_AMOUNT_RAW} raw USDC (= 100 USDC) after receiving from alpha, got {beta_balance_after}",
    );

    // Cleanup: TempDir drops here.
}
