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

/// Resolve the path to the `eth` binary under test. Cargo provides this via
/// the `CARGO_BIN_EXE_<name>` env var for integration tests.
fn eth_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_eth"))
}

/// Run the `eth` binary with `ETH_DATA_DIR` pointed at `data_dir` (so the
/// wallet store is isolated). Captures stdout + stderr + exit status.
fn run_eth(data_dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(eth_bin())
        .env("ETH_DATA_DIR", data_dir)
        .env("NO_COLOR", "1")
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
