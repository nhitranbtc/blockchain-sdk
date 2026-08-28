//! Phase 2 Polygon RPC integration tests (Issue #424 / Task 3 of #416).
//!
//! Mirrors the polygon-v1 spike V2 (chain_id) + V5 (EIP-1559 fee estimate):
//!   * `new_http_polygon_mainnet()` provider returns chain_id == 137
//!   * `new_http_polygon_amoy()` provider returns chain_id == 80_002
//!   * `estimate_eip1559_fees()` returns a valid (max_fee_per_gas,
//!     max_priority_fee_per_gas) tuple — V5 cadence proof (re-estimate
//!     between calls rather than caching).
//!
//! Both `new_http_polygon_*` constructors in `provider.rs` are thin
//! wrappers over the existing `new_http` — they pin the public RPC URL
//! (Q4: `polygon-rpc.com` mainnet, `polygon-amoy.drpc.org` Amoy) so
//! downstream callers don't hand-roll URL strings.
//!
//! SPKI pinning for ETH/Polygon RPCs is intentionally NOT applied here.
//! Per `provider.rs` lines 9-32 (F20 M-2 remediation), the ETH-side SPKI
//! verifier was removed pending composition with `webpki` +
//! `rustls::client::WebPkiServerVerifier`. Polygon uses the same
//! transport, so the constructors rely on rustls default system CAs.
//!
//! Pattern (L29): operator-driven, never runs in CI.
//!   RUN_POLYGON_MAINNET=1 cargo test -p evm-wallet-core --test polygon_rpc -- --ignored --nocapture
//!   RUN_POLYGON_AMOY=1    cargo test -p evm-wallet-core --test polygon_rpc -- --ignored --nocapture
//!
//! No env vars required beyond the gate flags.

#![cfg(test)]
#![allow(clippy::doc_lazy_continuation)]

use alloy_provider::Provider;

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_POLYGON_MAINNET=1"]
async fn polygon_mainnet_chain_id_returns_137() {
    if std::env::var("RUN_POLYGON_MAINNET").is_err() {
        return;
    }

    let provider = evm_wallet_core::provider::new_http_polygon_mainnet()
        .expect("polygon mainnet provider should construct");

    let chain_id = provider
        .get_chain_id()
        .await
        .expect("eth_chainId against polygon-rpc.com should succeed");

    assert_eq!(chain_id, 137, "polygon mainnet must report chain_id == 137");
    eprintln!("[Phase 2 V2 PASS] polygon mainnet chain_id = {chain_id}");
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_POLYGON_AMOY=1"]
async fn polygon_amoy_chain_id_returns_80002() {
    if std::env::var("RUN_POLYGON_AMOY").is_err() {
        return;
    }

    let provider = evm_wallet_core::provider::new_http_polygon_amoy()
        .expect("polygon amoy provider should construct");

    let chain_id = provider
        .get_chain_id()
        .await
        .expect("eth_chainId against polygon-amoy.drpc.org should succeed");

    assert_eq!(
        chain_id, 80_002,
        "polygon amoy (testnet) must report chain_id == 80002"
    );
    eprintln!("[Phase 2 V2 PASS] polygon amoy chain_id = {chain_id}");
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_POLYGON_MAINNET=1"]
async fn polygon_mainnet_estimate_eip1559_fees_returns_valid_tuple() {
    if std::env::var("RUN_POLYGON_MAINNET").is_err() {
        return;
    }

    let provider = evm_wallet_core::provider::new_http_polygon_mainnet()
        .expect("polygon mainnet provider should construct");

    // V5 cadence proof: call twice in quick succession and assert both
    // succeed (no cache, no per-tx retry on transient empty blocks).
    let first = provider
        .estimate_eip1559_fees()
        .await
        .expect("first estimate_eip1559_fees against mainnet should succeed");
    let second = provider
        .estimate_eip1559_fees()
        .await
        .expect("second estimate_eip1559_fees against mainnet should succeed");

    assert!(
        first.max_fee_per_gas > 0,
        "max_fee_per_gas must be > 0 (got {})",
        first.max_fee_per_gas
    );
    assert!(
        first.max_priority_fee_per_gas > 0,
        "max_priority_fee_per_gas must be > 0 (got {})",
        first.max_priority_fee_per_gas
    );
    assert!(
        first.max_fee_per_gas >= first.max_priority_fee_per_gas,
        "max_fee_per_gas ({}) must be >= max_priority_fee_per_gas ({})",
        first.max_fee_per_gas,
        first.max_priority_fee_per_gas
    );
    assert!(
        second.max_fee_per_gas > 0,
        "second estimate: max_fee_per_gas must be > 0 (got {})",
        second.max_fee_per_gas
    );
    assert!(
        second.max_priority_fee_per_gas > 0,
        "second estimate: max_priority_fee_per_gas must be > 0 (got {})",
        second.max_priority_fee_per_gas
    );
    assert!(
        second.max_fee_per_gas >= second.max_priority_fee_per_gas,
        "second estimate: max_fee_per_gas ({}) must be >= max_priority_fee_per_gas ({})",
        second.max_fee_per_gas,
        second.max_priority_fee_per_gas
    );

    eprintln!(
        "[Phase 2 V5 PASS] polygon mainnet fees (1st): max_fee={} priority={} | (2nd): max_fee={} priority={}",
        first.max_fee_per_gas,
        first.max_priority_fee_per_gas,
        second.max_fee_per_gas,
        second.max_priority_fee_per_gas
    );
}
