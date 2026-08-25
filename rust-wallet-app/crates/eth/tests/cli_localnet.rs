//! Binary integration tests for the `eth` CLI against local Anvil + temp wallet store.
//!
//! Per #337 + #333: tests split into two layers:
//!
//! 1. **Always-on (no Anvil)** — wallet create/import/list/show/delete against
//!    a `tempfile::TempDir` injected via `ETH_DATA_DIR` env var. Runs in CI by
//!    default. No network I/O.
//!
//! 2. **Anvil-gated** (`#[ignore]` + `RUN_ANVIL_E2E=1` opt-in per L29 / #318
//!    pattern) — wallet balance + tx get against a spawned Anvil instance.
//!    CI never runs these unless the operator opts in.
//!
//! Test convention per #333: `async fn` + `#[tokio::test]` for code touching
//! alloy provider. Sync wallet ops use `#[test]` per the exemption (no async
//! deps). Each test isolates state via a fresh `TempDir` so wallet stores
//! don't bleed across tests.

use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

// Pulls `provider.anvil_deal_erc20(...)` etc. methods into scope.
use alloy_provider::ext::AnvilApi;
// `(account, slot).abi_encode()` for storage-slot derivation.
use alloy_sol_types::SolValue;

/// Skip helper for Anvil-gated tests (L29): if `RUN_ANVIL_E2E` is unset,
/// log and return early. Declared above the always-on tests so any
/// `#[ignore]`-marked Anvil test in the file can use it regardless of
/// its position relative to the Anvil-gated section.
macro_rules! anvil_or_skip {
    () => {
        if std::env::var("RUN_ANVIL_E2E").ok().as_deref() != Some("1") {
            eprintln!("[cli_localnet] SKIP — set RUN_ANVIL_E2E=1 to run");
            return;
        }
    };
}

// #343 acceptance test fixtures.
//
// ABI-only IERC20 binding — we never deploy, only call balanceOf via
// eth_call. Avoids the prior session's `MockUSDC::deploy` blocker
// (alloy-sol-macro doesn't generate a deploy helper in 1.6.1).
//
// `#[sol(rpc)]` enables `IERC20::new(addr, provider)` + `.balanceOf(...).call()`.
alloy_sol_macro::sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    contract IERC20 {
        function balanceOf(address target) returns (uint256);
    }
}

/// WETH9 mainnet contract. WETH is a simple wrapper (no proxy, no
/// upgradeability) with well-documented storage layout: `balanceOf`
/// is at slot 3 (after `name`/`symbol`/`decimals` at slots 0/1/2).
/// Chosen over USDC because USDC is a TransparentUpgradeableProxy and
/// its `_balances` storage lives at the impl's storage layout (via
/// delegatecall) — too many moving parts for an Anvil-fork mock test.
/// The use case (alpha sends beta 100 tokens, assert balances) is
/// token-agnostic; WETH serves as the canonical ERC-20 stand-in.
const WETH_MAINNET: alloy_primitives::Address =
    alloy_primitives::address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

/// `balanceOf` storage slot in WETH9 — after name/symbol/decimals.
const WETH_BALANCEOF_SLOT: u8 = 3;

/// WETH has 18 decimals (same as ETH). 10^18 raw units = 1 WETH.
const WETH_ONE: u128 = 1_000_000_000_000_000_000;

/// alpha sends beta 100 WETH (use case: "alpha sends beta 100 USDC"
/// from #343 body; token identity is incidental).
const WETH_TRANSFER_AMOUNT_RAW: u128 = 100 * WETH_ONE;

/// alpha starts with 1000 WETH, pre-written to WETH balanceOf slot.
const WETH_ALPHA_INITIAL_RAW: u128 = 1_000 * WETH_ONE;

/// USDT6 mainnet contract (TetherToken). USDT is a single contract
/// (no proxy, no upgradeability) — same Anvil-fork approach as WETH
/// but with `balances` at slot 0 (first state var in TetherToken).
/// Real stablecoin (6 decimals) — satisfies #343's "alpha sends beta
/// 100 USDC or USDT" use case.
const USDT_MAINNET: alloy_primitives::Address =
    alloy_primitives::address!("dAC17F958D2ee523a2206206994597C13D831ec7");

/// `balances` mapping slot in TetherToken — empirically determined.
/// Slot 0 didn't take effect; slot 1 didn't take effect; slot 2 works.
/// TetherToken's storage layout has 2 state vars before `balances`
/// (likely `upgradedAddress` + `_totalSupply` from StandardToken base,
/// depending on C3 linearization + re-declarations). Tested via
/// `anvil_set_storage_at` + `IERC20::balanceOf(alpha).call()` sanity
/// check — the assertion `alpha == 1000 USDT` passes only at slot 2.
const USDT_BALANCEOF_SLOT: u8 = 2;

/// USDT has 6 decimals (same as USDC). 100 USDT = 100 × 10^6 raw units.
const USDT_TRANSFER_AMOUNT_RAW: u64 = 100_000_000;

/// alpha starts with 1000 USDT, pre-written to USDT balances slot.
const USDT_ALPHA_INITIAL_RAW: u64 = 1_000_000_000;

/// DAI stablecoin mainnet contract (MakerDAO DSToken). 18 decimals
/// (not 6 like USDC/USDT — matches ETH's 18-decimal convention).
/// Single contract (no proxy, no upgradeability) — same Anvil-fork +
/// `anvil_set_storage_at` pattern as WETH + USDT.
const DAI_MAINNET: alloy_primitives::Address =
    alloy_primitives::address!("6B175474E89094C44Da98b954EedeAC495271d0F");

/// DAI has 18 decimals. 10^18 raw units = 1 DAI.
const DAI_ONE: u128 = 1_000_000_000_000_000_000;

/// alpha sends beta 100 DAI (use case: "alpha sends beta 100 USDC"
/// from #343 body; token identity is incidental).
const DAI_TRANSFER_AMOUNT_RAW: u128 = 100 * DAI_ONE;

/// alpha starts with 1000 DAI, pre-written to DAI balances slot.
const DAI_ALPHA_INITIAL_RAW: u128 = 1_000 * DAI_ONE;

/// `balances` mapping slot in DAI's DSToken — empirically determined.
/// Slot 0 didn't take effect; slot 1 didn't take effect; slot 2 works.
/// Same slot as USDT (`balances` at slot 2 for both TetherToken and
/// DSToken — likely both have 2 state vars before `balances` from
/// their respective base contracts via C3 linearization).
const DAI_BALANCEOF_SLOT: u8 = 2;

