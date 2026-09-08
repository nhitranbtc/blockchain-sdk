//! V11 — Mainnet self-send gate via CLI (Q4).
//!
//! Plan §Q4: BLOCKING for v0.1 release. `$0.001 USDT-TRC20 to self`
//! (recipient == sender) on Mainnet, real value, real network.
//!
//! **CRITICAL design choice (REVISED 2026-09-07):** the **pre-check audit hook**
//! lives in the CLI (`crates/tron/src/handlers/trc20.rs`), NOT in this spike.
//! A regression where `recipient != operator_wallet` ships real money to the
//! wrong address — and that regression lives in the CLI, so the gate must
//! exercise the CLI, not bypass it via library calls.
//!
//! Operator wallet comes from the bundled `crates/tron-wallet-core/tokens/nile.json`
//! fixture (`test.owner_address`) via `common::nile_owner()`.

mod common;

#[test]
fn v11_pre_check_blocks_non_self_recipient() {
    // Operator wallet per `crates/tron-wallet-core/tokens/mainnet.json`
    // (`test.owner_address`); recipient below is a Nile fixture address
    // — guaranteed NOT equal, so the pre-check MUST fire.
    let operator = common::mainnet_owner();
    let other = common::nile_recipient();
    assert_ne!(
        operator, other,
        "operator ({operator}) must differ from other ({other}) for this test to be meaningful"
    );

    let mnemonic = common::nile_sender_mnemonic();
    let assert = common::tron()
        .arg("trc20")
        .arg("send")
        .arg("--mnemonic")
        .arg(&mnemonic)
        .arg("--contract")
        .arg("USDT")
        .arg("--to")
        .arg(other)
        .arg("--amount")
        .arg(common::one_usdt_display_amount())
        .arg("--network")
        .arg(common::mainnet_network())
        .arg("--confirm-yes")
        .assert();

    let out = assert.failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr);
    let stdout = String::from_utf8_lossy(&out.get_output().stdout);
    let combined = format!("{stderr}\n{stdout}");
    assert!(
        combined.contains("pre-check") || combined.contains("operator wallet"),
        "CLI must reject non-self mainnet transfer via pre-check audit; got stderr={stderr:?} stdout={stdout:?}"
    );

    eprintln!("[V11] pre-check correctly blocked non-self mainnet transfer");
}

#[test]
#[ignore = "GATED: real value, real network"]
fn v11_mainnet_self_send_succeeds() {
    let operator_wallet = common::mainnet_owner();

    // Self-send: `--to <operator-wallet>`. The pre-check audit must ALLOW
    // this (recipient == operator) and proceed to broadcast.
    let mnemonic = common::nile_sender_mnemonic();
    let assert = common::tron()
        .arg("trc20")
        .arg("send")
        .arg("--mnemonic")
        .arg(&mnemonic)
        .arg("--contract")
        .arg("USDT")
        .arg("--to")
        .arg(operator_wallet)
        .arg("--amount")
        .arg(common::one_usdt_display_amount()) // $0.001 USDT-TRC20
        .arg("--network")
        .arg(common::mainnet_network())
        .arg("--confirm-yes")
        .assert();

    let out = assert.success();
    let json: serde_json::Value = serde_json::from_slice(&out.get_output().stdout)
        .expect("CLI must emit valid JSON on successful broadcast");

    // CLI must emit the broadcast transaction id so the operator can
    // verify on TronScan.
    let txid = json
        .get("txid")
        .and_then(|v| v.as_str())
        .expect("CLI must emit `txid` on successful broadcast");
    assert_eq!(txid.len(), 64, "broadcast txid must be 64 hex chars");

    eprintln!("[V11] Mainnet self-send broadcast OK: {txid}");
}

#[test]
#[ignore = "GATED: real value, real network"]
fn v11_mainnet_self_send_then_balance_increases() {
    // After the self-send, the operator's USDT balance must not have
    // decreased by MORE than 0.001 USDT (we sent 0.001 to ourselves, so
    // net change is 0 minus the gas). A drop of more than 0.002 USDT
    // indicates funds went somewhere else — the audit hook regressed.
    let operator_wallet = common::mainnet_owner();

    // Get balance before.
    let before = common::tron()
        .arg("trc20")
        .arg("balance")
        .arg("--contract")
        .arg("USDT")
        .arg("--address")
        .arg(operator_wallet)
        .assert()
        .success();
    let before_balance: u64 =
        serde_json::from_slice::<serde_json::Value>(&before.get_output().stdout)
            .ok()
            .and_then(|j| j.get("balance").and_then(|v| v.as_u64()))
            .expect("CLI must emit `balance`");

    // Self-send 0.001 USDT.
    let mnemonic = common::nile_sender_mnemonic();
    common::tron()
        .arg("trc20")
        .arg("send")
        .arg("--mnemonic")
        .arg(&mnemonic)
        .arg("--contract")
        .arg("USDT")
        .arg("--to")
        .arg(operator_wallet)
        .arg("--amount")
        .arg(common::one_usdt_display_amount())
        .arg("--network")
        .arg(common::mainnet_network())
        .arg("--confirm-yes")
        .assert()
        .success();

    // Get balance after.
    let after = common::tron()
        .arg("trc20")
        .arg("balance")
        .arg("--contract")
        .arg("USDT")
        .arg("--address")
        .arg(operator_wallet)
        .assert()
        .success();
    let after_balance: u64 =
        serde_json::from_slice::<serde_json::Value>(&after.get_output().stdout)
            .ok()
            .and_then(|j| j.get("balance").and_then(|v| v.as_u64()))
            .expect("CLI must emit `balance`");

    // Net change for self-send = 0 (minus any TRX gas, which we do not measure
    // here). USDT balance should be EXACTLY equal before and after. A drop of
    // > 0 USDT means funds went elsewhere — audit hook regression.
    assert_eq!(
        before_balance, after_balance,
        "self-send must not change USDT balance; before={before_balance} after={after_balance} (delta={})",
        after_balance as i64 - before_balance as i64
    );
}
