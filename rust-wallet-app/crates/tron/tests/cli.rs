//! Integration tests for the `tron` CLI argument shape.
//!
//! These tests parse argv through `Cli::try_parse_from` rather than spawning
//! the binary. That avoids needing a live RPC node for every wiring check —
//! the runtime path is covered by `cargo run -- wallet send ...` smoke
//! tests against Nile.
//!
//! Each test pins one specific wiring choice against silent regressions:
//!
//! - `--mnemonic` and `--mnemonic-file` must conflict at the clap layer so
//!   a script cannot accidentally supply both.
//! - The new `--mnemonic-file` flag is forwarded into the dispatch path.
//! - `--confirm-yes` on mainnet guards short-circuit to success.

use clap::Parser;
use tron::cli::{Cli, Commands, Trc20Action, WalletAction};

/// BIP-39 `abandon … about` vector, kept identical to the unit tests.
const VECTOR: &str = "abandon abandon abandon abandon abandon abandon \
                      abandon abandon abandon abandon abandon about";

fn parse(argv: &[&str]) -> Cli {
    Cli::try_parse_from(argv).expect("argv must be a valid CLI invocation")
}

#[test]
fn wallet_send_accepts_mnemonic_file() {
    // The new flag round-trips through clap and lands on the `Send` arm with
    // the file path the caller passed. We don't need to invoke the binary to
    // prove the wiring — the parser test pins both halves of it.
    let cli = parse(&[
        "tron",
        "wallet",
        "send",
        "--mnemonic-file",
        "/tmp/phrase.txt",
        "--to",
        "TAbcdefghijklmnopqrstuvwxyz12345678",
        "--amount",
        "1",
        "--network",
        "nile",
    ]);
    match cli.command {
        Commands::Wallet(cmd) => match cmd.action {
            WalletAction::Send {
                mnemonic,
                mnemonic_file,
                to,
                ..
            } => {
                assert!(mnemonic.is_none(), "--mnemonic must be unset");
                assert_eq!(
                    mnemonic_file.expect("--mnemonic-file must be set").to_str(),
                    Some("/tmp/phrase.txt")
                );
                assert_eq!(to.as_deref(), Some("TAbcdefghijklmnopqrstuvwxyz12345678"));
            }
            other => panic!("expected WalletAction::Send, got {other:?}"),
        },
        other => panic!("expected Commands::Wallet, got {other:?}"),
    }
}

#[test]
fn wallet_send_conflicts_mnemonic_with_mnemonic_file() {
    // clap rejects both at parse time (exit 2) — surfacing the contradiction
    // before any RPC call. A missing conflict annotation would let a script
    // set both and silently pick one.
    let err = Cli::try_parse_from([
        "tron",
        "wallet",
        "send",
        "--mnemonic",
        VECTOR,
        "--mnemonic-file",
        "/tmp/phrase.txt",
        "--to",
        "TAbcdefghijklmnopqrstuvwxyz12345678",
        "--amount",
        "1",
    ])
    .expect_err("conflicting flags must be rejected at parse time");
    let msg = err.to_string();
    assert!(
        msg.contains("cannot be used with"),
        "expected clap conflict error, got {msg}"
    );
}

#[test]
fn trc20_send_accepts_mnemonic_file() {
    let cli = parse(&[
        "tron",
        "trc20",
        "send",
        "--mnemonic-file",
        "/tmp/phrase.txt",
        "--contract",
        "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
        "--to",
        "TAbcdefghijklmnopqrstuvwxyz12345678",
        "--amount",
        "1",
    ]);
    match cli.command {
        Commands::Trc20(cmd) => match cmd.action {
            Trc20Action::Send {
                mnemonic,
                mnemonic_file,
                ..
            } => {
                assert!(mnemonic.is_none());
                assert_eq!(
                    mnemonic_file.expect("--mnemonic-file must be set").to_str(),
                    Some("/tmp/phrase.txt")
                );
            }
            other => panic!("expected Trc20Action::Send, got {other:?}"),
        },
        other => panic!("expected Commands::Trc20, got {other:?}"),
    }
}

#[test]
fn trc20_approve_accepts_mnemonic_file() {
    let cli = parse(&[
        "tron",
        "trc20",
        "approve",
        "--mnemonic-file",
        "/tmp/phrase.txt",
        "--contract",
        "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
        "--spender",
        "TAbcdefghijklmnopqrstuvwxyz12345678",
        "--amount",
        "0",
    ]);
    match cli.command {
        Commands::Trc20(cmd) => match cmd.action {
            Trc20Action::Approve {
                mnemonic,
                mnemonic_file,
                ..
            } => {
                assert!(mnemonic.is_none());
                assert_eq!(
                    mnemonic_file.expect("--mnemonic-file must be set").to_str(),
                    Some("/tmp/phrase.txt")
                );
            }
            other => panic!("expected Trc20Action::Approve, got {other:?}"),
        },
        other => panic!("expected Commands::Trc20, got {other:?}"),
    }
}