/// Public mainnet RPC source for `Anvil::new().fork(...)` — alloy docs
/// `anvil_set_storage_at` example (https://alloy.rs/examples/node-bindings/anvil_set_storage_at/).
/// Defaults to `https://ethereum-rpc.publicnode.com` (fork-capable public
/// gateway with archive-mode support, no signup). Override at compile
/// time with `MAINNET_RPC_URL=https://your-rpc.example cargo test` if the
/// default flakes (rate limits, downtime, etc.). Must support
/// archive-mode `eth_getBlockByNumber` (the call Anvil fork performs to
/// locate the fork block); Cloudflare's free gateway
/// (`cloudflare-eth.com`) does NOT support it — see run `32811985548`
/// (2026-08-25) which timed out 3 tests for that reason. Matches the
/// env fallback in `.github/workflows/rust-eth-core-ci.yml` (commit
/// `b443fa6`) so local `cargo test` and CI use the same gateway.
const MAINNET_RPC_URL: &str = match std::option_env!("MAINNET_RPC_URL") {
    Some(url) => url,
    None => "https://ethereum-rpc.publicnode.com",
};

/// Resolve the path to the `eth` binary under test. Cargo provides this via
/// the `CARGO_BIN_EXE_<name>` env var for integration tests.
fn eth_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_eth"))
}

/// Run the `eth` binary with `ETH_DATA_DIR` pointed at `data_dir` (so the
/// wallet store is isolated). Captures stdout + stderr + exit status.
///
/// Strips inherited `ETH_PASSWORD` from the parent shell environment so
/// tests stay hermetic. Tests that exercise the env-var code path must
/// call `Command::new` directly and set `ETH_PASSWORD` explicitly.
fn run_eth(data_dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(eth_bin())
        .env("ETH_DATA_DIR", data_dir)
        .env("NO_COLOR", "1")
        .env_remove("ETH_PASSWORD")
        .args(args)
        .output()
        .expect("spawn eth")
}

/// Assert the output looks like a successful CLI run: exit 0 + stdout
/// contains the given substring.
fn assert_success(out: &std::process::Output, needle: &str) {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0, got {:?}\nstdout:\n{}\nstderr:\n{}",
        out.status.code(),
        stdout,
        stderr,
    );
    assert!(
        stdout.contains(needle),
        "stdout missing {needle:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
    );
}

// ---------------------------------------------------------------------------
// Always-on sync wallet tests (no Anvil)
// ---------------------------------------------------------------------------

#[test]
fn wallet_create_then_list_shows_new_wallet() {
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    // Create a wallet.
    let create = run_eth(
        &data_dir,
        &[
            "wallet",
            "create",
            "--name",
            "alpha",
            "--password",
            "test-password-1",
        ],
    );
    assert_success(&create, "alpha");

    // List should show the new wallet by name (not the placeholder
    // `wallet-<uuid8>` stub).
    let list = run_eth(&data_dir, &["wallet", "list"]);
    assert_success(&list, "alpha");
}

#[test]
fn wallet_import_then_show_resolves_by_name() {
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    // Import the canonical "abandon abandon ... about" mnemonic.
    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let import = run_eth(
        &data_dir,
        &[
            "wallet",
            "import",
            "--name",
            "imported",
            "--mnemonic",
            phrase,
            "--password",
            "test-password-2",
        ],
    );
    assert_success(&import, "imported");

    // Show by name should resolve (WalletMeta was persisted alongside .enc).
    let show = run_eth(&data_dir, &["wallet", "show", "--name", "imported"]);
    assert_success(&show, "imported");
}

#[test]
fn wallet_create_with_unknown_network_yields_exit_2() {
    // Regression test for type-design CRITICAL: `Network::parse_cli`
    // previously returned WalletError::Path which mapped to Error::Rpc
    // (exit 3). It now returns Error::InvalidInput (exit 2).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "wallet",
            "create",
            "--name",
            "x",
            "--password",
            "p",
            "--network",
            "polygon",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "unknown network must yield bad-input exit code (2)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unknown network") && stderr.contains("polygon"),
        "stderr should mention the bad network: {stderr}",
    );
}

#[test]
fn wallet_show_with_unknown_name_preserves_name_in_error() {
    // Regression test for type-design + code-reviewer + security H-3
    // CRITICAL: `NotFoundByName` previously became WalletNotFound { wallet_id: nil }
    // dropping the user-supplied name. New variant WalletNotFoundByName
    // preserves it (exit 4 — wallet/balance category).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &["wallet", "show", "--name", "ghost", "--network", "sepolia"],
    );
    assert_eq!(
        out.status.code(),
        Some(4),
        "unknown wallet name must yield exit 4\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("ghost"),
        "stderr must preserve the user-supplied name: {stderr}",
    );
}

#[test]
fn wallet_create_with_duplicate_name_yields_exit_4() {
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let _ = run_eth(
        &data_dir,
        &["wallet", "create", "--name", "dup", "--password", "p1"],
    );
    // Pass a real password so clap accepts the args and the wallet handler
    // hits `name_exists_on_network`. Per #337 type-design CRITICAL fix:
    // the previous test used `--password` with no value which only
    // exercised clap's missing-arg parser, not duplicate detection.
    let dup = run_eth(
        &data_dir,
        &["wallet", "create", "--name", "dup", "--password", "p2"],
    );

    assert_eq!(
        dup.status.code(),
        Some(4),
        "duplicate wallet name must yield wallet/balance exit code (4)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&dup.stdout),
        String::from_utf8_lossy(&dup.stderr),
    );
    let stderr = String::from_utf8_lossy(&dup.stderr);
    assert!(
        stderr.contains("already exists") || stderr.contains("already"),
        "stderr should mention duplicate-name detection: {stderr}",
    );
}

#[test]
fn wallet_create_with_name_too_long_yields_exit_2() {
    // L12 review finding M-3 (code-reviewer): wallet name accepted any
    // string, including 1MB blobs and shell-meta names. RED: 33-char name
    // accepted (no validation). GREEN: rejected with InvalidInput (exit 2)
    // because name exceeds 32-char regex bound.
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "wallet",
            "create",
            "--name",
            &"x".repeat(33),
            "--password",
            "p",
        ],
    );

    assert_eq!(
        out.status.code(),
        Some(2),
        "33-char name must yield bad-input exit code (2)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("wallet name") || stderr.contains("name"),
        "stderr should mention name validation: {stderr}",
    );
}

