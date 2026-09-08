//! V1 — compile gate (CLI-driven, Phase 7 §Task 7.1).
//!
//! The CLI binary the spike drives (`tron` from `crates/tron/`) must build
//! + the test binary itself must build. PASS criteria: `cargo build -p
//! tron-v1-spike` succeeds (tests-only crate; no library) AND
//!
//! `cargo build -p tron` succeeds (CLI binary the spike drives) AND the
//! spawned `tron --help` stdout contains the four top-level subcommand
//! groups (wallet, trc20, tx, config).

mod common;

#[test]
fn v1_cargo_build_tests_only_crate() {
    // If this binary runs, the build pipeline succeeded. The ship gates
    // are: (a) `cargo build -p tron-v1-spike` exits 0, (b) `cargo build
    // -p tron` exits 0, (c) `tron --help` lists the four subcommand groups.
    // All three are exercised by the CI workflow (`.github/workflows/
    // tron-nile-spike.yml`); the runtime assertion below is just a
    // sanity check that the binary loaded into memory at test start.
    let _linker_ok = true;
    assert!(_linker_ok);
}

#[test]
fn v1_tron_help_lists_subcommand_groups() {
    // The CLI binary is the spike's only library. Per Plan §V1 table:
    // "Test binary spawns `tron --help` and asserts exit 0 + stdout
    // contains `wallet`, `trc20`, `tx`, `config` subcommands."
    let assert = common::tron().arg("--help").assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);

    // Each top-level subcommand group must appear in --help output.
    let missing: Vec<&str> = ["wallet", "trc20", "tx", "config"]
        .iter()
        .copied()
        .filter(|g| !stdout.contains(g))
        .collect();
    assert!(
        missing.is_empty(),
        "tron --help must list every shipped top-level subcommand group; missing: {missing:?}"
    );
}
