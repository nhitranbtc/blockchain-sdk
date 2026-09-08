//! V6 — chain-id + address derivation smoke (CLI-driven, Phase 7 §Task 7.6).
//!
//! Both rows run offline against a per-test isolated data dir (no env-var gate,
//! no live RPC). The shipped `tron` CLI surface drives the assertions.

mod common;

use std::path::PathBuf;

use assert_cmd::Command;

fn isolated_config_home() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_path_buf();
    common::set_test_data_dir(path.clone());
    std::env::set_var("TRON_PASSWORD", common::test_password());
    (dir, path)
}

fn tron() -> Command {
    common::tron()
}

#[test]
fn v6_wallet_address_derives_34_char_t_string() {
    let out = tron()
        .args([
            "wallet",
            "address",
            "--pubkey",
            common::secp256k1_generator_pubkey_hex(),
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout);
    let address = stdout.trim();
    assert_eq!(
        address.len(),
        34,
        "TRON base58check address must be 34 chars; got {address:?}"
    );
    assert!(
        address.starts_with('T'),
        "TRON addresses start with 'T'; got {address:?}"
    );

    assert!(
        address[1..]
            .chars()
            .all(|c| common::bs58_alphabet().contains(c)),
        "non-prefix chars must be base58 alphabet (no 0/O/I/l); got {address:?}"
    );
    eprintln!("[V6-row_1] tron wallet address --pubkey (G point) -> {address}");
}

#[test]
fn v6_config_show_clean_dir_defaults_to_nile_rpc() {
    let (_dir, _path) = isolated_config_home();

    let out = tron().args(["config", "show", "--json"]).assert().success();
    let json: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("CLI must emit JSON");
    let network = json
        .get("network")
        .and_then(|v| v.as_str())
        .expect("network field");
    let rpc_url = json
        .get("rpc_url")
        .and_then(|v| v.as_str())
        .expect("rpc_url field");
    assert_eq!(
        network,
        common::nile_network(),
        "clean data dir must default to nile"
    );
    assert_eq!(
        rpc_url,
        common::nile_rpc_url(),
        "rpc_url must match the bundled network.json nile default"
    );
    eprintln!("[V6-row_2] config show --json (clean dir) -> network={network} rpc_url={rpc_url}");
}

#[test]
fn v6_config_set_network_nile_round_trips_through_show() {
    let (_dir, _path) = isolated_config_home();

    tron()
        .args(["config", "set-network", common::nile_network()])
        .assert()
        .success();

    let out = tron().args(["config", "show", "--json"]).assert().success();
    let json: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("CLI must emit JSON");
    assert_eq!(
        json.get("network").and_then(|v| v.as_str()),
        Some(common::nile_network())
    );
    assert_eq!(
        json.get("rpc_url").and_then(|v| v.as_str()),
        Some(common::nile_rpc_url())
    );
    eprintln!("[V6-row_3] config set-network / show round-trip OK on nile");
}