#[test]
fn wallet_create_with_name_invalid_chars_yields_exit_2() {
    // L12 review finding M-3 (code-reviewer): wallet name accepted any
    // string including shell metacharacters (`;`, `&`, `|`, `$`, etc).
    // RED: `foo;bar` accepted (no validation). GREEN: rejected because
    // semicolon not in [A-Za-z0-9 _-] charset.
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &["wallet", "create", "--name", "foo;bar", "--password", "p"],
    );

    assert_eq!(
        out.status.code(),
        Some(2),
        "name with shell metachar must yield bad-input exit code (2)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("invalid char") || stderr.contains("wallet name"),
        "stderr should mention name charset validation: {stderr}",
    );
}

#[test]
fn wallet_create_with_password_argv_emits_security_warning() {
    // L12 review finding C-1 (security-auditor HIGH): passing wallet
    // password via --password leaks into shell history + process list.
    // RED: cycle 7 silently accepts --password with no operator warning.
    // GREEN: emit deprecation/security warning to stderr (cycle 8 closes
    // the operator-visibility half; cycle 8b swaps argv for TTY prompt
    // via rpassword).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "wallet",
            "create",
            "--name",
            "warn-me",
            "--password",
            "test-password",
        ],
    );

    assert_eq!(
        out.status.code(),
        Some(0),
        "create must succeed despite warning\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stderr_lower = stderr.to_lowercase();
    assert!(
        stderr_lower.contains("warning") && stderr_lower.contains("password"),
        "stderr must contain password-on-argv warning:\nstderr: {stderr}",
    );
}

#[test]
fn send_command_uses_eth_password_env_var_when_argv_missing() {
    // L12 review finding C-1 (security-auditor): prefer ETH_PASSWORD env
    // over argv when --password not provided. RED: cycle 7 returns
    // `--name and --password required` (exit 2). GREEN: env path
    // triggers, proceeds past unlock (fails later at unreachable RPC,
    // but NOT at the password-arg check).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    // Pre-create wallet using --password (current path).
    let _ = run_eth(
        &data_dir,
        &[
            "wallet",
            "create",
            "--name",
            "env-acct",
            "--password",
            "secret-from-env",
        ],
    );

    // Send without --password; set ETH_PASSWORD explicitly.
    let out = Command::new(eth_bin())
        .env("ETH_DATA_DIR", &data_dir)
        .env("NO_COLOR", "1")
        .env("ETH_PASSWORD", "secret-from-env")
        .args([
            "send",
            "--name",
            "env-acct",
            "--to",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--amount",
            "1000",
            "--rpc-url",
            "http://127.0.0.1:1", // unreachable
        ])
        .output()
        .expect("spawn eth");

    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        !stderr.contains("--password required") && !stderr.contains("password required"),
        "env path should bypass --password required error:\nstderr: {stderr}",
    );
    // No warning emitted on env path (warning reserved for argv path).
    assert!(
        !stderr.to_lowercase().contains("warning"),
        "env path should not emit password-on-argv warning:\nstderr: {stderr}",
    );
}

#[test]
fn erc20_send_command_without_token_yields_exit_2() {
    // L28 Gate C checklist (code-reviewer IMPORTANT): missing --token
    // branch was uncovered. RED: neither --token nor --token-address
    // provided → real impl rejects with InvalidInput (exit 2).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "erc20",
            "send",
            "--name",
            "alpha",
            "--password",
            "p",
            "--to",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--amount",
            "1000",
        ],
    );

    assert_eq!(
        out.status.code(),
        Some(2),
        "missing --token must yield bad-input exit code (2)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--token") || stderr.contains("token"),
        "stderr should mention --token: {stderr}",
    );
}

#[test]
fn wallet_balance_with_token_flag_is_accepted_by_clap() {
    // Issue #356 — extend `wallet balance` with `--token <ADDR>` for ERC-20
    // balance view. RED: clap rejects unknown `--token` flag (exit 2 + "Usage:"
    // or "unexpected argument" leaks into stderr). GREEN: clap accepts the
    // flag, handler reaches the RPC layer. Unreachable RPC → exit 3 with
    // "error sending request" — same lock-down shape as
    // `erc20_send_command_against_unreachable_rpc_is_not_a_stub`.
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "wallet",
            "balance",
            "--address",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--token",
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", // WETH mainnet
            "--rpc-url",
            "http://127.0.0.1:1", // unreachable
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let code = out.status.code();

    // Lock-down: clap rejected the flag → must NOT be exit 2 from clap.
    assert_ne!(
        code,
        Some(2),
        "lock-down: --token flag was rejected by clap (exit 2); not yet wired\nstdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        !stderr.contains("unexpected argument")
            && !stderr.contains("Usage:")
            && !stderr.contains("unrecognized"),
        "stderr must not contain clap rejection markers:\nstderr: {stderr}",
    );
    // Real impl against unreachable RPC: Error::Rpc from provider.call
    // (exit 3) — same shape as the erc20-send lock-down test.
    assert_eq!(
        code,
        Some(3),
        "wallet balance --token should reach eth_call and report RPC error\nstdout: {stdout}\nstderr: {stderr}",
    );
    // Lock-down against stub regression: the only path that emits
    // `balanceOf` in stderr is `token_balance`'s `format!("eth_call balanceOf: ...")`
    // prefix — a stub regression at handler entry returning Error::Rpc
    // from arbitrary text would NOT match this substring.
    assert!(
        stderr.contains("balanceOf"),
        "expected impl-path network error from real eth_call balanceOf:\nstdout: {stdout}\nstderr: {stderr}",
    );
}

