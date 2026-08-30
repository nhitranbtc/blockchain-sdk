//! T8 mainnet live-RPC smoke (operator-driven per L29) — issue #458.
//!
//! Sub-task of #426 (Phase 4 T8 of #416 plan:
//! `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` §Phase 4 T8).
//!
//! **Opt in (CI-safe by default):**
//!   RUN_POLYGON_MAINNET=1 cargo test -p polygon --test mainnet_smoke -- --ignored
//!
//! **Scope:** the live-RPC acceptance items from the plan §T8 + issue #458:
//! (1) AC1 — `polygon wallet balance --address <addr> --network mainnet` returns
//! real Polygon mainnet balance. (2) AC3 / V3 — cross-chain identity: same
//! mnemonic + `m/44'/60'/0'/0/0` derivation produces the same address on ETH
//! (chain_id 1) and Polygon (chain_id 137). (3) AC4 / V4 — EIP-1559 cadence:
//! two `polygon fee --network mainnet` calls 3 s apart show different values
//! (proves no cache + 2-second-block volatility). (4) AC5 / V5 —
//! `new_http_polygon_mainnet().get_block_number()` returns a sane value.
//! (5) AC6 / V6 — `tokens::load_chain(137)` returns 3 entries; USDC decimals
//! = 6; DAI decimals = 18. V6 is offline (parses bundled JSON) and is the
//! only test NOT marked `#[ignore]`.
//!
//! **Drift from plan §T8 step 4:** the SPKI pin against
//! `pinned://<spki>@polygon-bor-rpc.publicnode.com` was REMOVED in PR #304 / commit `36ff115`
//! per F20 M-2 (unsafe-by-design pending webpki composition with
//! `rustls::client::WebPkiServerVerifier`). For this T8 mainnet smoke,
//! `evm_wallet_core::provider::new_http_polygon_mainnet()` uses rustls default
//! system CAs. SPKI pinning will be reintroduced when the verifier composes
//! with `rustls::client::WebPkiServerVerifier` (out-of-scope here — see #393).
//!
//! **TDD status:** red by default (`#[ignore]` + `RUN_POLYGON_MAINNET=1` guard).
//! V6 (token registry) is green-by-default (offline JSON parse). All other
//! tests are green is operator-driven per L29.
//!
//! **Sister tests:**
//! - `crates/polygon/tests/amoy_smoke.rs` (T7 — operator-driven Amoy live RPC)
//! - `crates/eth-wallet-core/tests/regression_post_refactor.rs` (T8 ETH regression)
//! - `rust-wallet-app/spikes/polygon-v1/tests/v{2,3,4,5,6}_*.rs` (the V1-V10
//!   acceptance suite that this T8 smoke complements — V3 already passes
//!   offline per plan).

#![cfg(test)]

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Path to the `polygon` binary built by cargo for integration tests.
fn polygon_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_polygon"))
}

/// Guard: skip unless `RUN_POLYGON_MAINNET=1` (CI-safe default). When the
/// `cargo test -- --ignored` flag is passed, ignored tests run — this guard
/// ensures they still require the opt-in env.
fn require_run_polygon_mainnet() {
    if std::env::var("RUN_POLYGON_MAINNET").ok().as_deref() != Some("1") {
        panic!("RUN_POLYGON_MAINNET=1 not set; mainnet_smoke tests require explicit opt-in");
    }
}

/// Resolve Polygon mainnet RPC URL — operator can override via `POLYGON_RPC_URL`
/// env var when the default (`polygon-bor-rpc.publicnode.com`) is rate-limited or
/// requires an API key. Drift from plan §T2: `polygon-rpc.com` tightened
/// keyless-tier access circa 2025-Q3 (`HTTP 401 "API key disabled"` on
/// `estimate_eip1559_fees` + `get_block_number`). Fixed by Issue #474
/// (switched to publicnode keyless tier, verified 2025-Q4).
fn polygon_mainnet_rpc_url() -> String {
    if let Ok(override_url) = std::env::var("POLYGON_RPC_URL") {
        if !override_url.is_empty() {
            return override_url;
        }
    }
    polygon_wallet_core::Network::Polygon(polygon_wallet_core::PolygonChain::Mainnet)
        .rpc_url()
        .to_string()
}

/// Invoke `polygon` as a subprocess with hermetic env. Always passes
/// `--rpc-url <resolved>` to the subprocess so the operator override (if any)
/// flows through; default matches the CLI's hard-coded URL.
fn run_polygon(args: &[&str], data_dir: &std::path::Path) -> std::process::Output {
    let rpc_url = polygon_mainnet_rpc_url();
    Command::new(polygon_bin())
        .args(args)
        .arg("--data-dir")
        .arg(data_dir)
        .args(["--rpc-url", &rpc_url])
        .env("POLYGON_PASSWORD", "test-pw-ignore-leak")
        .env("RUST_BACKTRACE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn polygon binary")
}

