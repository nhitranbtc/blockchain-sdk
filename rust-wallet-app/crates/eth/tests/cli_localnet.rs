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
        stderr.contains("unknown") && stderr.contains("polygon"),
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
    // Lock-down against stub regression: the real impl emits either
    // `eth_call balanceOf` (token_balance) or `eth_call decimals`
    // (query_decimals) in stderr — whichever eth_call fails first
    // against the unreachable RPC. A stub regression at handler entry
    // returning Error::Rpc from arbitrary text would NOT contain either
    // function name, since erc20::* are the only sites that format the
    // function selector into the error context.
    // Issue #366 swapped the call order so query_decimals runs first
    // when decimals_override is None; "decimals" is now the canonical
    // substring for the non-override path. "balanceOf" remains valid
    // evidence for the override path / sibling tests.
    assert!(
        stderr.contains("balanceOf") || stderr.contains("decimals"),
        "expected impl-path network error from real eth_call (balanceOf or decimals):\nstdout: {stdout}\nstderr: {stderr}",
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

#[test]
fn wallet_balance_all_flag_against_unreachable_rpc_exits_3_with_balanceof() {
    // Issue #358 — `eth wallet balance --all` iterates the bundled token
    // registry + any --token overrides, printing one line per token. AC #7
    // requires a lock-down always-on test: --all against an unreachable
    // RPC must reach the per-token balanceOf call (exit 3, stderr carries
    // the `balanceOf` substring from `erc20::token_balance`).
    //
    // RED: --all flag absent → clap rejects → exit 2 + "unexpected
    // argument" / "Usage:" leaks to stderr.
    // GREEN: clap accepts the flag, handler reaches `erc20::token_balance`,
    // unreachable RPC yields `Error::Rpc("eth_call balanceOf: ...")` →
    // exit 3 + stderr contains `balanceOf` (proves the per-token iteration
    // fired; a stub regression returning Error::Rpc from handler entry
    // would not contain the function selector substring).
    //
    // Failure isolation (AC #4) is exercised separately by the L29
    // operator-smoke test below — the unreachable-RPC case dominates with
    // the first RPC error per AC #7's exit-3 contract.
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "wallet",
            "balance",
            "--address",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--all",
            "--network",
            "sepolia", // sepolia.json has USDC entry → at least one token to iterate
            "--rpc-url",
            "http://127.0.0.1:1", // unreachable
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let code = out.status.code();

    // Lock-down: clap rejected --all → must NOT be exit 2.
    assert_ne!(
        code,
        Some(2),
        "lock-down: --all flag was rejected by clap (exit 2); not yet wired\nstdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        !stderr.contains("unexpected argument")
            && !stderr.contains("Usage:")
            && !stderr.contains("unrecognized"),
        "stderr must not contain clap rejection markers:\nstderr: {stderr}",
    );
    // Real impl against unreachable RPC: Error::Rpc from the first
    // per-token `eth_call balanceOf` (exit 3).
    assert_eq!(
        code,
        Some(3),
        "wallet balance --all should reach per-token balanceOf and report RPC error\nstdout: {stdout}\nstderr: {stderr}",
    );
    // Lock-down against stub regression: the real impl emits
    // `eth_call balanceOf: ...` in stderr from `erc20::token_balance`
    // (which prefixes the call context in its Error::Rpc mapping —
    // see `src/erc20.rs:118`). A stub regression at handler entry would
    // not contain this substring.
    assert!(
        stderr.contains("balanceOf"),
        "expected per-token balanceOf call to fire — stderr must contain `balanceOf` from `erc20::token_balance`:\nstdout: {stdout}\nstderr: {stderr}",
    );
}

#[test]
fn wallet_balance_all_json_against_unreachable_rpc_emits_error_rows() {
    // Issue #380 AC: --json output must surface per-token failures in the
    // structured output (not just stderr). Field-presence discriminator:
    // success rows carry `balance`+`decimals`, failure rows carry `error`+`context`.
    // Lock-down: when ALL RPCs fail, every row in the JSON array has `error`;
    // none has `balance`. Proves the failure channel is wired (not stubbed
    // out by the unreachable-RPC path swallowing the error).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "wallet",
            "balance",
            "--address",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--all",
            "--json",
            "--network",
            "sepolia",
            "--rpc-url",
            "http://127.0.0.1:1",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // AC contract: unreachable RPC → exit 3 (Error::Rpc from first failed call).
    assert_eq!(
        out.status.code(),
        Some(3),
        "wallet balance --all --json against unreachable RPC must exit 3\nstdout: {stdout}\nstderr: {stderr}",
    );
    // Per-token stderr must be suppressed in --json mode — the JSON row
    // carries the failure (Issue #380). The outer one-line `error: {e}`
    // summary from main.rs is operator noise and acceptable (operators
    // using --json pipe `2>/dev/null` or filter it out at the call site).
    assert!(
        !stderr.contains("error: balance for") && !stderr.contains("error: decimals for"),
        "--json mode must not emit per-token error lines to stderr; got: {stderr}"
    );
    // Sepolia registry has 1 USDC entry + no overrides → 1 row in the array.
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("stdout is not valid JSON: {e}\nstdout: {stdout}\nstderr: {stderr}")
    });
    let rows = parsed
        .as_array()
        .unwrap_or_else(|| panic!("top-level must be a JSON array: {parsed:?}"));
    assert!(
        !rows.is_empty(),
        "JSON array must contain at least one row, got: {rows:?}"
    );
    for (i, row) in rows.iter().enumerate() {
        let obj = row
            .as_object()
            .unwrap_or_else(|| panic!("row {i} not an object: {row:?}"));
        // Common keys for any row (success or failure).
        for key in ["symbol", "address", "error", "context"] {
            assert!(
                obj.contains_key(key),
                "row {i} missing key {key:?}: {row:?}"
            );
        }
        // Failure discriminator: ALL RPCs unreachable → NO row has `balance`.
        assert!(
            !obj.contains_key("balance"),
            "row {i} must NOT have `balance` when RPC fails: {row:?}"
        );
        assert!(
            !obj.contains_key("decimals"),
            "row {i} must NOT have `decimals` when RPC fails: {row:?}"
        );
        // `error` is a non-empty string.
        let err = obj["error"]
            .as_str()
            .unwrap_or_else(|| panic!("row {i} error not a string: {row:?}"));
        assert!(!err.is_empty(), "row {i} error must be non-empty: {row:?}");
        // `context` is "balance" or "decimals" (which RPC step failed).
        let ctx = obj["context"]
            .as_str()
            .unwrap_or_else(|| panic!("row {i} context not a string: {row:?}"));
        assert!(
            ctx == "balance" || ctx == "decimals",
            "row {i} context must be `balance` or `decimals`, got {ctx:?}: {row:?}"
        );
    }
}