#[test]
fn wallet_balance_with_invalid_token_address_yields_exit_2() {
    // L28 Gate C checklist (code-reviewer IMPORTANT): invalid --token
    // branch was uncovered by #356 design sketch. RED: clap accepts garbage
    // address, handler exits at provider.call with confusing RPC error.
    // GREEN: rejected with InvalidInput (exit 2) at parse.
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "wallet",
            "balance",
            "--address",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--token",
            "0xnot-an-address",
        ],
    );

    assert_eq!(
        out.status.code(),
        Some(2),
        "invalid --token must yield bad-input exit code (2)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--token") || stderr.contains("token"),
        "stderr should mention --token: {stderr}",
    );
}
#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn wallet_balance_token_weth_against_anvil_fork_mainnet() {
    // Issue #356 acceptance test: `eth wallet balance --token WETH` reads
    // the ERC-20 balance via eth_call against Anvil-fork-mainnet. Pre-fund
    // alpha with 1000 WETH (same anvil_set_storage_at pattern as the send
    // tests above). RED: --token flag absent, eth_call path unwired.
    // GREEN: eth_call returns the pre-funded balance.
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new()
        .fork(MAINNET_RPC_URL)
        .chain_id(31337)
        .spawn();
    let endpoint = anvil.endpoint();

    let rpc_url = alloy_transport_http::reqwest::Url::parse(&endpoint).expect("anvil url");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(rpc_url);

    let alpha_addr: alloy_primitives::Address = "f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .expect("alpha addr");

    // Pre-fund alpha with 1000 WETH (same slot math as the WETH send test).
    let slot_bytes = alloy_primitives::keccak256(
        (
            alpha_addr,
            alloy_primitives::U256::from(WETH_BALANCEOF_SLOT),
        )
            .abi_encode(),
    );
    let slot_u256: alloy_primitives::U256 = slot_bytes.into();
    let val_bytes: alloy_primitives::B256 = alloy_primitives::U256::from(WETH_ALPHA_INITIAL_RAW)
        .to_be_bytes::<32>()
        .into();
    provider
        .anvil_set_storage_at(WETH_MAINNET, slot_u256, val_bytes)
        .await
        .expect("anvil_set_storage_at(WETH, balanceOf[alpha], 1000 WETH)");

    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    // Read alpha's WETH balance via the CLI's new --token flag.
    let out = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "wallet",
            "balance",
            "--address",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--token",
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
            "--network",
            "anvil",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "balance --token WETH must succeed against forked Anvil\nstdout: {stdout}\nstderr: {stderr}",
    );
    // WETH has 18 decimals. 1000 WETH = 1e21 raw. Output should include
    // either "1000" or "1000.0..." plus a token symbol hint (default ETH
    // formatter prints whole + fractional). Tolerant: any line with the
    // pre-fund magnitude passes.
    assert!(
        stdout.contains("1000") || stdout.contains("1000.000000"),
        "expected pre-funded WETH balance in stdout, got: {stdout}",
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn wallet_balance_token_usdt_against_anvil_fork_mainnet() {
    // Issue #356 acceptance test, USDT variant. Same pattern as WETH but
    // with 6 decimals. RED: --token flag absent, --decimals override absent.
    // GREEN: USDT balance reads back with auto-detected 6-decimal scale.
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new()
        .fork(MAINNET_RPC_URL)
        .chain_id(31337)
        .spawn();
    let endpoint = anvil.endpoint();

    let rpc_url = alloy_transport_http::reqwest::Url::parse(&endpoint).expect("anvil url");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(rpc_url);

    let alpha_addr: alloy_primitives::Address = "f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .expect("alpha addr");

    // Pre-fund alpha with 1000 USDT at TetherToken's `balances[alpha]` slot.
    let slot_bytes = alloy_primitives::keccak256(
        (
            alpha_addr,
            alloy_primitives::U256::from(USDT_BALANCEOF_SLOT),
        )
            .abi_encode(),
    );
    let slot_u256: alloy_primitives::U256 = slot_bytes.into();
    let val_bytes: alloy_primitives::B256 = alloy_primitives::U256::from(USDT_ALPHA_INITIAL_RAW)
        .to_be_bytes::<32>()
        .into();
    provider
        .anvil_set_storage_at(USDT_MAINNET, slot_u256, val_bytes)
        .await
        .expect("anvil_set_storage_at(USDT, balances[alpha], 1000 USDT)");

    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "wallet",
            "balance",
            "--address",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--token",
            "0xdAC17F958D2ee523a2206206994597C13D831ec7",
            "--network",
            "anvil",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "balance --token USDT must succeed against forked Anvil\nstdout: {stdout}\nstderr: {stderr}",
    );
    // USDT has 6 decimals. 1000 USDT = 1e9 raw. Output should print
    // "1000.000000" if auto-detect works.
    assert!(
        stdout.contains("1000") || stdout.contains("1000.000000"),
        "expected pre-funded USDT balance in stdout, got: {stdout}",
    );
}

#[test]
fn tx_list_command_against_unreachable_rpc_is_not_a_stub() {
    // Per Issue #339 PR-B cycle 5: `eth tx list` is currently a stub
    // returning `Error::Rpc("tx list: wired in PR-B follow-up...")`. PR-B
    // replaces the stub with provider.get_logs / get_block_number scan.
    // RED: stub string leaks into stderr → assertion fails. GREEN: real
    // impl returns network error (unreachable RPC) → assertion passes.
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "tx",
            "list",
            "--limit",
            "5",
            "--rpc-url",
            "http://127.0.0.1:1", // unreachable
        ],
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("PR-B follow-up"),
        "tx list still wired to PR-B stub:\nstdout: {}\nstderr: {stderr}",
        String::from_utf8_lossy(&out.stdout),
    );
}

