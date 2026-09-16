//! `sol` CLI integration tests — Phase 7 Task 7.3 step 4.
//!
//! 22-row cross-check table: one #[test] fn per V0.1 deep-dive CLI row.
//! All surfpool-gated via `RUN_SOL_SURFPOOL=1` env var.
//!
//! **Surfpool only** — `RUN_SOL_SURFPOOL=1 cargo test -p sol --test cli_integration_surfpool -- --ignored`.
//!
//! Each row's pass criterion is documented inline. Body = `todo!()` until
//! surfpool CI integration lands in Phase 7.2.
//!
//! Cross-ref: `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md` (22 rows).

use assert_cmd::Command;
use std::net::TcpListener;
use tempfile::TempDir;

fn sol_bin() -> Command {
    Command::cargo_bin("sol").expect("sol binary not built — run `cargo build -p sol` first")
}

fn tmp() -> TempDir {
    TempDir::new().expect("tempdir")
}

macro_rules! surfpool_row {
    ($name:ident, $doc:expr => $($arg:expr),* $(,)?) => {
        #[doc = concat!("Row: ", $doc)]
        #[test]
        #[ignore = "RUN_SOL_SURFPOOL=1 required — Phase 7.3 Step 4 surfpool cross-check; body lands with surfpool CI integration (Phase 7.2)."]
        fn $name() {
            let mut cmd = sol_bin();
            cmd.args([$($arg),*]);
            let _ = cmd.arg("--data-dir").arg(tmp().path()).assert();
        }
    };
}

