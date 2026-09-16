//! `sol` CLI integration tests — address command surface.
//!
//! Phase 7 verification — `address new` and `address pubkey` clap-level rejects.
//! `P7-16`: `--mnemonic` (inline) is rejected at clap level on both subcommands.
//! Surfpool-gated derivation e2e lives in `cli_integration_surfpool.rs` row 11/12.

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

fn sol_bin() -> Command {
    Command::cargo_bin("sol").expect("sol binary not built — run `cargo build -p sol` first")
}

#[test]
fn address_new_requires_wallet_id_or_mnemonic_file() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("address")
        .arg("new")
        .assert()
        .failure()
        .stderr(contains("specify one of"));
}

#[test]
fn address_pubkey_requires_wallet_id() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("address")
        .arg("pubkey")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn address_new_does_not_accept_inline_mnemonic_p7_16() {
    // P7-16: `address new --mnemonic "<words>"` MUST be rejected at clap
    // level (flag does not exist on this subcommand). Mirrors the
    // `wallet import --mnemonic` P7-1 audit gate.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("address")
        .arg("new")
        .arg("--mnemonic")
        .arg("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn address_pubkey_does_not_accept_inline_mnemonic_p7_16() {
    // P7-16: `address pubkey --mnemonic` MUST be rejected at clap level too
    // (flag does not exist). Pubkey is documented as the --wallet-id short-form.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("address")
        .arg("pubkey")
        .arg("--mnemonic")
        .arg("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn address_new_with_both_wallet_id_and_mnemonic_file_rejects() {
    // Handler guard (handlers/address.rs line ~52): specifying both modes is
    // ambiguous → fail with handler message (NOT clap exit 2).
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("address")
        .arg("new")
        .arg("--wallet-id")
        .arg("00000000-0000-0000-0000-000000000000")
        .arg("--mnemonic-file")
        .arg("/dev/null")
        .assert()
        .failure()
        .stderr(contains("specify only one"));
}

#[test]
fn address_new_with_invalid_uuid_rejects() {
    // `parse_wallet_id_pub` rejects malformed UUID strings before any
    // summary lookup. Non-clap exit (any non-zero acceptable).
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("address")
        .arg("new")
        .arg("--wallet-id")
        .arg("not-a-uuid")
        .assert()
        .failure()
        .stderr(contains("invalid"));
}

#[test]
fn address_pubkey_with_invalid_uuid_rejects() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("address")
        .arg("pubkey")
        .arg("--wallet-id")
        .arg("not-a-uuid")
        .assert()
        .failure()
        .stderr(contains("invalid"));
}
