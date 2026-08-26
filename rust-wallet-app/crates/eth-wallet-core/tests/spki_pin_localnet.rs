//! User Story 29 — Connect to RPC endpoint without SPKI pin (system CAs only).
//!
//! Story 28 (SPKI pin) tests live in the issue #393 PR — that work adds
//! `new_http_pinned()` to `eth_wallet_core::provider` and a `pinned://` URL
//! scheme. Until then, this file covers the **no-pin** path only.
//!
//! Per L29 + Q8: live network smoke is operator-driven — Anvil tests are
//! `#[ignore]` (NEVER runs in CI) and require `RUN_ANVIL_E2E=1` to execute.
//! Always-on tests (no network I/O) confirm `new_http` constructs without
//! a SPKI pin and that the bundled registry parses for the chain-id guards.

use alloy_node_bindings::Anvil;
use alloy_provider::Provider;

use eth_wallet_core::provider::new_http;

// ---------------------------------------------------------------------------
// Always-on tests (no network I/O)
// ---------------------------------------------------------------------------

/// `new_http` must accept a plain HTTP localhost URL without requiring a
/// SPKI pin. This is the Story 29 baseline — the no-pin path must keep
/// working for Anvil / LAN / trusted-network use.
#[test]
fn new_http_accepts_localhost_http_without_spki_pin() {
    let url: reqwest::Url = "http://127.0.0.1:8545"
        .parse()
        .expect("localhost URL parses");
    let _provider = new_http(url).expect("new_http constructs without SPKI pin");
}

/// Sanity check on the bundled token registry that the chain-id guard in
/// `handlers::wallet_send_native` (Story 10 + Story 26) compares against:
/// chain-id 31337 must resolve to the Anvil stub registry (which is empty
/// in v0.3 — the guard is about the *number*, not the registry contents).
#[test]
fn chain_id_31337_resolves_to_anvil_stub_registry() {
    use eth_wallet_core::tokens::load_chain;
    let anvil_tokens = load_chain(31337).expect("anvil JSON parses");
    assert!(
        anvil_tokens.is_empty(),
        "anvil registry is intentionally empty (no bundled tokens for local dev)"
    );
}

// ---------------------------------------------------------------------------
// L29-gated Anvil smoke tests (operator-driven, set RUN_ANVIL_E2E=1)
// ---------------------------------------------------------------------------

/// Story 29 happy path — `eth --rpc-url http://127.0.0.1:8545` connects
/// to a locally-spawned Anvil via `new_http`, no SPKI pin required, and
/// the chain-id guard (`handlers.rs:650-659`) sees `31337` as expected.
///
/// Run with:
/// ```bash
/// RUN_ANVIL_E2E=1 cargo test --test spki_pin_localnet -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn no_pin_localhost_anvil_succeeds() {
    if std::env::var("RUN_ANVIL_E2E").ok().as_deref() != Some("1") {
        eprintln!("[spki-pin-localnet] SKIP — set RUN_ANVIL_E2E=1 to run");
        return;
    }

    let anvil = Anvil::new().spawn();
    let endpoint: reqwest::Url = anvil.endpoint().parse().expect("valid Anvil endpoint URL");
    eprintln!("[spki-pin-localnet] spawned Anvil at {endpoint} (no SPKI pin)");

    let provider = new_http(endpoint).expect("new_http without SPKI pin");
    let chain_id = provider.get_chain_id().await.expect("chain id RPC call");
    assert_eq!(
        chain_id, 31337,
        "anvil default chain id is 31337 (0x7a69); chain-id guard would pass"
    );

    let block_number = provider.get_block_number().await.expect("block number");
    eprintln!(
        "[spki-pin-localnet] no-pin get_chain_id -> {chain_id}, \
         get_block_number -> {block_number}"
    );
}

/// Story 29 negative path — Anvil spawned with a non-default chain id
/// proves the library does NOT enforce the chain-id guard; that's the
/// handler's job (`handlers.rs:650-659`). The library returns whatever
/// the RPC reports, and the handler decides whether to reject.
///
/// This guards against an accidental library-level chain-id assertion
/// (which would be wrong — the library should be chain-agnostic; the
/// policy lives in the CLI handler).
#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn no_pin_localhost_anvil_with_custom_chain_id_returns_custom_value() {
    if std::env::var("RUN_ANVIL_E2E").ok().as_deref() != Some("1") {
        eprintln!("[spki-pin-localnet] SKIP — set RUN_ANVIL_E2E=1 to run");
        return;
    }

    let custom_chain_id: u64 = 999;
    let anvil = Anvil::new().chain_id(custom_chain_id).spawn();
    let endpoint: reqwest::Url = anvil.endpoint().parse().expect("valid Anvil endpoint URL");
    eprintln!(
        "[spki-pin-localnet] spawned Anvil at {endpoint} \
         with chain_id={custom_chain_id} (no SPKI pin)"
    );

    let provider = new_http(endpoint).expect("new_http without SPKI pin");
    let reported_chain_id = provider.get_chain_id().await.expect("chain id RPC call");
    assert_eq!(
        reported_chain_id, custom_chain_id,
        "library returns the chain id the RPC reports — \
         no implicit chain-id enforcement at the library layer"
    );
    eprintln!(
        "[spki-pin-localnet] no-pin get_chain_id -> {reported_chain_id} \
         (handler-level guard would reject this against --network anvil)"
    );
}