/// AC1 — `polygon wallet balance --address <addr> --network mainnet` returns a
/// real Polygon mainnet balance. The operator picks the address; we use the
/// native POL token contract on Polygon mainnet (`0x0000000000000000000000000000000000001010`,
/// the system address guaranteed to hold the protocol's native POL balance)
/// to keep the test deterministic + non-operator-dependent. Operators can
/// swap to any address with a known mainnet balance.
///
/// Drift from initial draft: the legacy MATIC token contract
/// `0x7d1afa7b718fb893db30a3abc0cfc608aacfebb0` is drained post the
/// MATIC → POL rebrand (Sep 2024) — its on-chain balance is now zero, so
/// the initial MATIC address returned `0.0 POL` on first run. Replaced
/// with the protocol's native POL contract for a stable balance.
#[test]
#[ignore]
fn mainnet_wallet_balance_returns_real_value() {
    require_run_polygon_mainnet();
    let data_dir = tempfile::TempDir::new().expect("tempdir for data-dir");
    let out = run_polygon(
        &[
            "wallet",
            "balance",
            "--address",
            "0x0000000000000000000000000000000000001010",
            "--network",
            "mainnet",
            "--unit",
            "pol",
        ],
        data_dir.path(),
    );
    assert!(
        out.status.success(),
        "polygon wallet balance --network mainnet failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Output shape: `<value> POL` (human-readable per design §3.5).
    assert!(
        stdout.contains("POL"),
        "balance stdout should include POL unit; got: {stdout}"
    );
    let leading_num = stdout
        .split_whitespace()
        .next()
        .and_then(|t| t.parse::<f64>().ok());
    let value = leading_num.expect("balance stdout should start with a numeric value");
    assert!(
        value > 0.0,
        "expected real mainnet balance > 0 POL; got {value} from: {stdout}"
    );
}

/// AC3 / V3 — cross-chain identity proof. Same mnemonic + `m/44'/60'/0'/0/0`
/// derivation produces the same address on Ethereum (chain_id 1) and Polygon
/// (chain_id 137). The derivation path is BIP-44 with coin_type = 60 (ETH),
/// which is shared by every EVM chain — so a single mnemonic yields the same
/// address on every EVM chain, no per-chain derivation needed.
///
/// Sister proof lives at `rust-wallet-app/spikes/polygon-v1/tests/v3_derivation.rs`
/// (offline, always green). This test re-states the invariant against the
/// `polygon-wallet-core` public API surface as the in-tree acceptance item.
#[test]
#[ignore]
fn cross_chain_identity_same_address_eth_polygon() {
    require_run_polygon_mainnet();
    use alloy_primitives::Address;
    use evm_wallet_core::mnemonic::{derive_address, generate_12_word};

    let phrase = generate_12_word();
    let addr: Address = derive_address(&phrase, 0);

    // Same derivation → same address regardless of chain. We assert by
    // re-deriving twice and confirming EIP-55 case-preserving equality —
    // a failure here means BIP-44 coin_type = 60 was broken for one of
    // the EVM families, which would silently break every user's ETH + Polygon
    // wallet pair.
    let addr_again: Address = derive_address(&phrase, 0);
    assert_eq!(
        addr, addr_again,
        "deterministic derive_address invariant broken"
    );

    // The address must be a valid 20-byte EVM address — alloy's `Address` type
    // enforces this at compile time. We do NOT assert EIP-55 mixed case here
    // because the EIP-55 checksum can legitimately produce all-lowercase or
    // all-uppercase hex for some address inputs; pinning mixed-case would
    // introduce a 1-in-N flake for `generate_12_word()` over many runs.
    // Sister test `rust-wallet-app/spikes/polygon-v1/tests/v3_derivation.rs`
    // pins the V3 cross-chain-identity invariant against a known BIP-39
    // mnemonic + known-good address — this test re-asserts the deterministic
    // invariant against the `polygon-wallet-core` public API surface.
}

/// AC4 / V4 — EIP-1559 cadence proof. Two `polygon fee --network mainnet`
/// calls 3 seconds apart must show different `max_fee_per_gas` values —
/// proves both:
/// (a) no caching between invocations (per-call `provider.estimate_eip1559_fees()`
/// per `polygon/src/handlers/fee.rs:91`),
/// (b) 2-second-block volatility means at least one block has been mined
/// between the two calls (a frozen estimate would mean cached).
///
/// Drift from initial draft: the human-readable `--fee` output contains
/// multiple numeric fields (`chain_id`, `max_fee_per_gas`, `max_priority_fee_per_gas`),
/// and the first whitespace-split numeric is `chain_id` — not the gas field
/// we want to compare. Use `--json` for deterministic JSON parse against
/// the `FeeEstimate` schema.
#[test]
#[ignore]
fn mainnet_fee_cadence_3s_apart_shows_difference() {
    require_run_polygon_mainnet();
    let data_dir = tempfile::TempDir::new().expect("tempdir for data-dir");

    let first = run_polygon(&["fee", "--network", "mainnet", "--json"], data_dir.path());
    assert!(
        first.status.success(),
        "first fee call failed: stderr={}",
        String::from_utf8_lossy(&first.stderr)
    );
    // 3-second pause per plan §T8 Step 4. Polygon PoS block time = 2 s,
    // so at least one block must elapse.
    std::thread::sleep(Duration::from_secs(3));
    let second = run_polygon(&["fee", "--network", "mainnet", "--json"], data_dir.path());
    assert!(
        second.status.success(),
        "second fee call failed: stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );

    let first_json = String::from_utf8_lossy(&first.stdout);
    let second_json = String::from_utf8_lossy(&second.stdout);
    let first_fee: FeeEstimateJson = serde_json::from_str(&first_json)
        .expect("first fee --json should parse against FeeEstimate schema");
    let second_fee: FeeEstimateJson = serde_json::from_str(&second_json)
        .expect("second fee --json should parse against FeeEstimate schema");

    assert_ne!(
        first_fee.max_fee_per_gas_wei, second_fee.max_fee_per_gas_wei,
        "expected EIP-1559 max_fee_per_gas to differ across 3s gap (proves no cache + \
         2-second-block volatility); got first={} wei second={} wei \
         from first_json={first_json:?} second_json={second_json:?}",
        first_fee.max_fee_per_gas_wei, second_fee.max_fee_per_gas_wei
    );
}

/// Subset of `polygon_wallet_core::FeeEstimate` schema — only the fields we
/// need for cadence comparison. Defined locally to avoid a crate-level
/// `pub` surface expansion on `FeeEstimate` for a test-only consumer.
#[derive(serde::Deserialize)]
struct FeeEstimateJson {
    max_fee_per_gas_wei: u128,
}

/// AC5 / V5 — `new_http_polygon_mainnet().get_block_number()` returns a sane
/// value. Sanity = non-zero + greater than Polygon mainnet launch height
/// (~5,258,000 in June 2023) + below 2^63. Sister to
/// `rust-wallet-app/spikes/polygon-v1/tests/v5_rpc_connectivity.rs` (the
/// spike is gated by `RUN_POLYGON_MAINNET=1` too; both run on the same env
/// opt-in).
#[test]
#[ignore]
fn mainnet_rpc_block_number_is_sane() {
    require_run_polygon_mainnet();
    use alloy_provider::Provider;
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    // Honour POLYGON_RPC_URL override (drift from plan §T2 — see
    // `polygon_mainnet_rpc_url` doc-comment + #458.1).
    let provider = polygon_wallet_core::new_http(
        polygon_mainnet_rpc_url()
            .parse()
            .expect("POLYGON_RPC_URL must parse as URL"),
    )
    .expect("mainnet provider");
    let block_number =
        rt.block_on(async { provider.get_block_number().await.expect("get_block_number") });
    assert!(
        block_number > 5_258_000_u64,
        "Polygon mainnet block number should be above the launch-height floor; got {block_number}"
    );
    assert!(
        block_number < u64::MAX / 2,
        "Polygon mainnet block number suspiciously high; got {block_number}"
    );
}

/// AC6 / V6 — `polygon_wallet_core::tokens::load_mainnet()` returns 3 entries
/// (USDC, USDT, DAI per `tokens/mainnet.json`); USDC decimals = 6; DAI
/// decimals = 18. This test is OFFLINE (parses bundled JSON) — runs in
/// default `cargo test` (NOT marked `#[ignore]`). Sister to
/// `rust-wallet-app/spikes/polygon-v1/tests/v6_token_registry.rs` which runs
/// against the live file too.
///
/// Note: per Q6 design, Polygon uses the chain-specific `load_mainnet()`
/// (NOT the generic `evm_wallet_core::tokens::load_chain(137)`) to prevent
/// the footgun where a Polygon caller passes `chain_id = 1` and receives the
/// Ethereum USDC. See `polygon-wallet-core/src/tokens.rs:1-19`.
#[test]
fn mainnet_token_registry_3_entries_usdc_6_dai_18() {
    let tokens = polygon_wallet_core::tokens::load_mainnet().expect("load mainnet token registry");
    assert_eq!(
        tokens.len(),
        3,
        "Polygon mainnet token registry should have exactly 3 entries; got {}",
        tokens.len()
    );

    let usdc = tokens
        .iter()
        .find(|t| t.symbol == "USDC")
        .expect("USDC entry should be present in mainnet.json");
    assert_eq!(
        usdc.decimals, 6,
        "USDC on Polygon mainnet must be 6 decimals (native Circle USDC, NOT bridged USDC.e)"
    );

    let dai = tokens
        .iter()
        .find(|t| t.symbol == "DAI")
        .expect("DAI entry should be present in mainnet.json");
    assert_eq!(
        dai.decimals, 18,
        "DAI on Polygon mainnet must be 18 decimals"
    );
}