#[test]
fn erc20_send_command_against_unreachable_rpc_is_not_a_stub() {
    // Lock-down for MINOR #5 (code-reviewer): the original weak version
    // of this test omitted `--name` + `--password`, so the handler exited
    // at `--name required` (InvalidInput, exit 2) before reaching the
    // broadcast path. The assertion (`stderr does not contain
    // "PR-B follow-up"`) passed trivially even if the impl regressed
    // to the stub, because the stub error never had a chance to appear.
    //
    // RED: a stub regression at handler entry would never be caught by
    // the old test — assertion fires after the wrong code path runs.
    // GREEN: pre-create a wallet + pass --name / --password so the path
    // reaches `wallet_send_erc20` → `send_raw_transaction`. Against an
    // unreachable RPC, the real impl yields Error::Rpc from the network
    // layer (exit 3); a stub regression yields Error::Rpc from handler
    // entry with "wired in PR-B follow-up" text. Asserting both the
    // exit code (3, not 2/4/5) and the absent stub string proves the
    // network path was exercised.
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    // Pre-create wallet so --name lookup + --password unlock both pass.
    let _ = run_eth(
        &data_dir,
        &[
            "wallet",
            "create",
            "--name",
            "erc20-sender",
            "--password",
            "test-password",
        ],
    );

    let out = run_eth(
        &data_dir,
        &[
            "erc20",
            "send",
            "--name",
            "erc20-sender",
            "--password",
            "test-password",
            "--token",
            // Valid hex address; not a real ERC-20, but the RPC will fail
            // before any contract call resolves anyway.
            "0x0000000000000000000000000000000000000001",
            "--to",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--amount",
            "1000",
            "--rpc-url",
            "http://127.0.0.1:1", // unreachable
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let code = out.status.code();

    // Lock-down guard: must reach the network layer. Earlier exits prove
    // the test never exercised the impl path it claims to verify.
    assert_ne!(
        code,
        Some(2),
        "lock-down failed: --name / --password / --token parsing rejected\nstdout: {stdout}\nstderr: {stderr}",
    );
    assert_ne!(
        code,
        Some(4),
        "lock-down failed: wallet lookup rejected — pre-create step did not run\nstdout: {stdout}\nstderr: {stderr}",
    );
    assert_ne!(
        code,
        Some(5),
        "lock-down failed: signer unlock rejected — wrong password or pre-create step did not run\nstdout: {stdout}\nstderr: {stderr}",
    );
    // Real impl against unreachable RPC: Error::Rpc from send_raw_transaction
    // (exit 3). Stub regression also exits 3 but with different stderr.
    assert_eq!(
        code,
        Some(3),
        "erc20 send should reach send_raw_transaction and report RPC error\nstdout: {stdout}\nstderr: {stderr}",
    );
    // Bind the assertion to the impl path. Reviewer LOW #1: a stub
    // regression returning Error::Rpc from handler entry with any text
    // other than "PR-B follow-up" (e.g. "TBD", "TODO") would still pass
    // exit-code + absence checks without exercising the impl. Asserting
    // "error sending request" — reqwest's signature for an unreachable
    // host, produced only by a real HTTP call — closes the gap. This
    // signature is stable across which call in the chain fails first
    // (get_chain_id, send_raw_transaction, etc.).
    assert!(
        stderr.contains("error sending request"),
        "expected impl-path network error from real HTTP call:\nstdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        !stderr.contains("PR-B follow-up"),
        "erc20 send still wired to PR-B stub:\nstdout: {stdout}\nstderr: {stderr}",
    );
}

#[test]
fn send_command_without_to_address_yields_exit_2() {
    // L12 review finding (CRITICAL): missing --to defaulted to the zero
    // address — silent ETH burn. Per code-reviewer + type-design both
    // flagged this. RED: missing --to was accepted and broadcast to
    // 0x000…000. GREEN: rejected with InvalidInput (exit 2).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "send",
            "--name",
            "alpha",
            "--password",
            "p",
            "--amount",
            "1000",
            "--rpc-url",
            "http://127.0.0.1:1",
        ],
    );

    assert_eq!(
        out.status.code(),
        Some(2),
        "missing --to must yield bad-input exit code (2)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--to"),
        "stderr should mention --to: {stderr}",
    );
}

#[test]
fn send_command_with_invalid_to_yields_exit_2() {
    // L28 Gate C checklist (code-reviewer IMPORTANT): invalid --to
    // address branch was uncovered. RED: garbage address passed parse.
    // GREEN: rejected with InvalidInput (exit 2).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "send",
            "--name",
            "alpha",
            "--password",
            "p",
            "--to",
            "0xnot-an-address",
            "--amount",
            "1000",
        ],
    );

    assert_eq!(
        out.status.code(),
        Some(2),
        "invalid --to must yield bad-input exit code (2)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

#[test]
fn send_command_with_invalid_amount_yields_exit_2() {
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "send",
            "--name",
            "alpha",
            "--password",
            "p",
            "--to",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--amount",
            "not-a-number",
        ],
    );

    assert_eq!(
        out.status.code(),
        Some(2),
        "invalid --amount must yield bad-input exit code (2)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--amount") || stderr.contains("amount"),
        "stderr should mention amount: {stderr}",
    );
}

#[test]
fn send_command_with_unknown_wallet_yields_exit_4() {
    // L28 Gate C checklist (code-reviewer IMPORTANT): unknown wallet
    // name branch (WalletNotFoundByName → exit 4) was uncovered.
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "send",
            "--name",
            "ghost",
            "--password",
            "p",
            "--to",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--amount",
            "1000",
        ],
    );

    assert_eq!(
        out.status.code(),
        Some(4),
        "unknown wallet must yield wallet/balance exit code (4)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("ghost"),
        "stderr should preserve the user-supplied name: {stderr}",
    );
}

#[test]
fn send_command_with_wrong_password_yields_exit_5() {
    // L28 Gate C checklist (code-reviewer IMPORTANT): wrong-password
    // branch (DecryptionFailed → exit 5) was uncovered. RED: cycle-2
    // code returned generic DecryptionFailed regardless of underlying
    // WalletError variant. GREEN: map_wallet_err preserves variant → exit
    // 5 for actual Crypto failure (wrong password = AES-GCM auth tag
    // mismatch).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let _ = run_eth(
        &data_dir,
        &[
            "wallet",
            "create",
            "--name",
            "alpha",
            "--password",
            "right-password",
        ],
    );

    let out = run_eth(
        &data_dir,
        &[
            "send",
            "--name",
            "alpha",
            "--password",
            "WRONG-password",
            "--to",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--amount",
            "1000",
        ],
    );

    assert_eq!(
        out.status.code(),
        Some(5),
        "wrong password must yield signing-broadcast exit code (5)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.to_lowercase().contains("password") || stderr.to_lowercase().contains("decrypt"),
        "stderr should mention password/decryption: {stderr}",
    );
}

#[test]
fn send_command_with_wallet_identity_attempts_unlock() {
    // Per Issue #339 PR-B cycle 2: `eth send` must accept --name +
    // --password so the handler can unlock the signer before broadcast.
    // RED: clap rejects unknown flags → stderr mentions "unexpected" /
    // "Usage:" → assertion fails. GREEN: flags accepted, handler reaches
    // unlock + broadcast path (still fails at unreachable RPC, but no
    // longer a clap rejection).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    // Pre-create a wallet so --name resolves.
    let _ = run_eth(
        &data_dir,
        &[
            "wallet",
            "create",
            "--name",
            "alpha",
            "--password",
            "test-password",
        ],
    );

    let out = run_eth(
        &data_dir,
        &[
            "send",
            "--name",
            "alpha",
            "--password",
            "test-password",
            "--to",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--amount",
            "1000",
            "--rpc-url",
            "http://127.0.0.1:1", // unreachable
        ],
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("Usage:"),
        "send doesn't yet accept --name / --password:\nstderr: {stderr}",
    );
}