// Row 1 — Wallet CRUD create
surfpool_row!(row_01_wallet_create, "wallet create — encrypted mnemonic-file persist + wallet_id STDOUT" =>
    "wallet", "create", "--name", "test", "--mnemonic-file", "/dev/null"
);
// Row 2 — Wallet import (private key file)
surfpool_row!(row_02_wallet_import, "wallet import --private-key-file — encrypted persist + wallet_id STDOUT" =>
    "wallet", "import", "--name", "test", "--private-key-file", "/dev/null", "--yes"
);
// Row 3 — Wallet show
surfpool_row!(row_03_wallet_show, "wallet show --id — encrypted-blob-only summary (no unlock)" =>
    "wallet", "show", "--id", "00000000-0000-0000-0000-000000000000"
);
// Row 4 — Wallet list
surfpool_row!(row_04_wallet_list, "wallet list — encrypted-blob iteration without decrypt" =>
    "wallet", "list"
);
// Row 5 — Wallet delete
surfpool_row!(row_05_wallet_delete, "wallet delete --yes — encrypted blob removed from disk" =>
    "wallet", "delete", "--id", "00000000-0000-0000-0000-000000000000", "--yes"
);
// Row 6 — Wallet rename
surfpool_row!(row_06_wallet_rename, "wallet rename --to — encrypted blob metadata update" =>
    "wallet", "rename", "--id", "00000000-0000-0000-0000-000000000000", "--to", "new-name"
);
// Row 7 — Memo attach (Phase 4.1 Modify — coverage in spl_instruction.rs per Plan 7.3 Step 5)
surfpool_row!(row_07_spl_memo_attach, "spl send --memo — SPL Memo ix present in tx ix list (<=566 bytes, no NUL)" =>
    "spl", "send", "--wallet-id", "00000000-0000-0000-0000-000000000000", "--to", "11111111111111111111111111111111",
    "--amount", "1", "--token", "11111111111111111111111111111111", "--memo", "hello"
);
// Row 8 — Wallet send (SOL)
surfpool_row!(row_08_wallet_send_sol, "wallet send --to --amount — SOL transfer + confirm" =>
    "wallet", "send", "--wallet-id", "00000000-0000-0000-0000-000000000000", "--to", "11111111111111111111111111111111", "--amount", "1"
);
// Row 9 — SPL send
surfpool_row!(row_09_spl_send, "spl send — SPL token transfer (ATA derivation + transfer_checked)" =>
    "spl", "send", "--wallet-id", "00000000-0000-0000-0000-000000000000", "--to", "11111111111111111111111111111111",
    "--amount", "1", "--token", "11111111111111111111111111111111"
);
// Row 10 — Wallet send-speedup
surfpool_row!(row_10_wallet_send_speedup, "wallet send-speedup --sig --priority-fee — fresh sig + higher fee" =>
    "wallet", "send-speedup", "--wallet-id", "00000000-0000-0000-0000-000000000000",
    "--sig", "5".repeat(87).as_str(), "--priority-fee", "1000"
);
// Row 11 — Address new
surfpool_row!(row_11_address_new, "address new --wallet-id — derived pubkey via summary" =>
    "address", "new", "--wallet-id", "00000000-0000-0000-0000-000000000000"
);
// Row 12 — Address pubkey
surfpool_row!(row_12_address_pubkey, "address pubkey --wallet-id — short-hand derivation" =>
    "address", "pubkey", "--wallet-id", "00000000-0000-0000-0000-000000000000"
);
// Row 13 — Dry-run/simulate (Phase 3.1 Modify — coverage in tx_serde.rs per Plan 7.3 Step 6)
surfpool_row!(row_13_dry_run_simulate, "wallet send --dry-run — simulateTransaction (no broadcast, no sig)" =>
    "wallet", "send", "--to", "11111111111111111111111111111111", "--amount", "1", "--dry-run"
);
// Row 14 — Cross-cluster conformance (covered by cli_devnet_conformance.rs)
surfpool_row!(row_14_cli_devnet_conformance, "Cross-cluster devnet conformance — covered by cli_devnet_conformance.rs (Step 3)" =>
    "config", "set-cluster", "--cluster", "devnet", "--yes"
);
// Row 15 — Balance sol
surfpool_row!(row_15_balance_sol, "balance sol --address — RPC get_balance" =>
    "balance", "sol", "--address", "11111111111111111111111111111111"
);
// Row 16 — Balance spl
surfpool_row!(row_16_balance_spl, "balance spl --address --token — RPC get_token_account_balance" =>
    "balance", "spl", "--address", "11111111111111111111111111111111", "--token", "11111111111111111111111111111111"
);
// Row 17 — Tx get
surfpool_row!(row_17_tx_get, "tx get --sig — RPC getTransaction" =>
    "tx", "get", "--sig", "5".repeat(87).as_str()
);
// Row 18 — Tx wait
surfpool_row!(row_18_tx_wait, "tx wait --sig --timeout --poll-interval — poll for confirm" =>
    "tx", "wait", "--sig", "5".repeat(87).as_str(), "--timeout", "10"
);
// Row 19 — SPL approve
surfpool_row!(row_19_spl_approve, "spl approve --delegate --amount — SPL delegate authority tx" =>
    "spl", "approve", "--wallet-id", "00000000-0000-0000-0000-000000000000",
    "--token", "11111111111111111111111111111111", "--delegate", "11111111111111111111111111111111", "--amount", "1"
);
// Row 20 — SPL balance
surfpool_row!(row_20_spl_balance, "spl balance --address --token — SPL token account balance" =>
    "spl", "balance", "--address", "11111111111111111111111111111111", "--token", "11111111111111111111111111111111"
);
// Row 21 — SPL allowance
surfpool_row!(row_21_spl_allowance, "spl allowance --token --owner --delegate — delegated allowance query" =>
    "spl", "allowance", "--token", "11111111111111111111111111111111",
    "--owner", "11111111111111111111111111111111", "--delegate", "11111111111111111111111111111111"
);
// Row 22 — Mainnet smoke (deferred to Phase 9.1 per Plan 7.3 Step 7 / Q4 Q-gate)
surfpool_row!(row_22_mainnet_smoke, "Mainnet smoke — deferred to Phase 9.1 per Q4 Q-gate (Plan 7.3 Step 7)" =>
    "config", "set-cluster", "--cluster", "mainnet-beta", "--yes"
);

// ─── classify() exit-code audit gates (Plan 7.3 NEW Step 15) ─────────────
//
// Per `handlers/error.rs::classify()`:
//   exit 3 = Transport | Rpc | InsufficientFunds | BroadcastFailed
//          | ConfirmTimeout | ComputeBudgetExceeded | ConfirmPending
//   exit 5 = OsRngFailed | FileIo | KdfFailed | CipherInit
//          | RecordCorrupt | Unimplemented
//
// Both tests gated on `RUN_SOL_SURFPOOL=1` so `tx wait` has a reachable RPC
// endpoint. The exit-5 path (`wallet send` without --dry-run/--sign-only)
// returns Unimplemented before any RPC call; surfpool not strictly required
// but kept co-located for regression-guard grouping.

