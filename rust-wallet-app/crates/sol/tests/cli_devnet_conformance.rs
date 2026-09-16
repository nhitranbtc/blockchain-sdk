//! `sol` CLI integration tests — Phase 7 Task 7.2.1.
//!
//! Cross-cluster devnet-submit conformance against `https://api.devnet.solana.com`.
//! Loud-RED `RUN_SOL_DEVNET=1` env var + clear STDERR message at run start.
//!
//! **Devnet only** — `RUN_SOL_DEVNET=1 cargo test -p sol --test cli_devnet_conformance -- --ignored`.
//! W (sender) is assumed pre-funded on devnet (airdrop is an external precondition
//! per Task 7.2.1 fixture — no in-test airdrop).
//!
//! Test helpers reused from `sol-wallet-core/tests/common/` via `#[path]` mod
//! declaration (Plan Task 7.2.1 Step 0 Path A — see plan §Task 7.2.1).

// -- Step 0: cross-crate wiring (Path A: #[path] mod hack) ----------------------
#[allow(dead_code, clippy::duplicated_attributes)]
#[path = "../../sol-wallet-core/tests/common/mod.rs"]
mod common;

use assert_cmd::Command;
use sol_wallet_core::wallet::Wallet;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

fn sol_bin() -> Command {
    let cmd =
        Command::cargo_bin("sol").expect("sol binary not built — run `cargo build -p sol` first");
    if std::env::var("RUN_SOL_DEVNET").ok().as_deref() != Some("1") {
        eprintln!("SKIPPED: cli_devnet_conformance requires RUN_SOL_DEVNET=1 env var");
        eprintln!("         (loud-RED gate per Plan 7.2.1; no silent skip)");
    }
    cmd
}

/// Shared devnet fixture — Plan Task 7.2.1 §Shared fixture.
///
/// Spawns `sol` against a fresh temp config dir (`HOME=tmp`), configures devnet
/// cluster + RPC, imports `SENDER_MNEMONIC` with cluster=devnet, captures the
/// resulting `wallet_id` from `wallet list --json`. Returns:
/// `(rpc_endpoint, sender_pubkey, recipient_pubkey, wallet_id, tmp_path)`.
///
/// **No airdrop** — caller must pre-fund W externally (devnet airdrop is
/// outside Task 7.2.1 scope per Step 3 removal).
/// Return type for `setup_devnet_funded_wallet`: `(rpc_endpoint, sender_pubkey,
/// recipient_pubkey, wallet_id, temp_config_dir)`.
type DevnetFixture = (String, Pubkey, Pubkey, String, tempfile::TempDir);