#[test]
fn send_command_against_unreachable_rpc_is_not_a_stub() {
    // Per Issue #337 PR-B: `eth send` is currently a stub returning
    // `Error::Rpc("wallet send-native: wired in PR-B follow-up...")`. PR-B
    // replaces the stub with sign+broadcast. RED: stub string leaks into
    // stderr → assertion fails. GREEN: real impl returns network error
    // (unreachable RPC) → assertion passes. No Anvil required (the
    // unreachable port = deterministic network failure).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "send",
            "--to",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--amount",
            "1000",
            "--rpc-url",
            "http://127.0.0.1:1", // unreachable
        ],
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("PR-B follow-up"),
        "send still wired to PR-B stub:\nstdout: {}\nstderr: {stderr}",
        String::from_utf8_lossy(&out.stdout),
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn send_command_against_anvil_returns_tx_hash() {
    // Per Issue #339 PR-B cycle 3+4: `eth send` must sign + broadcast a
    // real native ETH tx against Anvil. RED: cycle 2 uses
    // `provider.send_transaction` (no signing) → broadcast fails with
    // "missing signature" / nonce mismatch → assertion fails. GREEN:
    // switch to `sign_native_eth_tx` + `encoded_envelope` +
    // `provider.send_raw_transaction` → tx hash returned.
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new().spawn();
    let endpoint = anvil.endpoint();

    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    // Anvil dev mnemonic #0: "test test ... junk" → address
    // 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 pre-funded with 10000 ETH.
    // This is the canonical Anvil default mnemonic (matches `alloy`'s docs
    // and the address Anvil uses for account #0).
    let phrase = "test test test test test test test test test test test junk";
    let _ = run_eth(
        &data_dir,
        &[
            "wallet",
            "import",
            "--name",
            "anvil-acct",
            "--mnemonic",
            phrase,
            "--password",
            "test-password",
            "--network",
            "anvil",
        ],
    );

    let out = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "send",
            "--name",
            "anvil-acct",
            "--password",
            "test-password",
            "--network",
            "anvil",
            "--to",
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8", // Anvil account #1
            "--amount",
            "1000000000000000000", // 1 ETH in wei
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "send must succeed against Anvil\nstdout: {stdout}\nstderr: {stderr}",
    );
    // stdout should contain a tx hash (0x + 64 hex chars = 66 chars total).
    let hex_count = stdout
        .chars()
        .filter(|c| c.is_ascii_hexdigit() && *c != '\n')
        .count();
    assert!(
        stdout.contains("0x") && hex_count >= 64,
        "expected tx hash (0x + 64 hex chars) in stdout, got: {stdout}",
    );
}

