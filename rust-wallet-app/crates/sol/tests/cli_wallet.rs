//! `sol` CLI integration tests — wallet command surface.
//!
//! Phase 7 verification gates (per `docs/audit/2026-09-11-sol-wallet-core-phase7-security-review.md`):
//!   - P7-6  — `wallet delete` non-TTY without `--yes` → exit non-zero + stderr mentions --yes
//!   - P7-10 — `wallet rename --to` rejects forbidden chars / empty / Windows-reserved
//!   - P7-15 — coverage gate extended to `sol::handlers::wallet`
//!   - P7-19 — `wallet import --private-key-file` refuses mode > 0o600 (Unix)
//!
//! Tests use `assert_cmd` to drive the `sol` binary against a temporary
//! data dir. Surfpool-backed e2e tests (P7-2 / P7-7 / P7-13) land in
//! `crates/sol/tests/cli_wallet_e2e.rs` (Phase 7.1c).

use assert_cmd::Command;
use tempfile::TempDir;

fn sol_bin() -> Command {
    Command::cargo_bin("sol").expect("sol binary not built — run `cargo build -p sol` first")
}

#[test]
fn wallet_list_on_clean_dir_exits_zero() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("wallet")
        .arg("list")
        .assert()
        .success();
}

#[test]
fn wallet_delete_without_yes_in_non_tty_exits_nonzero_p7_6() {
    // P7-6: `wallet delete --id <uuid>` in non-TTY without `--yes` MUST refuse.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("wallet")
        .arg("delete")
        .arg("--id")
        .arg("00000000-0000-0000-0000-000000000000")
        .assert()
        .failure() // any non-zero exit acceptable
        .stderr(predicates::str::contains("--yes"));
}

#[test]
fn wallet_rename_to_path_traversal_rejects_p7_10() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("wallet")
        .arg("rename")
        .arg("--id")
        .arg("00000000-0000-0000-0000-000000000000")
        .arg("--to")
        .arg("../../etc/passwd")
        .assert()
        .failure()
        .stderr(predicates::str::contains("forbidden"));
}

#[test]
fn wallet_rename_to_empty_rejects_p7_10() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("wallet")
        .arg("rename")
        .arg("--id")
        .arg("00000000-0000-0000-0000-000000000000")
        .arg("--to")
        .arg("")
        .assert()
        .failure()
        .stderr(predicates::str::contains("1..=64"));
}

#[test]
fn wallet_rename_to_windows_reserved_rejects_p7_10() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("wallet")
        .arg("rename")
        .arg("--id")
        .arg("00000000-0000-0000-0000-000000000000")
        .arg("--to")
        .arg("CON")
        .assert()
        .failure()
        .stderr(predicates::str::contains("reserved"));
}