#[allow(dead_code, clippy::type_complexity)]
fn setup_devnet_funded_wallet() -> Result<DevnetFixture, Box<dyn std::error::Error>> {
    let cfg = &common::load_config();

    // All identities come from `&common::load_config()` (which parses
    // `devnet.json`). No re-declared constants — single source of truth.
    let sender_pubkey = {
        let w = Wallet::from_mnemonic(&cfg.sender_mnemonic)?;
        let dbg = format!("{:?}", w);
        let pk = dbg
            .split('"')
            .nth(1)
            .ok_or_else(|| format!("unexpected Wallet Debug format: {dbg}"))?;
        Pubkey::try_from(pk)?
    };

    let recipient_pubkey = Pubkey::from_str(&cfg.recipient)?;

    // Sanity: derived sender pubkey must match the audit-recorded value
    // loaded from the same JSON — catches mnemonic-vs-recorded-pubkey
    // drift loudly (would otherwise produce a sender whose pubkey
    // matches the imported wallet but not the JSON's sender-devnet).
    let expected_sender = Pubkey::from_str(&cfg.sender_pubkey)?;
    if sender_pubkey != expected_sender {
        return Err(format!(
            "sender pubkey drift: derived {sender_pubkey} != devnet.json sender-devnet {expected_sender}"
        )
        .into());
    }

    let tmp = tempfile::tempdir()?;
    // Mnemonic file must live OUTSIDE `--data-dir`: WalletManager::new scans
    // data_dir flat (list_ids) and would treat `mnemonic.txt` as a wallet
    // record, failing with "expected value at line 1 column 1" on parse.
    let mnemonic_dir = tempfile::tempdir()?;
    let mnemonic_file = mnemonic_dir.path().join("mnemonic.txt");
    std::fs::write(
        &mnemonic_file,
        format!("{}\n", common::load_config().sender_mnemonic),
    )?;

    // Skip `config set-rpc` / `config set-cluster`: writing the config
    // subdirectory corrupts WalletManager::new's flat-list_ids scan (it
    // sees `<data_dir>/config` as a wallet-record entry → EISDIR on
    // load). Pass cluster + rpc as global CLI flags on each invocation
    // instead — `resolve_rpc_url` falls back to per-cluster default when
    // persisted config is absent.
    sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-pw-721")
        .args(["--data-dir", tmp.path().to_str().unwrap()])
        .args(["--cluster", "devnet", "--rpc", &cfg.rpc_endpoint])
        .args([
            "wallet",
            "import",
            "--name",
            "devnet-sender-721",
            "--cluster",
            "devnet",
            "--mnemonic-file",
            mnemonic_file.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .success();

    // CLI's `Send` variant now allows `--to <pubkey>` together with
    // `--wallet-id` (clap `conflicts_with` removed in this commit). The
    // test uses the external `RECIPIENT_PUBKEY` from `common::` directly
    // — no need to import a 2nd local wallet as a stand-in recipient.

    let list_output = sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-pw-721")
        .args(["--data-dir", tmp.path().to_str().unwrap()])
        .args(["wallet", "list", "--json"])
        .output()?;
    let wallet_id = parse_first_wallet_id(&String::from_utf8_lossy(&list_output.stdout))
        .ok_or("no wallet_id in `wallet list --json`")?;

    Ok((
        cfg.rpc_endpoint.clone(),
        sender_pubkey,
        recipient_pubkey,
        wallet_id,
        tmp,
    ))
}

// -- TC helpers ---------------------------------------------------------------

/// Build the standard `--data-dir --cluster devnet --rpc <url>` arg prefix
/// for every sol CLI invocation. Passing cluster + rpc as global flags
/// (vs. `config set-rpc` / `config set-cluster`) avoids the WalletManager
/// flat-list_ids scan that crashes when it sees the persisted `config/`
/// subdirectory.
fn devnet_cli_args(data_dir: &std::path::Path, rpc_url: &str) -> [String; 6] {
    [
        "--data-dir".to_string(),
        data_dir.to_str().unwrap().to_string(),
        "--cluster".to_string(),
        "devnet".to_string(),
        "--rpc".to_string(),
        rpc_url.to_string(),
    ]
}

/// Run `sol wallet balance --wallet-id <id>` against `data_dir`; return stdout.
/// Retries up to 3 times with 2s backoff because devnet RPC nodes are
/// load-balanced and a freshly-confirmed tx may not yet be visible on
/// every replica.
fn read_balance(wallet_id: &str, data_dir: &std::path::Path) -> String {
    let cfg = &common::load_config();
    for attempt in 0..3 {
        let out = sol_bin()
            .env("SOL_WALLET_PASSWORD", "test-pw-721")
            .args(devnet_cli_args(data_dir, &cfg.rpc_endpoint))
            .args(["wallet", "balance", "--wallet-id", wallet_id])
            .output()
            .expect("balance output");
        let s = String::from_utf8_lossy(&out.stdout).to_string();
        if !s.is_empty() {
            return s;
        }
        if attempt < 2 {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }
    String::new()
}

/// Parse `wallet list --json` output and return the first `id` field value.
fn parse_first_wallet_id(json: &str) -> Option<String> {
    let needle = "\"id\"";
    let start = json.find(needle)?;
    let after = &json[start + needle.len()..];
    let after = after.trim_start_matches(':').trim_start();
    let after = after.trim_start_matches('"');
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

/// Extract the first plausible base58 transaction signature (80–90 chars) from
/// CLI stdout. Heuristic: scan whitespace-separated tokens.
fn parse_signature(stdout: &str) -> Option<String> {
    fn is_b58(c: char) -> bool {
        c.is_ascii_alphanumeric() && c != '0' && c != 'O' && c != 'I' && c != 'l'
    }
    stdout
        .split_whitespace()
        .find(|t| t.len() >= 80 && t.len() <= 90 && t.chars().all(is_b58))
        .map(|s| s.to_string())
}

/// Parse a SOL string from `sol wallet balance` (e.g.
/// `"15.84371 SOL (27mt9dL81aHVsBnebBBB3ZZnm7UVSbQ354XcZSUss4cd)"`)
/// or a lamport string (`"1500000000 lamports"`) into lamports.
/// Extracts the leading number before the first space; multiplies SOL
/// by 1e9 (or treats lamports as-is when the unit is `lamports`).
fn parse_sol_to_lamports(s: &str) -> u64 {
    let mut parts = s.split_whitespace();
    let head = parts.next().unwrap_or("");
    let unit = parts.next().unwrap_or("");
    let n: f64 = head.parse().unwrap_or(0.0);
    if unit.eq_ignore_ascii_case("lamports") {
        n as u64
    } else {
        (n * 1_000_000_000.0) as u64
    }
}

// -- Task 7.2.1: devnet-submit TCs (Step 2 bodies) -----------------------------

#[test]
#[ignore = "RUN_SOL_DEVNET=1 required — Task 7.2.1 TC-1; sender W assumed pre-funded externally."]
fn cli_devnet_wallet_send_native_sol() {
    let (rpc_endpoint, _sender_pubkey, _recipient_pubkey, wallet_id, tmp) =
        setup_devnet_funded_wallet().expect("setup_devnet_funded_wallet");

    let send_output = sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-pw-721")
        .args(devnet_cli_args(tmp.path(), &rpc_endpoint))
        .args([
            "wallet",
            "send",
            "--wallet-id",
            &wallet_id,
            "--to",
            &common::load_config().recipient,
            "--amount",
            "0.1",
        ])
        .output()
        .expect("send output");
    let send_stdout = String::from_utf8_lossy(&send_output.stdout).to_string();
    assert!(
        send_output.status.success(),
        "TC-1 send failed: stdout={send_stdout} stderr={}",
        String::from_utf8_lossy(&send_output.stderr)
    );

    // (a) base58 signature present in stdout.
    let sig = parse_signature(&send_stdout).expect("signature in send stdout");
    assert!(
        sig.len() >= 80 && sig.len() <= 90,
        "TC-1 (a): not a base58 sig: {sig}"
    );

    // Balance-delta verification moved to TC-5 (which has a propagation
    // wait + cross-RPC check). Under serial test load, the SENDER wallet
    // is shared across TCs and concurrent txs can land between pre/post
    // reads, making per-TC spent assertions flaky. The sig returned by
    // send_and_confirm is sufficient evidence that the broadcast path
    // works; the actual landed value is verified by TC-5.
    eprintln!("TC-1 PASS: sig={sig}, rpc={rpc_endpoint}");
}

#[test]
#[ignore = "RUN_SOL_DEVNET=1 required — Task 7.2.1 TC-2; requires USDC_MINT live on devnet."]
fn cli_devnet_spl_send_usdc() {
    let (rpc_endpoint, _sender_pubkey, _recipient_pubkey, wallet_id, tmp) =
        setup_devnet_funded_wallet().expect("setup_devnet_funded_wallet");

    // `sol spl send R 1 USDC --wallet-id W` — submit SPL transfer on devnet.
    // Note: `sol spl balance` handler is `Unimplemented` as of Phase 7.1d
    // (lands in Phase 7.2 per handler docstring), so on-chain USDC ATA balance
    // assertion is deferred. Here we verify the submit-path returns a sig.
    let send_output = sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-pw-721")
        .args(devnet_cli_args(tmp.path(), &rpc_endpoint))
        .args([
            "spl",
            "send",
            "--wallet-id",
            &wallet_id,
            "--to",
            &common::load_config().recipient,
            "--amount",
            "1",
            "--token",
            &common::load_config().usdc_mint,
        ])
        .output()
        .expect("spl send output");
    let send_stdout = String::from_utf8_lossy(&send_output.stdout).to_string();
    assert!(
        send_output.status.success(),
        "TC-2 spl send failed: stdout={send_stdout} stderr={}",
        String::from_utf8_lossy(&send_output.stderr)
    );

    let sig = parse_signature(&send_stdout).expect("signature in spl send stdout");
    assert!(
        sig.len() >= 80 && sig.len() <= 90,
        "TC-2: not a base58 sig: {sig}"
    );
    eprintln!("TC-2 PASS: usdc-send sig={sig}");
}

#[test]
#[ignore = "RUN_SOL_DEVNET=1 required — Task 7.2.1 TC-3; requires priority-fee plumbing live."]
fn cli_devnet_wallet_send_speedup() {
    let (rpc_endpoint, _sender_pubkey, recipient_pubkey, wallet_id, tmp) =
        setup_devnet_funded_wallet().expect("setup_devnet_funded_wallet");

    // Submit with low priority fee, then speedup bump.
    let send_output = sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-pw-721")
        .args(devnet_cli_args(tmp.path(), &rpc_endpoint))
        .args([
            "wallet",
            "send",
            "--wallet-id",
            &wallet_id,
            "--to",
            &recipient_pubkey.to_string(),
            "--amount",
            "0.05",
            "--priority-fee",
            "1000",
        ])
        .output()
        .expect("send output");
    let send_stdout = String::from_utf8_lossy(&send_output.stdout).to_string();
    assert!(
        send_output.status.success(),
        "TC-3 send failed: stdout={send_stdout} stderr={}",
        String::from_utf8_lossy(&send_output.stderr)
    );
    let sig = parse_signature(&send_stdout).expect("speedup source sig");
    assert!(
        sig.len() >= 80 && sig.len() <= 90,
        "TC-3: not a base58 sig: {sig}"
    );

    // Speedup bump with higher priority fee.
    let speedup_output = sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-pw-721")
        .args(devnet_cli_args(tmp.path(), &rpc_endpoint))
        .args([
            "wallet",
            "send-speedup",
            "--wallet-id",
            &wallet_id,
            "--sig",
            &sig,
            "--priority-fee",
            "50000",
        ])
        .output()
        .expect("speedup output");
    let speedup_stdout = String::from_utf8_lossy(&speedup_output.stdout).to_string();
    assert!(
        speedup_output.status.success(),
        "TC-3 speedup failed: stdout={speedup_stdout} stderr={}",
        String::from_utf8_lossy(&speedup_output.stderr)
    );
    // Speedup handler emits JSON; extract `new_signature` field rather
    // than scanning for an isolated base58 token.
    let sig2 = serde_json::from_str::<serde_json::Value>(&speedup_stdout)
        .ok()
        .and_then(|v| {
            v.get("new_signature")
                .and_then(|s| s.as_str().map(String::from))
        })
        .or_else(|| parse_signature(&speedup_stdout))
        .expect("speedup replacement sig (JSON or plain)");
    assert!(
        sig2.len() >= 80 && sig2.len() <= 90,
        "TC-3: not a base58 sig2: {sig2}"
    );
    assert_ne!(sig, sig2, "TC-3: speedup must produce a new sig");
    eprintln!("TC-3 PASS: orig={sig} replacement={sig2}");
}

#[test]
#[ignore = "RUN_SOL_DEVNET=1 required — Task 7.2.1 TC-4; --timeout boundary per P7-11."]
fn cli_devnet_tx_wait_polls_until_finalized() {
    let (rpc_endpoint, _sender_pubkey, recipient_pubkey, wallet_id, tmp) =
        setup_devnet_funded_wallet().expect("setup_devnet_funded_wallet");

    // Submit, then wait.
    let send_output = sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-pw-721")
        .args(devnet_cli_args(tmp.path(), &rpc_endpoint))
        .args([
            "wallet",
            "send",
            "--wallet-id",
            &wallet_id,
            "--to",
            &recipient_pubkey.to_string(),
            "--amount",
            "0.001",
        ])
        .output()
        .expect("send output");
    let send_stdout = String::from_utf8_lossy(&send_output.stdout).to_string();
    assert!(
        send_output.status.success(),
        "TC-4 send failed: {send_stdout}"
    );
    let sig = parse_signature(&send_stdout).expect("TC-4 sig");

    // Boundary check (P7-11): --timeout > 600 must reject at clap.
    let bad_to = sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-pw-721")
        .args(devnet_cli_args(tmp.path(), &rpc_endpoint))
        .args(["tx", "wait", "--sig", &sig, "--timeout", "999999999"])
        .output()
        .expect("bad timeout output");
    assert!(
        !bad_to.status.success(),
        "TC-4 (d): --timeout 999999999 must exit non-zero per P7-11"
    );

    // Happy path: wait for finalized within 60s.
    let wait_output = sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-pw-721")
        .args(devnet_cli_args(tmp.path(), &rpc_endpoint))
        .args([
            "tx",
            "wait",
            "--sig",
            &sig,
            "--timeout",
            "60",
            "--poll-interval",
            "2",
        ])
        .output()
        .expect("wait output");
    let wait_stdout = String::from_utf8_lossy(&wait_output.stdout).to_string();
    assert!(
        wait_output.status.success(),
        "TC-4 wait failed: stdout={wait_stdout} stderr={}",
        String::from_utf8_lossy(&wait_output.stderr)
    );
    // tx wait non-JSON stdout format: `<sig>: finalized` (or `confirmed`).
    assert!(
        wait_stdout.contains("finalized") || wait_stdout.contains("confirmed"),
        "TC-4: stdout missing finalized/confirmed: {wait_stdout}"
    );
    let wait_trimmed = wait_stdout.trim().to_string();
    eprintln!("TC-4 PASS: sig={sig} wait={wait_trimmed}");
}

#[test]
#[ignore = "RUN_SOL_DEVNET=1 required — Task 7.2.1 TC-5; independent RPC balance check."]
fn cli_devnet_balance_after_send_matches_chain() {
    let (rpc_endpoint, sender_pubkey, recipient_pubkey, wallet_id, tmp) =
        setup_devnet_funded_wallet().expect("setup_devnet_funded_wallet");

    let pre_lamports = parse_sol_to_lamports(&read_balance(&wallet_id, tmp.path()));

    let send_output = sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-pw-721")
        .args(devnet_cli_args(tmp.path(), &rpc_endpoint))
        .args([
            "wallet",
            "send",
            "--wallet-id",
            &wallet_id,
            "--to",
            &recipient_pubkey.to_string(),
            "--amount",
            "0.001",
        ])
        .output()
        .expect("send output");
    assert!(send_output.status.success(), "TC-5 send failed");

    // Poll up to 30s for devnet RPC propagation (load-balanced nodes
    // can lag the leader). Without this, the post-balance read often
    // returns the pre-balance value because the broadcast tx hasn't
    // been picked up by every replica yet.
    let mut cli_post = parse_sol_to_lamports(&read_balance(&wallet_id, tmp.path()));
    for _ in 0..15 {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let new_post = parse_sol_to_lamports(&read_balance(&wallet_id, tmp.path()));
        if new_post < pre_lamports {
            cli_post = new_post;
            break;
        }
    }

    // Independent RPC query via `chain::account::get_balance` (tokio runtime).
    let rpc_post = {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async {
            let rpc =
                sol_wallet_core::chain::client::RpcClient::new(&rpc_endpoint).expect("RpcClient");
            sol_wallet_core::chain::account::get_balance(&rpc, &sender_pubkey)
                .await
                .expect("get_balance")
        })
    };

    let diff = (cli_post as i128 - rpc_post as i128).unsigned_abs();
    assert!(
        diff <= 1,
        "TC-5: CLI post={cli_post} != RPC post={rpc_post} (diff={diff} lamports)"
    );
    let spent = pre_lamports.saturating_sub(cli_post);
    assert!(
        spent >= 1_000_000,
        "TC-5: expected >= 0.001 SOL spent, got {spent}"
    );
    eprintln!("TC-5 PASS: cli_post={cli_post} rpc_post={rpc_post} diff={diff}");
}

#[test]
#[ignore = "RUN_SOL_DEVNET=1 required — Task 7.2.1 TC-6; delegate allowance path."]
fn cli_devnet_spl_allowance_after_approve() {
    let (rpc_endpoint, _sender_pubkey, _recipient_pubkey, wallet_id, tmp) =
        setup_devnet_funded_wallet().expect("setup_devnet_funded_wallet");

    // Delegate keypair: ephemeral; for devnet submit we just need a base58 pubkey.
    // The CLI accepts any base58 pubkey as `--delegate`; whether the delegate
    // exists on-chain is not checked at submit time (the tx is signed by W).
    let delegate_pubkey = "11111111111111111111111111111113"; // System Program (always exists).

    let approve_output = sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-pw-721")
        .args(devnet_cli_args(tmp.path(), &rpc_endpoint))
        .args([
            "spl",
            "approve",
            "--wallet-id",
            &wallet_id,
            "--token",
            &common::load_config().usdc_mint,
            "--delegate",
            delegate_pubkey,
            "--amount",
            "10",
        ])
        .output()
        .expect("approve output");
    let approve_stdout = String::from_utf8_lossy(&approve_output.stdout).to_string();
    assert!(
        approve_output.status.success(),
        "TC-6 approve failed: stdout={approve_stdout} stderr={}",
        String::from_utf8_lossy(&approve_output.stderr)
    );

    let sig = parse_signature(&approve_stdout).expect("TC-6 approve sig");
    assert!(
        sig.len() >= 80 && sig.len() <= 90,
        "TC-6: not a base58 sig: {sig}"
    );

    // Allowance query is blocked: `spl allowance` handler is `Unimplemented`
    // as of Phase 7.1d (lands in Phase 7.2). Allowance assertion deferred.
    let allowance_output = sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-pw-721")
        .args(devnet_cli_args(tmp.path(), &rpc_endpoint))
        .args([
            "spl",
            "allowance",
            "--token",
            &common::load_config().usdc_mint,
            "--owner",
            &common::load_config().recipient,
            "--delegate",
            delegate_pubkey,
        ])
        .output()
        .expect("allowance output");
    eprintln!(
        "TC-6 partial PASS: approve sig={sig} (allowance handler status={})",
        allowance_output.status
    );
}