// ---------------------------------------------------------------------------
// Anvil-gated RPC tests (L29 opt-in)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn wallet_balance_against_anvil_default_account() {
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new().spawn();
    let endpoint = anvil.endpoint();

    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    // Anvil dev account #0 — pre-funded with 10000 ETH per Anvil defaults.
    // Address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
    let out = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "wallet",
            "balance",
            "--address",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "balance must succeed against Anvil\nstdout: {stdout}\nstderr: {stderr}",
    );
    // Anvil dev accounts start with 10000 ETH = 1e22 wei. Output should
    // mention a non-zero balance.
    assert!(
        stdout.contains("10000") || stdout.contains("10000.000"),
        "expected 10000 ETH balance, got: {stdout}",
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn tx_get_returns_not_found_for_unknown_hash() {
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new().spawn();
    let endpoint = anvil.endpoint();

    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "tx",
            "get",
            "--tx-hash",
            "0x0000000000000000000000000000000000000000000000000000000000000000",
        ],
    );

    // Unknown hash on a live node: returns RPC error → exit 3 per M11.
    assert_eq!(
        out.status.code(),
        Some(3),
        "unknown tx hash must yield rpc-error exit code (3)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn alpha_send_beta_100_weth_against_anvil_fork_mainnet() {
    // Issue #343 acceptance test: alpha sends beta 100 ERC-20 tokens
    // (WETH stand-in for USDC, see #343 body "Implementation notes")
    // against a local Anvil forked from Ethereum mainnet.
    //
    // Flow:
    // 1. Fork Anvil from mainnet via `Anvil::new().fork(MAINNET_RPC_URL)
    //    .chain_id(31337).spawn()` — loads WETH bytecode + storage.
    // 2. Pre-fund alpha with 1000 WETH via `anvil_set_storage_at`:
    //    slot = keccak256(abi.encode(alpha, 3)) (WETH's balanceOf is
    //    at slot 3 after name/symbol/decimals), value = 1000 WETH.
    //    Sidesteps the `anvil_deal_erc20` cheatcode (assumes non-proxy
    //    mapping layout; defeated by USDC's proxy pattern) and the
    //    `MockUSDC::deploy` blocker (alloy-sol-macro 1.6.1 doesn't
    //    generate deploy helpers).
    // 3. Import alpha's wallet via the CLI (Anvil default mnemonic).
    // 4. Run `eth erc20 send --token WETH --amount 100 WETH --network
    //    anvil` against alpha's wallet.
    // 5. Assert exit 0 + tx hash + post-state balances via
    //    `IERC20::balanceOf` eth_call (alpha 900 WETH, beta 100 WETH).
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new()
        .fork(MAINNET_RPC_URL)
        .chain_id(31337)
        .spawn();
    let endpoint = anvil.endpoint();

    let rpc_url = alloy_transport_http::reqwest::Url::parse(&endpoint).expect("anvil url");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(rpc_url);

    // Anvil default accounts:
    //   #0: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 (pre-funded 10000 ETH)
    //   #1: 0x70997970C51812dc3A010C7d01b50e0d17dc79C8
    let alpha_addr: alloy_primitives::Address = "f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .expect("alpha addr");
    let beta_addr: alloy_primitives::Address = "70997970C51812dc3A010C7d01b50e0d17dc79C8"
        .parse()
        .expect("beta addr");

    // Pre-deal 1000 WETH to alpha by writing directly to WETH's
    // `balanceOf[alpha]` storage slot. WETH9 storage layout: `name`,
    // `symbol`, `decimals` at slots 0/1/2, then `mapping(address =>
    // uint256) balanceOf` at slot 3. Solidity mapping layout means the
    // actual storage slot for `balanceOf[alpha]` is
    // `keccak256(abi.encode(alpha, 3))`. Per alloy docs example
    // (https://alloy.rs/examples/node-bindings/anvil_set_storage_at/).
    let slot_bytes = alloy_primitives::keccak256(
        (
            alpha_addr,
            alloy_primitives::U256::from(WETH_BALANCEOF_SLOT),
        )
            .abi_encode(),
    );
    let slot_u256: alloy_primitives::U256 = slot_bytes.into();
    let val_bytes: alloy_primitives::B256 = alloy_primitives::U256::from(WETH_ALPHA_INITIAL_RAW)
        .to_be_bytes::<32>()
        .into();
    provider
        .anvil_set_storage_at(WETH_MAINNET, slot_u256, val_bytes)
        .await
        .expect("anvil_set_storage_at(WETH, balanceOf[alpha], 1000 WETH)");

    // Pre-fund sanity assert (reviewer LOW): a future slot-math regression
    // (wrong slot index, swapped keccak arguments) would surface as a
    // generic "erc20 send failed" inside the CLI rather than a pinpoint
    // assertion near the slot-write. Catch it here so the failure message
    // points at the pre-fund, not the transfer.
    let weth = IERC20::new(WETH_MAINNET, &provider);
    let alpha_balance_after_fund = weth
        .balanceOf(alpha_addr)
        .call()
        .await
        .expect("alpha balanceOf after pre-fund");
    assert_eq!(
        alpha_balance_after_fund,
        alloy_primitives::U256::from(WETH_ALPHA_INITIAL_RAW),
        "pre-fund sanity: alpha should have {} WETH after anvil_set_storage_at, got {alpha_balance_after_fund}",
        WETH_ALPHA_INITIAL_RAW,
    );

    // Import alpha's wallet via the CLI (Anvil default mnemonic).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();
    let phrase = "test test test test test test test test test test test junk";
    let _ = run_eth(
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
            "anvil",
        ],
    );

    // Run the CLI's erc20 send path: alpha -> beta, 100 WETH raw units.
    let out = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "erc20",
            "send",
            "--name",
            "alpha",
            "--password",
            "test-password",
            "--token",
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
            "--to",
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            "--amount",
            &WETH_TRANSFER_AMOUNT_RAW.to_string(),
            "--network",
            "anvil",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "erc20 send must succeed against forked Anvil\nstdout: {stdout}\nstderr: {stderr}",
    );
    // stdout should contain a tx hash (0x + 64 hex chars = 66 chars).
    let hex_count = stdout
        .chars()
        .filter(|c| c.is_ascii_hexdigit() && *c != '\n')
        .count();
    assert!(
        stdout.contains("0x") && hex_count >= 64,
        "expected tx hash (0x + 64 hex chars) in stdout: {stdout}",
    );

    // Read post-state balances via IERC20 ABI (eth_call, no signing).
    let weth = IERC20::new(WETH_MAINNET, &provider);
    let alpha_balance = weth
        .balanceOf(alpha_addr)
        .call()
        .await
        .expect("alpha balanceOf");
    let beta_balance = weth
        .balanceOf(beta_addr)
        .call()
        .await
        .expect("beta balanceOf");

    let expected_alpha_remaining =
        alloy_primitives::U256::from(WETH_ALPHA_INITIAL_RAW - WETH_TRANSFER_AMOUNT_RAW);
    let expected_beta_received = alloy_primitives::U256::from(WETH_TRANSFER_AMOUNT_RAW);

    assert_eq!(
        alpha_balance,
        expected_alpha_remaining,
        "alpha should have {} WETH after sending {} to beta (started {}): got {alpha_balance}",
        WETH_ALPHA_INITIAL_RAW - WETH_TRANSFER_AMOUNT_RAW,
        WETH_TRANSFER_AMOUNT_RAW,
        WETH_ALPHA_INITIAL_RAW,
    );
    assert_eq!(
        beta_balance, expected_beta_received,
        "beta should have {} WETH after receiving from alpha (started 0): got {beta_balance}",
        WETH_TRANSFER_AMOUNT_RAW,
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn alpha_send_beta_100_usdt_against_anvil_fork_mainnet() {
    // Issue #343 acceptance test, USDT variant. Real stablecoin with
    // 6 decimals (matching the original #343 USDC spec — operator picked
    // USDT in addition to USDC; see PR #347 thread for the rationale).
    //
    // USDT (TetherToken) is a single contract on mainnet — no proxy, no
    // upgradeability — so the same Anvil-fork + `anvil_set_storage_at`
    // approach used for WETH works here. Storage layout: `balances`
    // mapping (no underscore; Tether naming) is the first state var
    // → slot 0. `balanceOf[alpha]` is at `keccak256(abi.encode(alpha, 0))`.
    //
    // USDC variant deferred — USDC's TransparentUpgradeableProxy layout
    // makes slot math unreliable without finding the impl address from
    // the proxy's EIP-1967 storage slot (work for a follow-up PR).
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new()
        .fork(MAINNET_RPC_URL)
        .chain_id(31337)
        .spawn();
    let endpoint = anvil.endpoint();

    let rpc_url = alloy_transport_http::reqwest::Url::parse(&endpoint).expect("anvil url");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(rpc_url);

    let alpha_addr: alloy_primitives::Address = "f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .expect("alpha addr");
    let beta_addr: alloy_primitives::Address = "70997970C51812dc3A010C7d01b50e0d17dc79C8"
        .parse()
        .expect("beta addr");

    // Pre-fund alpha with 1000 USDT at TetherToken's `balances[alpha]`
    // mapping slot (slot 0). Solidity mapping slot math:
    // keccak256(abi.encode(key, slot)).
    let slot_bytes = alloy_primitives::keccak256(
        (
            alpha_addr,
            alloy_primitives::U256::from(USDT_BALANCEOF_SLOT),
        )
            .abi_encode(),
    );
    let slot_u256: alloy_primitives::U256 = slot_bytes.into();
    let val_bytes: alloy_primitives::B256 = alloy_primitives::U256::from(USDT_ALPHA_INITIAL_RAW)
        .to_be_bytes::<32>()
        .into();
    provider
        .anvil_set_storage_at(USDT_MAINNET, slot_u256, val_bytes)
        .await
        .expect("anvil_set_storage_at(USDT, balances[alpha], 1000 USDT)");

    // Pre-fund sanity assert.
    let usdt = IERC20::new(USDT_MAINNET, &provider);
    let alpha_balance_after_fund = usdt
        .balanceOf(alpha_addr)
        .call()
        .await
        .expect("alpha balanceOf after pre-fund");
    assert_eq!(
        alpha_balance_after_fund,
        alloy_primitives::U256::from(USDT_ALPHA_INITIAL_RAW),
        "pre-fund sanity: alpha should have {} USDT after anvil_set_storage_at, got {alpha_balance_after_fund}",
        USDT_ALPHA_INITIAL_RAW,
    );

    // Import alpha's wallet via the CLI (Anvil default mnemonic).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();
    let phrase = "test test test test test test test test test test test junk";
    let _ = run_eth(
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
            "anvil",
        ],
    );

    // Run the CLI's erc20 send path: alpha -> beta, 100 USDT raw units.
    let out = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "erc20",
            "send",
            "--name",
            "alpha",
            "--password",
            "test-password",
            "--token",
            "0xdAC17F958D2ee523a2206206994597C13D831ec7",
            "--to",
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            "--amount",
            &USDT_TRANSFER_AMOUNT_RAW.to_string(),
            "--network",
            "anvil",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "erc20 send must succeed against forked Anvil\nstdout: {stdout}\nstderr: {stderr}",
    );
    let hex_count = stdout
        .chars()
        .filter(|c| c.is_ascii_hexdigit() && *c != '\n')
        .count();
    assert!(
        stdout.contains("0x") && hex_count >= 64,
        "expected tx hash (0x + 64 hex chars) in stdout: {stdout}",
    );

    // Read post-state balances via IERC20 ABI (eth_call, no signing).
    let alpha_balance = usdt
        .balanceOf(alpha_addr)
        .call()
        .await
        .expect("alpha balanceOf");
    let beta_balance = usdt
        .balanceOf(beta_addr)
        .call()
        .await
        .expect("beta balanceOf");

    let expected_alpha_remaining =
        alloy_primitives::U256::from(USDT_ALPHA_INITIAL_RAW - USDT_TRANSFER_AMOUNT_RAW);
    let expected_beta_received = alloy_primitives::U256::from(USDT_TRANSFER_AMOUNT_RAW);

    assert_eq!(
        alpha_balance,
        expected_alpha_remaining,
        "alpha should have {} USDT after sending {} to beta (started {}): got {alpha_balance}",
        USDT_ALPHA_INITIAL_RAW - USDT_TRANSFER_AMOUNT_RAW,
        USDT_TRANSFER_AMOUNT_RAW,
        USDT_ALPHA_INITIAL_RAW,
    );
    assert_eq!(
        beta_balance, expected_beta_received,
        "beta should have {} USDT after receiving from alpha (started 0): got {beta_balance}",
        USDT_TRANSFER_AMOUNT_RAW,
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn alpha_send_beta_100_dai_against_anvil_fork_mainnet() {
    // Issue #343 acceptance test, DAI variant. Third real stablecoin
    // (18 decimals, single contract — same Anvil-fork + slot-write
    // pattern as WETH + USDT). Operator picked "3rd token variant"
    // to validate the pattern generalizes across stablecoins.
    //
    // DAI (MakerDAO DSToken) is a single contract — no proxy, no
    // upgradeability. Storage layout: `balances` mapping (DSToken's
    // `balances` public state var) is the first state var → slot 0.
    // `balances[alpha]` is at `keccak256(abi.encode(alpha, 0))`.
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new()
        .fork(MAINNET_RPC_URL)
        .chain_id(31337)
        .spawn();
    let endpoint = anvil.endpoint();

    let rpc_url = alloy_transport_http::reqwest::Url::parse(&endpoint).expect("anvil url");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(rpc_url);

    let alpha_addr: alloy_primitives::Address = "f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .expect("alpha addr");
    let beta_addr: alloy_primitives::Address = "70997970C51812dc3A010C7d01b50e0d17dc79C8"
        .parse()
        .expect("beta addr");

    // Pre-fund alpha with 1000 DAI at DAI's `balances[alpha]` mapping
    // slot (slot 0 for DSToken). Solidity mapping slot math:
    // keccak256(abi.encode(key, slot)).
    let slot_bytes = alloy_primitives::keccak256(
        (alpha_addr, alloy_primitives::U256::from(DAI_BALANCEOF_SLOT)).abi_encode(),
    );
    let slot_u256: alloy_primitives::U256 = slot_bytes.into();
    let val_bytes: alloy_primitives::B256 = alloy_primitives::U256::from(DAI_ALPHA_INITIAL_RAW)
        .to_be_bytes::<32>()
        .into();
    provider
        .anvil_set_storage_at(DAI_MAINNET, slot_u256, val_bytes)
        .await
        .expect("anvil_set_storage_at(DAI, balances[alpha], 1000 DAI)");

    // Pre-fund sanity assert.
    let dai = IERC20::new(DAI_MAINNET, &provider);
    let alpha_balance_after_fund = dai
        .balanceOf(alpha_addr)
        .call()
        .await
        .expect("alpha balanceOf after pre-fund");
    assert_eq!(
        alpha_balance_after_fund,
        alloy_primitives::U256::from(DAI_ALPHA_INITIAL_RAW),
        "pre-fund sanity: alpha should have {} DAI after anvil_set_storage_at, got {alpha_balance_after_fund}",
        DAI_ALPHA_INITIAL_RAW,
    );

    // Import alpha's wallet via the CLI (Anvil default mnemonic).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();
    let phrase = "test test test test test test test test test test test junk";
    let _ = run_eth(
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
            "anvil",
        ],
    );

    // Run the CLI's erc20 send path: alpha -> beta, 100 DAI raw units.
    let out = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "erc20",
            "send",
            "--name",
            "alpha",
            "--password",
            "test-password",
            "--token",
            "0x6B175474E89094C44Da98b954EedeAC495271d0F",
            "--to",
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            "--amount",
            &DAI_TRANSFER_AMOUNT_RAW.to_string(),
            "--network",
            "anvil",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "erc20 send must succeed against forked Anvil\nstdout: {stdout}\nstderr: {stderr}",
    );
    let hex_count = stdout
        .chars()
        .filter(|c| c.is_ascii_hexdigit() && *c != '\n')
        .count();
    assert!(
        stdout.contains("0x") && hex_count >= 64,
        "expected tx hash (0x + 64 hex chars) in stdout: {stdout}",
    );

    // Read post-state balances via IERC20 ABI (eth_call, no signing).
    let alpha_balance = dai
        .balanceOf(alpha_addr)
        .call()
        .await
        .expect("alpha balanceOf");
    let beta_balance = dai
        .balanceOf(beta_addr)
        .call()
        .await
        .expect("beta balanceOf");

    let expected_alpha_remaining =
        alloy_primitives::U256::from(DAI_ALPHA_INITIAL_RAW - DAI_TRANSFER_AMOUNT_RAW);
    let expected_beta_received = alloy_primitives::U256::from(DAI_TRANSFER_AMOUNT_RAW);

    assert_eq!(
        alpha_balance,
        expected_alpha_remaining,
        "alpha should have {} DAI after sending {} to beta (started {}): got {alpha_balance}",
        DAI_ALPHA_INITIAL_RAW - DAI_TRANSFER_AMOUNT_RAW,
        DAI_TRANSFER_AMOUNT_RAW,
        DAI_ALPHA_INITIAL_RAW,
    );
    assert_eq!(
        beta_balance, expected_beta_received,
        "beta should have {} DAI after receiving from alpha (started 0): got {beta_balance}",
        DAI_TRANSFER_AMOUNT_RAW,
    );
}