#[test]
fn wallet_balance_all_json_override_without_decimals_forces_query_decimals_failure() {
    // Companion to the lock-down test above. Sepolia's only registry entry
    // (USDC) carries `registry_decimals: Some(6)`, so `query_decimals` is
    // skipped (cache hit) and the failure fires on `token_balance` →
    // `context: "balance"`. To exercise the `context: "decimals"` code
    // path we add a `--token` override with no registry hit AND no
    // `--decimals` flag — the handler must call `query_decimals` against
    // the unreachable RPC, surfacing a row with `context: "decimals"`.
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "wallet",
            "balance",
            "--address",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--all",
            "--json",
            "--token",
            "0x000000000000000000000000000000000000beef",
            "--network",
            "sepolia",
            "--rpc-url",
            "http://127.0.0.1:1",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("stdout is not valid JSON: {e}\nstdout: {stdout}\nstderr: {stderr}")
    });
    let rows = parsed
        .as_array()
        .unwrap_or_else(|| panic!("top-level must be a JSON array: {parsed:?}"));

    // Sepolia USDC (registry hit, cache hit on decimals → context "balance")
    // + override (no registry hit, no --decimals → context "decimals") = 2 rows.
    assert!(
        rows.len() >= 2,
        "expected >=2 rows (USDC + override), got: {rows:?}"
    );

    // At least one row must carry `context: "decimals"` (proves the path
    // is reachable, not vacuously true via the existing balance-only test).
    let any_decimals_context = rows.iter().any(|r| r["context"] == "decimals");
    assert!(
        any_decimals_context,
        "at least one row must carry `context: decimals` to exercise that branch: {rows:?}"
    );

    // Locate the override row by EIP-55 address (Display format).
    let override_addr_str = "0x000000000000000000000000000000000000bEEF";
    let override_row = rows
        .iter()
        .find(|r| r["address"] == override_addr_str)
        .unwrap_or_else(|| {
            panic!("override row with address {override_addr_str} missing: {rows:?}")
        });
    assert_eq!(
        override_row["context"], "decimals",
        "override row (no registry hit, no --decimals) must surface query_decimals error: {override_row:?}"
    );
}

#[test]
fn wallet_balance_all_with_anvil_network_and_no_token_overrides_yields_exit_2() {
    // L12 coverage G1 — `--network anvil --all` with no `--token`
    // override finds zero entries (Anvil registry is the empty v0.2
    // stub per `load_anvil_returns_empty_list_for_v0_2_stub`). The
    // handler must surface the `entries.is_empty()` guard as
    // `InvalidInput` (exit 2) before any RPC call — proves the empty-
    // registry branch is wired (currently silent dead code).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "wallet",
            "balance",
            "--address",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--all",
            "--network",
            "anvil",
            // RPC URL omitted — guard fires before any RPC call.
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(2),
        "empty-registry --all must yield bad-input exit code (2)\nstdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        stderr.contains("chain_id=31337"),
        "stderr should name the empty registry's chain_id so operators understand why --all found nothing:\nstderr: {stderr}",
    );
    assert!(
        stderr.contains("--all requires at least one bundled entry or --token"),
        "stderr should hint at the --token override escape hatch:\nstderr: {stderr}",
    );
}

#[test]
fn wallet_balance_all_with_invalid_token_override_yields_exit_2() {
    // L12 coverage G4 + Issue #379 — `--all --token 0xnot-an-address`
    // exercises the clap-level address validation. After #379 the
    // `--token` field is parsed by clap's `value_parser = parse_address`,
    // so bad addresses are rejected at parse time with clap's
    // `invalid value '...' for '--token ...'` format. Locks that
    // (a) the handler-level `Address::from_str(...).map_err(...)` path
    // is gone (handler never runs), and (b) the rejection echoes the
    // invalid value back to the operator.
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "wallet",
            "balance",
            "--address",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--all",
            "--token",
            "0xnot-an-address",
            "--network",
            "sepolia",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(2),
        "invalid --token inside --all must yield bad-input exit code (2)\nstdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        stderr.contains("--token"),
        "stderr should mention --token:\nstderr: {stderr}",
    );
    // clap-level rejection (Issue #379): `invalid value '0xnot-an-address'
    // for '--token <TOKEN>'`. The phrase `invalid value` is clap-specific
    // — handler-level rejection would say `Invalid input:` instead. A
    // regression that drops the value_parser and falls back to handler
    // parsing would fail this assertion.
    assert!(
        stderr.contains("invalid value") && stderr.contains("0xnot-an-address"),
        "stderr should contain clap's `invalid value` marker + the bad value (locks value_parser path):\nstderr: {stderr}",
    );
}