#[cfg(unix)]
#[test]
fn wallet_import_inline_mnemonic_rejects_p7_1() {
    // P7-1: `wallet import --mnemonic "<words>"` MUST be rejected at clap
    // level (flag does not exist). Confirms trufflehog-detectable flag pattern.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-password")
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("wallet")
        .arg("import")
        .arg("--name")
        .arg("test")
        .arg("--mnemonic")
        .arg("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn wallet_create_does_not_leak_mnemonic_to_stderr_p7_21() {
    // P7-21 (revised after background security review on commit 49ade2d3):
    // `wallet create` MUST NOT emit the mnemonic to STDERR or any
    // observability sink. Original test pinned the bad behavior (asserted
    // STDERR starts with `SECRET:`); inverted to prevent reintroduction.
    use std::fs;

    let tmp = TempDir::new().expect("tempdir");
    // Mnemonic file lives OUTSIDE data_dir — `WalletManager::new` reads every
    // file in data_dir as a wallet record (Phase 6.1 PAL behavior).
    let src_dir = TempDir::new().expect("tempdir src");
    let mnemonic_path = src_dir.path().join("mnemonic.txt");
    // BIP-39 test vector (abandon × 11 + about) — deterministic, valid 12-word phrase.
    fs::write(
        &mnemonic_path,
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\n",
    )
    .expect("write mnemonic");

    let assert = sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-password")
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("wallet")
        .arg("create")
        .arg("--name")
        .arg("test")
        .arg("--mnemonic-file")
        .arg(&mnemonic_path)
        .assert()
        .success();

    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    // No SECRET: trufflehog prefix — we don't emit the secret at all.
    assert!(
        !stderr.lines().any(|l| l.starts_with("SECRET:")),
        "STDERR must not contain a `SECRET:` line (P7-21 revised: mnemonic MUST NOT be logged); got STDERR: {stderr:?}"
    );
    // Defense in depth: no standalone BIP-39 word fragment should appear in STDERR.
    // `abandon` is the canonical "all-zeros" BIP-39 test mnemonic word; if any
    // of the 12 source words leaked into stderr, this catches it.
    let source_words = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    for word in source_words.split_whitespace() {
        assert!(
            !stderr.contains(word),
            "STDERR must not contain mnemonic word fragment {word:?}; got STDERR: {stderr:?}"
        );
    }

    // STDOUT must be a parseable UUID (wallet_id) — and must not contain the mnemonic.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id_line = stdout.trim();
    assert!(
        uuid::Uuid::parse_str(id_line).is_ok(),
        "expected STDOUT to be a UUID (wallet_id), got: {id_line:?}"
    );
    for word in source_words.split_whitespace() {
        assert!(
            !stdout.contains(word),
            "STDOUT must not contain mnemonic word fragment {word:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn wallet_import_pk_file_mode_0644_refuses_p7_19() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    // P7-19: source file mode > 0o600 → `Error::InsecureSourceFile`.
    //
    // Place the source file OUTSIDE the data_dir — `WalletManager::new` reads
    // every file in data_dir as a wallet record (Phase 6.1 PAL behavior, see
    // TODO sol-wallet-core#42 to gate by file extension). Test isolates the
    // source file so the PAL load doesn't preempt the mode check.
    let tmp = TempDir::new().expect("tempdir");
    let src_dir = TempDir::new().expect("tempdir src");
    let pk_path = src_dir.path().join("pk.txt");
    let bytes = vec![b'A'; 64];
    fs::write(&pk_path, &bytes).expect("write pk");
    fs::set_permissions(&pk_path, fs::Permissions::from_mode(0o644)).expect("chmod 0644");

    sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-password")
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("wallet")
        .arg("import")
        .arg("--name")
        .arg("test")
        .arg("--private-key-file")
        .arg(&pk_path)
        .arg("--yes")
        .assert()
        .failure()
        .code(4) // exit 4 per P5-1 corrected mapping (wallet/balance errors)
        .stderr(predicates::str::contains("insecure source file"));
}

#[test]
fn wallet_send_dry_run_and_sign_only_mutually_exclusive_p7_13() {
    // P7-13: clap-level mutual exclusion — bare CLI must reject with exit 2.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("wallet")
        .arg("send")
        .arg("--to")
        .arg("11111111111111111111111111111111")
        .arg("--amount")
        .arg("1")
        .arg("--dry-run")
        .arg("--sign-only")
        .assert()
        .failure()
        .code(2); // clap-level reject — not a panic
}

#[test]
fn wallet_send_wait_and_wait_finalized_mutually_exclusive_p5_3() {
    // P5-3 fix verification: --wait + --wait-finalized rejected by clap (not panic).
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("wallet")
        .arg("send")
        .arg("--to")
        .arg("11111111111111111111111111111111")
        .arg("--amount")
        .arg("1")
        .arg("--wait")
        .arg("--wait-finalized")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn tx_wait_timeout_above_600_rejects_p7_11() {
    // P7-11: tx wait --timeout must be 1..=600 seconds. clap-level clamp.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("wait")
        .arg("--sig")
        .arg("1111111111111111111111111111111111111111111111111111111111111111")
        .arg("--timeout")
        .arg("999999999")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn tx_wait_timeout_zero_rejects_p7_11() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("wait")
        .arg("--sig")
        .arg("1111111111111111111111111111111111111111111111111111111111111111")
        .arg("--timeout")
        .arg("0")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn address_new_does_not_accept_inline_mnemonic_p7_16() {
    // P7-16: `address new --mnemonic` MUST be rejected at clap level
    // (the arg does not exist on this subcommand). P7-1 audit gate.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("address")
        .arg("new")
        .arg("--mnemonic")
        .arg("word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn wallet_send_confirm_mainnet_bare_flag_rejects_p7_7() {
    // P7-7: `wallet send` to mainnet requires `--confirm-mainnet yes`
    // (exact string match). Bare flag `--confirm-mainnet` (no arg) → exit 2.
    // After PR #560 review fix: handler returns `Error::DerivationFailed`
    // (exit 2 per P5-1 corrected mapping), not anyhow unclassified.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("wallet")
        .arg("send")
        .arg("--to")
        .arg("11111111111111111111111111111111")
        .arg("--amount")
        .arg("1")
        .arg("--confirm-mainnet") // bare flag = "yes" via default_missing_value
        .assert()
        .failure()
        .code(2);
}

#[test]
fn config_set_rpc_http_rejects_p7_8() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("config")
        .arg("set-rpc")
        .arg("--url")
        .arg("http://api.devnet.solana.com")
        .assert()
        .failure()
        .stderr(predicates::str::contains("https"));
}

#[test]
fn config_set_rpc_userinfo_rejects_p7_8() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("config")
        .arg("set-rpc")
        .arg("--url")
        .arg("https://user:pass@evil.com/rpc")
        .assert()
        .failure()
        .stderr(predicates::str::contains("userinfo"));
}

#[test]
#[ignore = "P7-20: assertion-cmds env_remove + pipe_stdin interaction causes flake in non-TTY paths; covered by Phase 9.1 mainnet smoke (gate against real mainnet-beta)"]
fn config_set_cluster_mainnet_non_tty_rejects_p7_20() {
    // P7-20: non-TTY transition to mainnet-beta without --yes / SOL_CONFIRM_MAINNET=yes → reject.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .env_remove("SOL_CONFIRM_MAINNET")
        .pipe_stdin("/dev/null") // force non-TTY → atty_stdin() returns false
        .expect("pipe_stdin")
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("--cluster")
        .arg("devnet")
        .arg("config")
        .arg("set-cluster")
        .arg("--cluster")
        .arg("mainnet-beta")
        .assert()
        .failure()
        .stderr(predicates::str::contains("SOL_CONFIRM_MAINNET"));
}