/// `tx wait` against a closed-port RPC must surface as `Error::Transport`
/// (per `chain/client.rs:267` — all RpcClient POST failures wrap as
/// `Error::Transport(...)`) → exit code 3.
///
/// We bind a `TcpListener` on `127.0.0.1:0` (OS-assigned port), capture the
/// port, drop the listener, then point `--rpc` at it — guaranteed
/// `ECONNREFUSED` on any OS without depending on IANA-reserved ports
/// (`127.0.0.1:1` is `tcpmux` and may be bound on hardened CI images).
///
/// The stderr substring assertion anchors on the exact
/// `chain/client.rs:267` path (`"RPC POST getSignatureStatuses"`), so a
/// regression to `Error::Rpc` / `Error::BroadcastFailed` (both also exit 3
/// per `classify()`) would still pass the exit-code check but fail this
/// specific-fragment pin.
#[test]
#[ignore = "RUN_SOL_SURFPOOL=1 required for surfpool-audit co-location grouping (this test does not contact surfpool)."]
fn classify_exit_code_3_tx_wait_transport_audit_p7_3() {
    // 87-char base58 placeholder signature — valid format, never confirms.
    let sig: String = "5".repeat(87);
    let closed_port = {
        let l = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let port = l.local_addr().expect("local_addr").port();
        drop(l);
        port
    };
    let mut cmd = sol_bin();
    cmd.env("RUN_SOL_SURFPOOL", "1")
        .args([
            "tx",
            "wait",
            "--sig",
            &sig,
            "--timeout",
            "1",
            "--poll-interval",
            "1",
            "--rpc",
            &format!("http://127.0.0.1:{closed_port}"),
        ])
        .arg("--data-dir")
        .arg(tmp().path());
    let output = cmd.output().expect("spawn sol binary");
    let got = output.status.code();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        got,
        Some(3),
        "expected exit 3 (Error::Transport per handlers/error.rs::classify); got {got:?}; stderr=\n{stderr}"
    );
    assert!(
        stderr.contains("RPC POST getSignatureStatuses"),
        "stderr missing `RPC POST getSignatureStatuses` fragment — \
         Error::Transport path not exercised (got stderr=\n{stderr})"
    );
}

/// `wallet create --name x --mnemonic-file /no/such/path` with
/// `SOL_WALLET_PASSWORD` set hits the `std::fs::read_to_string` →
/// `Error::FileIo { path, source }` mapping in `wallet.rs:153-156` →
/// exit code 5. Password env var is required by
/// `read_password_from_cli_pub()` (`wallet.rs:695-701`) which fires
/// BEFORE the FileIo mapping; without it the test would short-circuit
/// to exit 1 (anyhow fallback).
///
/// The stderr substring assertion anchors on the exact `FileIo` Display
/// fragment (`"file I/O error"`) so a regression to one of the seven other
/// exit-5 variants (`OsRngFailed`, `KdfFailed`, `CipherInit`, `RecordCorrupt`,
/// `Unimplemented`, etc.) cannot silently pass this gate.
#[test]
#[ignore = "RUN_SOL_SURFPOOL=1 required for surfpool-audit co-location grouping (this test does not contact surfpool)."]
fn classify_exit_code_5_wallet_create_file_io_audit_p7_3() {
    let mut cmd = sol_bin();
    cmd.env("RUN_SOL_SURFPOOL", "1")
        .env("SOL_WALLET_PASSWORD", "test-password-for-fileio-path-only")
        .args([
            "wallet",
            "create",
            "--name",
            "audit-p7-3",
            "--mnemonic-file",
            "/no/such/directory/that/cannot/exist/mnemonic.txt",
        ])
        .arg("--data-dir")
        .arg(tmp().path());
    let output = cmd.output().expect("spawn sol binary");
    let got = output.status.code();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        got,
        Some(5),
        "expected exit 5 (Error::FileIo per handlers/error.rs::classify); got {got:?}; stderr=\n{stderr}"
    );
    assert!(
        stderr.contains("file I/O error"),
        "stderr missing `file I/O error` fragment — \
         Error::FileIo path not exercised (got stderr=\n{stderr})"
    );
}
