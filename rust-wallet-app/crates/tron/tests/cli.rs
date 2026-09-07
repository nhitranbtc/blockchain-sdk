//! `tron` CLI integration tests — plan §Phase 6 Verification.
//!
//! Subprocess-level, so they assert the contract a script actually sees:
//! the subcommand inventory, the exit codes, and the STDOUT/STDERR split.
//! No test touches the network; every command exercised here is either
//! `--help`, local config, or a path that fails before any RPC call.

use std::process::Command;

fn tron(data_dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tron"));
    cmd.arg("--data-dir").arg(data_dir);
    cmd
}

/// `--help` on a group, returned as (stdout, exit code).
fn help_of(args: &[&str]) -> (String, i32) {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tron(dir.path())
        .args(args)
        .arg("--help")
        .output()
        .expect("spawn tron --help");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn root_help_lists_all_six_top_level_groups() {
    let (stdout, code) = help_of(&[]);
    assert_eq!(code, 0, "`tron --help` must exit 0");
    for group in ["wallet", "address", "balance", "trc20", "tx", "config"] {
        assert!(
            stdout.contains(group),
            "root help must list `{group}`; got:\n{stdout}"
        );
    }
}

#[test]
fn wallet_help_lists_nine_subcommands() {
    let (stdout, code) = help_of(&["wallet"]);
    assert_eq!(code, 0);
    for sub in [
        "create",
        "import",
        "show",
        "list",
        "delete",
        "rename",
        "balance",
        "send",
        "send-speedup",
    ] {
        assert!(
            stdout.contains(sub),
            "wallet help must list `{sub}`; got:\n{stdout}"
        );
    }
}

#[test]
fn trc20_help_lists_four_subcommands() {
    let (stdout, code) = help_of(&["trc20"]);
    assert_eq!(code, 0);
    for sub in ["send", "approve", "balance", "allowance"] {
        assert!(
            stdout.contains(sub),
            "trc20 help must list `{sub}`; got:\n{stdout}"
        );
    }
}

#[test]
fn tx_help_lists_two_subcommands() {
    let (stdout, code) = help_of(&["tx"]);
    assert_eq!(code, 0);
    for sub in ["get", "wait"] {
        assert!(
            stdout.contains(sub),
            "tx help must list `{sub}`; got:\n{stdout}"
        );
    }
}

#[test]
fn address_and_config_help_list_their_subcommands() {
    let (address, _) = help_of(&["address"]);
    for sub in ["new", "xpub"] {
        assert!(address.contains(sub), "address help must list `{sub}`");
    }
    let (config, _) = help_of(&["config"]);
    for sub in ["show", "set-rpc", "set-network"] {
        assert!(config.contains(sub), "config help must list `{sub}`");
    }
}

#[test]
fn config_show_exits_zero_on_a_clean_data_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tron(dir.path())
        .args(["config", "show"])
        .output()
        .expect("spawn config show");
    assert_eq!(
        out.status.code(),
        Some(0),
        "config show must work before any config file exists; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("rpc_url"), "got:\n{stdout}");
    assert!(
        stdout.contains("nile"),
        "default network is nile; got:\n{stdout}"
    );
}

#[test]
fn config_show_json_is_parseable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tron(dir.path())
        .args(["config", "show", "--json"])
        .output()
        .expect("spawn config show --json");
    assert_eq!(out.status.code(), Some(0));
    let value: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("--json output must be valid JSON on STDOUT");
    assert_eq!(value["network"], "nile");
}

#[test]
fn config_set_network_then_show_reflects_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let set = tron(dir.path())
        .args(["config", "set-network", "mainnet"])
        .output()
        .expect("spawn set-network");
    assert_eq!(set.status.code(), Some(0));
    // Diagnostics belong on STDERR — STDOUT stays empty for non-data commands.
    assert!(
        set.stdout.is_empty(),
        "set-network must not write to STDOUT; got {:?}",
        String::from_utf8_lossy(&set.stdout)
    );

    let show = tron(dir.path())
        .args(["config", "show", "--json"])
        .output()
        .expect("spawn show");
    let value: serde_json::Value = serde_json::from_slice(&show.stdout).expect("json");
    assert_eq!(value["network"], "mainnet");
}

#[test]
fn config_set_rpc_rejects_a_non_http_url_with_exit_two() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tron(dir.path())
        .args(["config", "set-rpc", "ftp://example.invalid"])
        .output()
        .expect("spawn set-rpc");
    assert_eq!(out.status.code(), Some(2), "bad input is exit 2");
}

#[test]
fn address_new_derives_a_t_address_from_a_mnemonic_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let phrase_path = dir.path().join("phrase.txt");
    std::fs::write(
        &phrase_path,
        "abandon abandon abandon abandon abandon abandon \
         abandon abandon abandon abandon abandon about",
    )
    .expect("write phrase");

    let out = tron(dir.path())
        .args(["address", "new", "--mnemonic-file"])
        .arg(&phrase_path)
        .args(["--index", "0"])
        .output()
        .expect("spawn address new");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let address = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(address.starts_with('T'), "got {address:?}");
    assert_eq!(address.len(), 34, "got {address:?}");
}