#[test]
fn wallet_balance_with_decimals_without_token_or_all_yields_exit_2() {
    // L12 coverage G5 — manual `--decimals` validation in `main.rs`
    // replaces the removed `requires = "token"` (clap has no OR).
    // Without --token or --all the override is meaningless.
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let out = run_eth(
        &data_dir,
        &[
            "wallet",
            "balance",
            "--address",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--decimals",
            "6",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(2),
        "--decimals alone (no --token / --all) must yield bad-input exit code (2)\nstdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        stderr.contains("--decimals requires --token or --all"),
        "stderr should name the rejection:\nstderr: {stderr}",
    );
}

#[test]
fn wallet_balance_with_multiple_token_without_all_yields_exit_2() {
    // L12 coverage G6 — multi-`--token` without `--all` would silently
    // drop all but the first token (L12 review H-3). The guard
    // surfaces as `InvalidInput` so the operator adds `--all` (or
    // drops the extras) — never silently loses intent.
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
            "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", // USDC
            "--token",
            "0xdAC17F958D2ee523a2206206994597C13D831ec7", // USDT
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(2),
        "multiple --token without --all must yield bad-input exit code (2)\nstdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        stderr.contains("multiple --token values require --all"),
        "stderr should name the rejection:\nstderr: {stderr}",
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn wallet_balance_all_batch_mainnet_prints_one_line_per_token() {
    // Issue #358 AC #1, #3 — `eth wallet balance --all --network mainnet`
    // against a forked Anvil iterates the bundled mainnet registry
    // (USDC, USDT, DAI) and prints one line per token in
    // `<symbol> <scaled_balance> <token-addr>` format. We pre-fund USDT
    // + DAI (slot 2 each — both single contracts, no proxy) and leave
    // USDC at 0 (TransparentUpgradeableProxy returns 0 for un-funded
    // holders without any pre-fund; balanceOf roundtrip still works).
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new()
        .fork(MAINNET_RPC_URL)
        .chain_id(1)
        .spawn();
    let endpoint = anvil.endpoint();

    let rpc_url = alloy_transport_http::reqwest::Url::parse(&endpoint).expect("anvil url");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(rpc_url);

    let alpha_addr: alloy_primitives::Address = "f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .expect("alpha addr");

    // Pre-fund USDT at slot 2 (TetherToken's `balances` mapping).
    let usdt_slot = alloy_primitives::keccak256(
        (
            alpha_addr,
            alloy_primitives::U256::from(USDT_BALANCEOF_SLOT),
        )
            .abi_encode(),
    );
    let usdt_slot_u: alloy_primitives::U256 = usdt_slot.into();
    let usdt_val: alloy_primitives::B256 = alloy_primitives::U256::from(USDT_ALPHA_INITIAL_RAW)
        .to_be_bytes::<32>()
        .into();
    provider
        .anvil_set_storage_at(USDT_MAINNET, usdt_slot_u, usdt_val)
        .await
        .expect("anvil_set_storage_at(USDT, balances[alpha], 1000 USDT)");

    // Pre-fund DAI at slot 2 (DSToken's `balances` mapping — same slot
    // math as USDT; both contracts have 2 state vars before balances).
    let dai_slot = alloy_primitives::keccak256(
        (alpha_addr, alloy_primitives::U256::from(DAI_BALANCEOF_SLOT)).abi_encode(),
    );
    let dai_slot_u: alloy_primitives::U256 = dai_slot.into();
    let dai_val: alloy_primitives::B256 = alloy_primitives::U256::from(DAI_ALPHA_INITIAL_RAW)
        .to_be_bytes::<32>()
        .into();
    provider
        .anvil_set_storage_at(DAI_MAINNET, dai_slot_u, dai_val)
        .await
        .expect("anvil_set_storage_at(DAI, balances[alpha], 1000 DAI)");

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
            "--all",
            "--network",
            "mainnet",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "wallet balance --all mainnet must succeed against forked Anvil\nstdout: {stdout}\nstderr: {stderr}",
    );
    // AC #1: one line per registered token — USDC + USDT + DAI.
    // USDC line: balance 0 (no pre-fund); USDT line: 1000.000000;
    // DAI line: 1000 (18 decimals, no fractional digits).
    assert!(
        stdout.contains("USDC 0"),
        "expected USDC line in stdout (proxy returns 0 for un-funded), got: {stdout}",
    );
    assert!(
        stdout.contains("USDT 1000"),
        "expected pre-funded USDT balance in stdout, got: {stdout}",
    );
    assert!(
        stdout.contains("DAI 1000"),
        "expected pre-funded DAI balance in stdout, got: {stdout}",
    );
    // Each line carries the contract address (EIP-55 mixed-case form
    // after `{:#x}` — alloy 1.8 `Address` Display produces the checksum)
    // so the operator can verify which entry printed.
    assert!(
        stdout.contains("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
        "expected USDC contract address (EIP-55) in stdout, got: {stdout}",
    );
    assert!(
        stdout.contains("0xdAC17F958D2ee523a2206206994597C13D831ec7"),
        "expected USDT contract address (EIP-55) in stdout, got: {stdout}",
    );
    assert!(
        stdout.contains("0x6B175474E89094C44Da98b954EedeAC495271d0F"),
        "expected DAI contract address (EIP-55) in stdout, got: {stdout}",
    );
    // Three distinct lines = three registry entries iterated (no
    // duplicates from the registry + a spurious override).
    let token_lines = stdout
        .lines()
        .filter(|l| l.starts_with("USDC ") || l.starts_with("USDT ") || l.starts_with("DAI "))
        .count();
    assert_eq!(
        token_lines, 3,
        "expected exactly 3 token lines (USDC + USDT + DAI), got {token_lines}:\nstdout: {stdout}",
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn wallet_balance_all_failure_isolation_logs_per_token_errors_and_exits_0() {
    // Issue #358 AC #2, #4 — `--all` + `--token <STOP-only-bytecode-addr>`
    // exercises both ad-hoc-override (AC #2) and failure isolation (AC
    // #4). The override target returns empty bytes from `eth_call` →
    // `AbiDecodeFailed` (exit 2) per-token. Pre-funded USDT/DAI registry
    // entries still succeed, so the batch exits 0 with the override
    // error logged to stderr.
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new()
        .fork(MAINNET_RPC_URL)
        .chain_id(1)
        .spawn();
    let endpoint = anvil.endpoint();

    let rpc_url = alloy_transport_http::reqwest::Url::parse(&endpoint).expect("anvil url");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(rpc_url);

    // Install STOP-only bytecode at `NOT_ERC20_ADDR` so the override
    // target's `balanceOf` returns empty bytes → decode-fail path.
    provider
        .anvil_set_code(
            NOT_ERC20_ADDR,
            alloy_primitives::Bytes::from_static(STOP_BYTECODE),
        )
        .await
        .expect("anvil_set_code(STOP-only bytecode at 0x...beef)");

    let alpha_addr: alloy_primitives::Address = "f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .expect("alpha addr");

    // Pre-fund USDT at slot 2 so the registry USDT entry succeeds.
    let usdt_slot = alloy_primitives::keccak256(
        (
            alpha_addr,
            alloy_primitives::U256::from(USDT_BALANCEOF_SLOT),
        )
            .abi_encode(),
    );
    let usdt_slot_u: alloy_primitives::U256 = usdt_slot.into();
    let usdt_val: alloy_primitives::B256 = alloy_primitives::U256::from(USDT_ALPHA_INITIAL_RAW)
        .to_be_bytes::<32>()
        .into();
    provider
        .anvil_set_storage_at(USDT_MAINNET, usdt_slot_u, usdt_val)
        .await
        .expect("anvil_set_storage_at(USDT, balances[alpha], 1000 USDT)");

    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    // --token override = STOP-only address → decode-fail path. AC #2
    // says overrides append AFTER registry entries; AC #4 says per-token
    // failure doesn't abort the batch. Both exercised by this single run.
    let out = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "wallet",
            "balance",
            "--address",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--all",
            "--token",
            &format!("{NOT_ERC20_ADDR:#x}"),
            // --decimals 18 forces `token_balance` to fire (skipping the
            // `query_decimals` auto-detect, which would otherwise hit the
            // STOP-only bytecode first and fail with the `"decimals"`
            // context — not what the assertion expects).
            "--decimals",
            "18",
            "--network",
            "mainnet",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // AC #4: at least one registry token succeeded → exit 0.
    assert_eq!(
        out.status.code(),
        Some(0),
        "wallet balance --all + --token override must exit 0 when at least one registry token succeeded\nstdout: {stdout}\nstderr: {stderr}",
    );
    // AC #2: USDT (registry entry) + override 0x...beef both attempted;
    // USDT printed successfully → success line in stdout. The `--decimals 18`
    // override (H-2 fix from L12 code-reviewer, see PR #371) forces
    // `format_wei_as(1e9, 18)` = `"0.000000001000000000"` (raw 1e9 / 10^18 = 0,
    // frac padded to 18 digits). The assertion matches that formatted output.
    assert!(
        stdout.contains("USDT 0.000000001000000000"),
        "expected pre-funded USDT balance (raw 1e9 at --decimals 18) in stdout despite override failure, got: {stdout}",
    );
    // AC #4: override error logged to stderr (per-token failure didn't
    // abort the batch — the operator sees which subset failed).
    assert!(
        stderr.contains(&*format!("{NOT_ERC20_ADDR:#x}")),
        "expected override address in stderr failure log, got: {stderr}",
    );
    assert!(
        stderr.contains("balanceOf"),
        "expected balanceOf context in override failure stderr, got: {stderr}",
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn wallet_balance_all_json_output_emits_array_of_rows() {
    // Issue #358 AC #5 — `--json` flag emits an array of
    // `{symbol, address, balance, decimals}` rows instead of the
    // line-per-token text format. Verify JSON shape + per-row fields.
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new()
        .fork(MAINNET_RPC_URL)
        .chain_id(1)
        .spawn();
    let endpoint = anvil.endpoint();

    let rpc_url = alloy_transport_http::reqwest::Url::parse(&endpoint).expect("anvil url");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(rpc_url);

    let alpha_addr: alloy_primitives::Address = "f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .expect("alpha addr");

    // Pre-fund USDT so the JSON output has a non-trivial balance row.
    let usdt_slot = alloy_primitives::keccak256(
        (
            alpha_addr,
            alloy_primitives::U256::from(USDT_BALANCEOF_SLOT),
        )
            .abi_encode(),
    );
    let usdt_slot_u: alloy_primitives::U256 = usdt_slot.into();
    let usdt_val: alloy_primitives::B256 = alloy_primitives::U256::from(USDT_ALPHA_INITIAL_RAW)
        .to_be_bytes::<32>()
        .into();
    provider
        .anvil_set_storage_at(USDT_MAINNET, usdt_slot_u, usdt_val)
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
            "--all",
            "--json",
            "--network",
            "mainnet",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "wallet balance --all --json must succeed against forked Anvil\nstdout: {stdout}\nstderr: {stderr}",
    );
    // Parse stdout as JSON. The array may include USDC (balance 0),
    // USDT (balance 1000.000000), DAI (balance 0) — at least USDT must
    // appear with the expected balance. Failed tokens are NOT in JSON
    // output (operator parses `decimals` field instead of inspecting
    // the output array — same shape as `erc20 register --list`).
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}\nstdout: {stdout}"));
    let rows = parsed.as_array().expect("top-level must be a JSON array");
    assert!(
        !rows.is_empty(),
        "JSON array must contain at least one row, got: {rows:?}"
    );
    // Every row must have the four required keys (AC #5).
    for (i, row) in rows.iter().enumerate() {
        let obj = row
            .as_object()
            .unwrap_or_else(|| panic!("row {i} not an object: {row:?}"));
        for key in ["symbol", "address", "balance", "decimals"] {
            assert!(
                obj.contains_key(key),
                "row {i} missing key {key:?}: {row:?}"
            );
        }
        // `decimals` must be a JSON number.
        assert!(
            obj["decimals"].is_number(),
            "row {i} decimals must be a number, got {:?}: {row:?}",
            obj["decimals"],
        );
    }
    // At least one row must be USDT with the pre-funded balance.
    let usdt_row = rows
        .iter()
        .find(|r| r["symbol"] == "USDT")
        .expect("USDT row must be in JSON output");
    assert_eq!(
        // `format_wei_as(1_000_000_000, 6)` returns `"1000"` — the helper
        // omits the `.000000` fractional part when frac is zero. (Previous
        // assertion expected `"1000.000000"` because the author misread the
        // helper; the impl is correct.) Whole = 1000, frac = 0.
        usdt_row["balance"], "1000",
        "USDT balance must be the pre-funded 1000 USDT (whole=1000, frac=0, format_wei_as drops trailing zeros), got: {usdt_row:?}"
    );
    assert_eq!(
        usdt_row["decimals"], 6,
        "USDT decimals must be 6 from the registry, got: {usdt_row:?}"
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn wallet_balance_all_json_failure_channel_emits_mixed_rows() {
    // Issue #380 — `--json` output must surface per-token failures alongside
    // successes in the same array. Field-presence discriminator: success
    // rows carry `balance`+`decimals`, failure rows carry `error`+`context`.
    // This L29 mirrors `failure_isolation_logs_per_token_errors_and_exits_0`
    // but asserts the structured JSON channel (not just stderr).
    //
    // Setup: forked mainnet Anvil, pre-fund USDT (success path) + install
    // STOP-only bytecode at the override address (decode-fail path).
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new()
        .fork(MAINNET_RPC_URL)
        .chain_id(1)
        .spawn();
    let endpoint = anvil.endpoint();

    let rpc_url = alloy_transport_http::reqwest::Url::parse(&endpoint).expect("anvil url");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(rpc_url);

    let alpha_addr: alloy_primitives::Address = "f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .expect("alpha addr");

    // Pre-fund USDT at slot 2 so the registry USDT entry succeeds.
    let usdt_slot = alloy_primitives::keccak256(
        (
            alpha_addr,
            alloy_primitives::U256::from(USDT_BALANCEOF_SLOT),
        )
            .abi_encode(),
    );
    let usdt_slot_u: alloy_primitives::U256 = usdt_slot.into();
    let usdt_val: alloy_primitives::B256 = alloy_primitives::U256::from(USDT_ALPHA_INITIAL_RAW)
        .to_be_bytes::<32>()
        .into();
    provider
        .anvil_set_storage_at(USDT_MAINNET, usdt_slot_u, usdt_val)
        .await
        .expect("anvil_set_storage_at(USDT)");

    // Install STOP-only bytecode at the override address so its `balanceOf`
    // returns empty bytes → decode-fail path (the failure_isolation test
    // uses --decimals 18 to skip past the `query_decimals` step; mirror
    // that here so the failure fires on `token_balance` specifically,
    // not on `query_decimals`).
    provider
        .anvil_set_code(
            NOT_ERC20_ADDR,
            alloy_primitives::Bytes::from_static(STOP_BYTECODE),
        )
        .await
        .expect("anvil_set_code(STOP-only bytecode)");

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
            "--all",
            "--json",
            "--token",
            &format!("{NOT_ERC20_ADDR:#x}"),
            "--decimals",
            "18",
            "--network",
            "mainnet",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // AC #4: at least one registry token succeeded → exit 0.
    assert_eq!(
        out.status.code(),
        Some(0),
        "wallet balance --all --json must exit 0 when at least one token succeeded\nstdout: {stdout}\nstderr: {stderr}",
    );
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("stdout is not valid JSON: {e}\nstdout: {stdout}\nstderr: {stderr}")
    });
    let rows = parsed
        .as_array()
        .unwrap_or_else(|| panic!("top-level must be a JSON array: {parsed:?}"));
    assert!(
        !rows.is_empty(),
        "JSON array must contain at least one row, got: {rows:?}"
    );

    // Find the USDT success row.
    let usdt_row = rows
        .iter()
        .find(|r| r["symbol"] == "USDT")
        .expect("USDT row must be in JSON output");
    let usdt_obj = usdt_row.as_object().expect("USDT row must be an object");
    for key in ["symbol", "address", "balance", "decimals"] {
        assert!(
            usdt_obj.contains_key(key),
            "USDT success row missing {key:?}: {usdt_row:?}"
        );
    }
    assert!(
        !usdt_obj.contains_key("error"),
        "USDT success row must NOT carry `error`: {usdt_row:?}"
    );
    assert!(
        !usdt_obj.contains_key("context"),
        "USDT success row must NOT carry `context`: {usdt_row:?}"
    );

    // Find the override failure row (matched by EIP-55 address — same
    // display format as the success branch's `{}` formatter).
    let override_addr_str = format!("{NOT_ERC20_ADDR}");
    let fail_row = rows
        .iter()
        .find(|r| r["address"] == override_addr_str)
        .unwrap_or_else(|| {
            panic!("override row with address {override_addr_str} missing: {rows:?}")
        });
    let fail_obj = fail_row.as_object().expect("failure row must be an object");
    for key in ["symbol", "address", "error", "context"] {
        assert!(
            fail_obj.contains_key(key),
            "override failure row missing {key:?}: {fail_row:?}"
        );
    }
    assert!(
        !fail_obj.contains_key("balance"),
        "override failure row must NOT carry `balance`: {fail_row:?}"
    );
    assert!(
        !fail_obj.contains_key("decimals"),
        "override failure row must NOT carry `decimals`: {fail_row:?}"
    );
    assert_eq!(
        fail_obj["context"], "balance",
        "override failure row context must be `balance` (token_balance call fired): {fail_row:?}"
    );
    // Version-tolerant: `format!("{e}")` for Error::AbiDecodeFailed renders
    // `"ABI decode failed for <context>: <reason>"` where `<context>` is the
    // call selector (`balanceOf`, `decimals`, etc). We assert the
    // case-insensitive `balanceof` substring so the lock survives wording
    // changes in the prefix/reason fields — proves `token_balance` (not
    // `query_decimals`) fired. We deliberately do NOT assert the contract
    // address appears: the error context is the function selector, not the
    // callee, so the address is in the row's `address` field, not the
    // `error` field.
    let err = fail_obj["error"].as_str().expect("error must be a string");
    assert!(
        !err.is_empty(),
        "override failure row error must be non-empty: {fail_row:?}"
    );
    assert!(
        err.to_ascii_lowercase().contains("balanceof"),
        "override failure row error should mention the failed selector (balanceOf), got: {err:?}"
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn wallet_balance_all_decimals_override_applies_to_every_token() {
    // Issue #358 AC #6 — `--decimals <N>` overrides the per-token
    // `decimals()` auto-detect and applies to EVERY token in the batch.
    // We pre-fund USDT (6 decimals in registry) and assert that running
    // `--all --decimals 2` formats USDT at 2-decimal scale (1e9 raw
    // becomes 100,000,000) — proving the override wins over the
    // registry cache. If the override propagated to only one token,
    // other tokens in the registry would print at their cached scale
    // and the test would fail.
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new()
        .fork(MAINNET_RPC_URL)
        .chain_id(1)
        .spawn();
    let endpoint = anvil.endpoint();

    let rpc_url = alloy_transport_http::reqwest::Url::parse(&endpoint).expect("anvil url");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(rpc_url);

    let alpha_addr: alloy_primitives::Address = "f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .expect("alpha addr");

    // Pre-fund USDT at slot 2 with 1000 USDT (raw 1e9).
    let usdt_slot = alloy_primitives::keccak256(
        (
            alpha_addr,
            alloy_primitives::U256::from(USDT_BALANCEOF_SLOT),
        )
            .abi_encode(),
    );
    let usdt_slot_u: alloy_primitives::U256 = usdt_slot.into();
    let usdt_val: alloy_primitives::B256 = alloy_primitives::U256::from(USDT_ALPHA_INITIAL_RAW)
        .to_be_bytes::<32>()
        .into();
    provider
        .anvil_set_storage_at(USDT_MAINNET, usdt_slot_u, usdt_val)
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
            "--all",
            "--decimals",
            "2", // forces 2-decimal scale across the whole batch
            "--network",
            "mainnet",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "wallet balance --all --decimals 2 must succeed against forked Anvil\nstdout: {stdout}\nstderr: {stderr}",
    );
    // USDT pre-funded 1000 USDT (raw 1e9). With --decimals 2, formatted
    // value = 1e9 / 10^2 = 1e7 = 10,000,000 (8 digits). Verifies the
    // override wins over the registry's cached 6. (Previous assertion
    // was off-by-10x at 9 digits = 1e8.)
    assert!(
        stdout.contains("USDT 10000000"),
        "expected USDT scaled by --decimals 2 (raw 1e9 / 10^2 = 1e7 = 10,000,000), got: {stdout}",
    );
    // DAI (18 decimals in registry) without override would print
    // "DAI 1000" (raw 1e21, 18 decimals). With --decimals 2, raw 1e21
    // / 10^2 = 1e19, formatted = "10000000000000000000" (19 zeros).
    // DAI has no pre-fund so balance = 0; assert it printed (proves
    // the override propagated to DAI's row, not just USDT).
    assert!(
        stdout.contains("DAI "),
        "expected DAI line in stdout (override must propagate to every registry token), got: {stdout}",
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

/// Public Sepolia RPC source for `Anvil::new().fork(...)` — used by the
/// Issue #360 L29 USDC test. Defaults to a fork-capable public gateway.
/// Override at compile time with `SEPOLIA_RPC_URL=https://...` if the
/// default flakes. Same env-fallback convention as `MAINNET_RPC_URL`
/// above (so local `cargo test` and CI use the same gateway).
const SEPOLIA_RPC_URL: &str = match std::option_env!("SEPOLIA_RPC_URL") {
    Some(url) => url,
    None => "https://ethereum-sepolia-rpc.publicnode.com",
};

/// Sepolia Circle USDC contract (matches `tokens/sepolia.json`).
/// Stable for the lifetime of Sepolia — pinned in the registry file
/// (issue #360 AC #4).
const USDC_SEPOLIA: alloy_primitives::Address =
    alloy_primitives::address!("1c7D4B196Cb0C7B01d743Fbc6116a902379C7238");

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn wallet_balance_token_usdc_on_sepolia_prints_symbol() {
    // Issue #360 acceptance bullet 5: against live Sepolia, `eth wallet
    // balance --token <USDC_SEPOLIA> --network sepolia` prints the
    // human-readable symbol instead of the contract address.
    //
    // RED (pre-#360): stderr path prints `<scaled> <token-addr>`; the
    // output line contains the address, NOT the symbol. GREEN: registry
    // short-circuit hits on sepolia.json → output line begins with `USDC `
    // and never contains the contract address.
    //
    // We fork Sepolia via Anvil rather than hit Sepolia directly so the
    // test stays deterministic and CI-friendly (matches the L29 pattern
    // for the mainnet-fork tests above). The registry short-circuit is
    // chain_id-driven, so a fork at chain_id 11155111 still hits the
    // Sepolia bundle.
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new()
        .fork(SEPOLIA_RPC_URL)
        .chain_id(11155111)
        .spawn();
    let endpoint = anvil.endpoint();

    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    // Use Anvil dev account #0 as the holder — even a 0-balance address
    // exercises the registry short-circuit + format path. The point is
    // the LABEL, not the magnitude (issue #360 AC #5 confirms the
    // expected output shape, not a specific pre-fund amount).
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
            // `USDC_SEPOLIA` is the canonical Sepolia USDC address — also
            // pinned in `tokens/sepolia.json`. Using the constant here
            // makes drift between the registry and the test impossible.
            &format!("{USDC_SEPOLIA:#x}"),
            "--network",
            "sepolia",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "balance --token USDC must succeed against forked Sepolia\nstdout: {stdout}\nstderr: {stderr}",
    );
    // Issue #360 AC #3: `eth wallet balance --token <ADDR>` output
    // becomes `<symbol> <scaled>` (e.g. `USDC 15.000000`). The
    // registry short-circuit ensures the symbol is `USDC`, not the
    // raw address.
    assert!(
        stdout.starts_with("USDC ") || stdout.contains(" USDC "),
        "expected output to begin with the USDC symbol, got: {stdout}",
    );
    // Lock-down: the contract address must NOT appear in the output
    // line. The registry short-circuit is the whole point — a regression
    // that drops the label back to the address (pre-#360 behavior) would
    // surface here.
    assert!(
        !stdout.contains("0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238"), // USDC_SEPOLIA
        "output must NOT contain the USDC contract address when registry hits, got: {stdout}",
    );
    // Lock-down: must contain a numeric balance (whole or `whole.frac`).
    // Tolerant of zero balance — the format is the contract, not the
    // magnitude.
    assert!(
        stdout.contains('0') || stdout.contains('1') || stdout.contains('2'),
        "expected numeric balance in stdout, got: {stdout}",
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

// ---------------------------------------------------------------------------
// Issue #357 box 7 (deferred follow-up): 2 lock-down tests for the
// `Error::AbiDecodeFailed` exit-2 path on ERC-20 decode failures.
//
// Strategy: install STOP-only bytecode (`0x00`) at a deterministic
// non-ERC-20 address via `anvil_set_code`. The EVM responds to
// `eth_call balanceOf`/`decimals` with `0x` (empty bytes). The
// `*Call::abi_decode_returns(&[])` calls fail with buffer-too-short —
// the new wrap sites (PR #361, commit `a0d59f6`) return
// `Error::AbiDecodeFailed { context: "balanceOf" | "decimals", reason }`,
// and `exit_code()` maps to 2. Distinguishing decode-fail (exit 2) from
// RPC-fail (exit 3) is the whole point of #357 — operators scripting
// around exit codes can now tell the two failure modes apart.
//
// These tests stay `#[ignore]` per L29 (operator-driven). Box 7 flips
// `[x]` after an operator runs `RUN_ANVIL_E2E=1 cargo test -p eth
// --test cli_localnet -- --ignored` and confirms the tests pass.
// ---------------------------------------------------------------------------

/// Deterministic non-ERC-20 address. STOP-only bytecode installed via
/// `anvil_set_code(0x...beef, 0x00)` so `eth_call` returns `0x` (empty
/// bytes). Used by both `wallet_balance_token_non_erc20_*` tests.
const NOT_ERC20_ADDR: alloy_primitives::Address =
    alloy_primitives::address!("000000000000000000000000000000000000beef");

/// STOP-only bytecode (single `0x00` opcode). `eth_call` to this contract
/// returns success with `0x` output — short enough that
/// `*Call::abi_decode_returns(&[])` fails for `uint256` / `uint8` returns.
const STOP_BYTECODE: &[u8] = &[0x00];

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn wallet_balance_token_non_erc20_with_decimals_override_yields_exit_2() {
    // Issue #357 box 7 lock-down: `Error::AbiDecodeFailed { context: "balanceOf" }`.
    // RED: with `--decimals 18` override, `query_decimals` is skipped, so
    // the only decode path exercised is `balanceOfCall::abi_decode_returns`
    // in `erc20::token_balance`. If `0x...beef` had valid ERC-20 bytecode
    // the decode would succeed (exit 0). STOP-only bytecode forces empty
    // `eth_call` response → decode fails → `Error::AbiDecodeFailed` → exit 2.
    //
    // GREEN: exit code = 2 (NOT 3 — distinguishes decode-fail from RPC-fail)
    // + stderr contains the "balanceOf" context string + does NOT contain
    // the `Error::Rpc` "rpc:" prefix (lock-down for "decode-fail path
    // exercised, not RPC-fail path").
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new().chain_id(31337).spawn();
    let endpoint = anvil.endpoint();

    let rpc_url = alloy_transport_http::reqwest::Url::parse(&endpoint).expect("anvil url");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(rpc_url);

    provider
        .anvil_set_code(
            NOT_ERC20_ADDR,
            alloy_primitives::Bytes::from_static(STOP_BYTECODE),
        )
        .await
        .expect("anvil_set_code(STOP-only bytecode at 0x...beef)");

    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    // --decimals 18 override skips `query_decimals`; only the `token_balance`
    // `balanceOf` decode-fail path is exercised.
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
            "0x000000000000000000000000000000000000beef",
            "--decimals",
            "18",
            "--network",
            "anvil",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(2),
        "non-ERC-20 token (decode-fail on balanceOf) must yield bad-input exit code (2), NOT RPC exit 3\nstdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        stderr.contains("balanceOf"),
        "stderr should mention the balanceOf decode context:\nstderr: {stderr}",
    );
    // Lock-down: confirm decode-fail path (Error::AbiDecodeFailed) fired,
    // not RPC-fail path (Error::Rpc carries "rpc:" prefix per its
    // `#[error("rpc: {0}")]` Display impl).
    assert!(
        !stderr.contains("rpc:"),
        "stderr must not contain Error::Rpc prefix — the decode-fail path should fire, not RPC transport-fail:\nstderr: {stderr}",
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn wallet_balance_token_non_erc20_auto_detect_yields_exit_2() {
    // Issue #357 box 7 lock-down: `Error::AbiDecodeFailed { context: "decimals" }`.
    // RED: no `--decimals` override, so `query_decimals` runs first (auto-detect).
    // STOP-only bytecode forces empty `eth_call decimals` response → decode fails
    // → `Error::AbiDecodeFailed { context: "decimals", reason }` → exit 2.
    //
    // GREEN: exit code = 2 + stderr contains "decimals" context string + does
    // NOT contain the `Error::Rpc` "rpc:" prefix.
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new().chain_id(31337).spawn();
    let endpoint = anvil.endpoint();

    let rpc_url = alloy_transport_http::reqwest::Url::parse(&endpoint).expect("anvil url");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(rpc_url);

    provider
        .anvil_set_code(
            NOT_ERC20_ADDR,
            alloy_primitives::Bytes::from_static(STOP_BYTECODE),
        )
        .await
        .expect("anvil_set_code(STOP-only bytecode at 0x...beef)");

    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    // No --decimals override → `query_decimals` runs first; this test
    // exercises the `decimalsCall::abi_decode_returns` decode-fail path.
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
            "0x000000000000000000000000000000000000beef",
            "--network",
            "anvil",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(2),
        "non-ERC-20 token decode-fail (auto-detect path on decimals) must yield exit 2\nstdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        stderr.contains("decimals"),
        "stderr should mention the decimals decode context:\nstderr: {stderr}",
    );
    assert!(
        !stderr.contains("rpc:"),
        "stderr must not contain Error::Rpc prefix:\nstderr: {stderr}",
    );
}

// ---------------------------------------------------------------------------
// Issue #354 — dynamic gas estimation (M-3 from #352 code-review).
//
// Override precedence: CLI flag / env var > provider.estimate_eip1559_fees().
// Partial override (only one of max-fee / max-prio set) → exit 2 (InvalidInput)
// per #297 M11. Invalid wei value (not parseable as u128) → exit 2 (clap parse
// error). The override path must wire through to the signed envelope — Anvil
// test queries the broadcast tx back via alloy and asserts max_fee_per_gas
// matches the CLI override.
// ---------------------------------------------------------------------------

#[test]
fn send_command_with_invalid_max_fee_wei_yields_exit_2() {
    // L28 Gate C: invalid override rejected at clap parse (exit 2).
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let import = run_eth(
        &data_dir,
        &[
            "wallet",
            "import",
            "--name",
            "alpha",
            "--mnemonic",
            phrase,
            "--password",
            "test-password-1",
            "--network",
            "anvil",
        ],
    );
    assert_success(&import, "alpha");

    let out = run_eth(
        &data_dir,
        &[
            "send",
            "--name",
            "alpha",
            "--password",
            "test-password-1",
            "--network",
            "anvil",
            "--to",
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            "--amount",
            "1",
            "--max-fee-per-gas",
            "not-a-wei-value",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(2),
        "invalid --max-fee-per-gas value must yield exit 2\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("invalid") || stderr.contains("parse"),
        "stderr should mention invalid/parse error from clap u128 parse:\nstderr: {stderr}"
    );
}

#[test]
fn send_command_with_only_max_fee_per_gas_yields_exit_2() {
    // L28 Gate C: partial override (one of two) rejected as InvalidInput (exit 2).
    // EIP-1559 requires BOTH max-fee-per-gas + max-priority-fee-per-gas OR NEITHER.
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let import = run_eth(
        &data_dir,
        &[
            "wallet",
            "import",
            "--name",
            "alpha",
            "--mnemonic",
            phrase,
            "--password",
            "test-password-1",
            "--network",
            "anvil",
        ],
    );
    assert_success(&import, "alpha");

    let out = run_eth(
        &data_dir,
        &[
            "send",
            "--name",
            "alpha",
            "--password",
            "test-password-1",
            "--network",
            "anvil",
            "--to",
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            "--amount",
            "1",
            "--max-fee-per-gas",
            "99000000000",
            // Missing --max-priority-fee-per-gas — must be rejected as InvalidInput.
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(2),
        "partial override (only --max-fee-per-gas) must yield exit 2\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("either set both --max-fee-per-gas and --max-priority-fee-per-gas")
            || stderr.contains("omit both to use the network fee estimate"),
        "stderr should mention the InvalidInput override-precedence message:\nstderr: {stderr}"
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn send_command_with_max_fee_overrides_uses_overridden_gas() {
    // Proves the CLI override path wires through to the signed envelope:
    // broadcast with --max-fee-per-gas 99000000000 + --max-priority-fee-per-gas
    // 2000000000, query the tx back via alloy, assert the gas fields match.
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new().spawn();
    let endpoint = anvil.endpoint();
    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

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
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            "--amount",
            "1",
            "--max-fee-per-gas",
            "99000000000",
            "--max-priority-fee-per-gas",
            "2000000000",
        ],
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "send with overrides must succeed against Anvil\nstdout: {stdout}\nstderr: {stderr}"
    );

    // Pull tx hash from stdout (handler prints the hash as the last token).
    let tx_hash_str = stdout
        .split_whitespace()
        .find(|tok| tok.starts_with("0x") && tok.len() == 66)
        .unwrap_or_else(|| panic!("no tx hash in stdout: {stdout}"));

    // Query the broadcast tx back via alloy to confirm override wired through.
    use alloy_provider::Provider;
    let provider = eth_wallet_core::new_http(endpoint.parse().expect("parse anvil endpoint url"))
        .expect("provider");
    let tx_hash: alloy_primitives::B256 = tx_hash_str.parse().expect("parse tx hash");
    let tx = provider
        .get_transaction_by_hash(tx_hash)
        .await
        .expect("get tx")
        .expect("tx must exist");

    let eip1559 = tx
        .inner
        .as_eip1559()
        .expect("tx must be EIP-1559 (sent via sign_native_eth_tx)");
    assert_eq!(
        eip1559.tx().max_fee_per_gas,
        99_000_000_000u128,
        "override max_fee_per_gas must reach the signed envelope"
    );
    assert_eq!(
        eip1559.tx().max_priority_fee_per_gas,
        2_000_000_000u128,
        "override max_priority_fee_per_gas must reach the signed envelope"
    );
}