#[test]
fn address_new_rejects_a_bad_mnemonic_with_exit_two() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tron(dir.path())
        .args(["address", "new", "--mnemonic", "not a valid bip39 phrase"])
        .output()
        .expect("spawn address new");
    assert_eq!(out.status.code(), Some(2), "invalid mnemonic is exit 2");
}

#[test]
fn wallet_create_routes_the_mnemonic_to_stderr_not_stdout() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tron(dir.path())
        .args(["wallet", "create", "--words", "12", "--network", "nile"])
        .env("TRON_PASSWORD", "correct-horse-battery-staple")
        .output()
        .expect("spawn wallet create");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    // STDOUT is the wallet id and nothing else: 32 hex chars.
    assert_eq!(
        stdout.len(),
        32,
        "STDOUT must be the wallet id; got {stdout:?}"
    );
    assert!(stdout.chars().all(|c| c.is_ascii_hexdigit()));
    // The phrase is on STDERR, so `$(tron wallet create)` never captures it.
    assert!(
        stderr.contains("RECOVERY PHRASE"),
        "phrase must be announced on STDERR; got:\n{stderr}"
    );
    assert!(
        !stdout.contains(' '),
        "STDOUT must never contain a seed phrase; got {stdout:?}"
    );
}

#[test]
fn wallet_create_then_list_and_show_round_trip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let create = tron(dir.path())
        .args(["wallet", "create", "--words", "24"])
        .env("TRON_PASSWORD", "correct-horse-battery-staple")
        .output()
        .expect("spawn create");
    let id = String::from_utf8_lossy(&create.stdout).trim().to_string();

    let list = tron(dir.path())
        .args(["wallet", "list"])
        .output()
        .expect("spawn list");
    assert_eq!(list.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&list.stdout).contains(&id),
        "created wallet must appear in `wallet list`"
    );

    let show = tron(dir.path())
        .args(["wallet", "show", "--id", &id, "--json"])
        .env("TRON_PASSWORD", "correct-horse-battery-staple")
        .output()
        .expect("spawn show");
    assert_eq!(
        show.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&show.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&show.stdout).expect("json");
    assert!(value["address"]
        .as_str()
        .expect("address field")
        .starts_with('T'));
}

#[test]
fn wallet_show_with_a_wrong_passphrase_exits_four() {
    let dir = tempfile::tempdir().expect("tempdir");
    let create = tron(dir.path())
        .args(["wallet", "create"])
        .env("TRON_PASSWORD", "correct-horse-battery-staple")
        .output()
        .expect("spawn create");
    let id = String::from_utf8_lossy(&create.stdout).trim().to_string();

    let show = tron(dir.path())
        .args(["wallet", "show", "--id", &id])
        .env("TRON_PASSWORD", "wrong-passphrase")
        .output()
        .expect("spawn show");
    assert_eq!(
        show.status.code(),
        Some(4),
        "a wrong passphrase is a wallet issue (exit 4), not bad input"
    );
}

#[test]
fn wallet_delete_removes_the_blob_when_confirmed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let create = tron(dir.path())
        .args(["wallet", "create"])
        .env("TRON_PASSWORD", "correct-horse-battery-staple")
        .output()
        .expect("spawn create");
    let id = String::from_utf8_lossy(&create.stdout).trim().to_string();

    let del = tron(dir.path())
        .args(["wallet", "delete", "--id", &id, "--confirm-yes"])
        .output()
        .expect("spawn delete");
    assert_eq!(del.status.code(), Some(0));

    let list = tron(dir.path())
        .args(["wallet", "list"])
        .output()
        .expect("spawn list");
    assert!(
        !String::from_utf8_lossy(&list.stdout).contains(&id),
        "deleted wallet must not be listed"
    );
}

#[test]
fn wallet_rename_rewrites_the_label() {
    let dir = tempfile::tempdir().expect("tempdir");
    let create = tron(dir.path())
        .args(["wallet", "create", "--name", "old", "--network", "nile"])
        .env("TRON_PASSWORD", "correct-horse-battery-staple")
        .output()
        .expect("spawn create");
    let id = String::from_utf8_lossy(&create.stdout).trim().to_string();

    let renamed = tron(dir.path())
        .args(["wallet", "rename", "--id", &id, "--to", "new"])
        .env("TRON_PASSWORD", "correct-horse-battery-staple")
        .output()
        .expect("spawn rename");
    assert_eq!(
        renamed.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&renamed.stderr)
    );

    let show = tron(dir.path())
        .args(["wallet", "show", "--id", &id, "--json"])
        .env("TRON_PASSWORD", "correct-horse-battery-staple")
        .output()
        .expect("spawn show");
    let value: serde_json::Value = serde_json::from_slice(&show.stdout).expect("json");
    assert_eq!(value["name"], "new");
    assert_eq!(value["network"], "nile", "rename must not lose the network");
}

#[test]
fn wallet_rename_with_a_wrong_passphrase_exits_four() {
    let dir = tempfile::tempdir().expect("tempdir");
    let create = tron(dir.path())
        .args(["wallet", "create"])
        .env("TRON_PASSWORD", "correct-horse-battery-staple")
        .output()
        .expect("spawn create");
    let id = String::from_utf8_lossy(&create.stdout).trim().to_string();

    let renamed = tron(dir.path())
        .args(["wallet", "rename", "--id", &id, "--to", "new"])
        .env("TRON_PASSWORD", "wrong")
        .output()
        .expect("spawn rename");
    assert_eq!(renamed.status.code(), Some(4));
}

#[test]
fn wallet_import_from_a_private_key_file_warns_there_is_no_phrase_backup() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key_path = dir.path().join("key.hex");
    // The public BIP-32 test-vector scalar — valid, and holds nothing.
    std::fs::write(
        &key_path,
        "e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35\n",
    )
    .expect("write key");

    let out = tron(dir.path())
        .args(["wallet", "import", "--private-key-file"])
        .arg(&key_path)
        .args(["--name", "paper", "--json"])
        .env("TRON_PASSWORD", "correct-horse-battery-staple")
        .output()
        .expect("spawn import");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(value["is_private_key"], true);
    assert!(value["address"].as_str().expect("address").starts_with('T'));
    // A raw-key wallet has no phrase, and the CLI must say so rather than let
    // an operator believe a seed backup exists.
    assert!(String::from_utf8_lossy(&out.stderr).contains("no recovery phrase"));
}

#[test]
fn wallet_import_rejects_an_invalid_private_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key_path = dir.path().join("key.hex");
    std::fs::write(&key_path, "0".repeat(64)).expect("write key");

    let out = tron(dir.path())
        .args(["wallet", "import", "--private-key-file"])
        .arg(&key_path)
        .env("TRON_PASSWORD", "correct-horse-battery-staple")
        .output()
        .expect("spawn import");
    assert_eq!(out.status.code(), Some(2), "a zero scalar is bad input");
}

#[test]
fn wallet_list_shows_names_when_a_passphrase_is_available() {
    let dir = tempfile::tempdir().expect("tempdir");
    tron(dir.path())
        .args(["wallet", "create", "--name", "daily", "--network", "nile"])
        .env("TRON_PASSWORD", "pw")
        .output()
        .expect("spawn create");

    let listed = tron(dir.path())
        .args(["wallet", "list", "--json"])
        .env("TRON_PASSWORD", "pw")
        .output()
        .expect("spawn list");
    let value: serde_json::Value = serde_json::from_slice(&listed.stdout).expect("json");
    assert_eq!(value["wallets"][0]["name"], "daily");
    assert_eq!(value["wallets"][0]["network"], "nile");
}

#[test]
fn wallet_send_rejects_a_bad_recipient_before_any_network_call() {
    let dir = tempfile::tempdir().expect("tempdir");
    let create = tron(dir.path())
        .args(["wallet", "create"])
        .env("TRON_PASSWORD", "pw")
        .output()
        .expect("spawn create");
    let id = String::from_utf8_lossy(&create.stdout).trim().to_string();

    let out = tron(dir.path())
        .args([
            "wallet",
            "send",
            "--wallet-id",
            &id,
            "--to",
            "not-an-address",
            "--amount",
            "1",
            // An unroutable RPC: reaching it would be the failure mode this
            // test asserts *against*.
            "--rpc-url",
            "http://127.0.0.1:1",
        ])
        .env("TRON_PASSWORD", "pw")
        .output()
        .expect("spawn send");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a TRON address"));
}

#[test]
fn wallet_send_requires_a_recipient() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tron(dir.path())
        .args([
            "wallet",
            "send",
            "--wallet-id",
            &"0".repeat(32),
            "--amount",
            "1",
        ])
        .env("TRON_PASSWORD", "pw")
        .output()
        .expect("spawn send");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--to"));
}

#[test]
fn send_speedup_rejects_a_non_positive_fee_limit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tron(dir.path())
        .args([
            "wallet",
            "send-speedup",
            "--wallet-id",
            &"0".repeat(32),
            "--txid",
            "deadbeef",
            "--fee-limit",
            "0",
        ])
        .env("TRON_PASSWORD", "pw")
        .output()
        .expect("spawn send-speedup");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn an_unreachable_rpc_is_exit_three() {
    // Exit 3 is the "upstream transport" code scripts retry on; conflating it
    // with bad input (2) would make them retry a typo forever.
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tron(dir.path())
        .args([
            "balance",
            "--address",
            "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
            "--rpc-url",
            "http://127.0.0.1:1",
        ])
        .output()
        .expect("spawn balance");
    assert_eq!(
        out.status.code(),
        Some(3),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn unknown_subcommand_is_a_clap_usage_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tron(dir.path())
        .args(["wallet", "frobnicate"])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2), "clap usage errors exit 2");
}

#[test]
fn wallet_create_rejects_an_unsupported_word_count() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tron(dir.path())
        .args(["wallet", "create", "--words", "18"])
        .env("TRON_PASSWORD", "correct-horse-battery-staple")
        .output()
        .expect("spawn create");
    assert_eq!(out.status.code(), Some(2), "only 12 and 24 are valid");
}
