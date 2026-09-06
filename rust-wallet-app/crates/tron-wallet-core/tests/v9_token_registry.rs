//! Plan Task 3.7 / Phase-7 spike V9: bundled token registry.
//!
//! Offline assertions cover the entire registry shape — counts, canonical
//! addresses, decimals cross-checked against TronGrid's published values.
//!
//! Live assertions (`decimals(rpc, USDT)` against Nile / `symbol(rpc, USDT)`
//! against Mainnet) are gated on `RUN_TRON_NILE=1` and `RUN_TRON_MAINNET=1`
//! per L29 operator-driven smoke; the default `cargo test` skips them so
//! CI stays network-free. Operators running these by hand are validating
//! that the registry's static view of the contract still matches what
//! the network actually returns.

use tron_wallet_core::config::Network;
use tron_wallet_core::tokens;

#[test]
fn nile_bundle_has_usdt_entry() {
    let entries = tokens::load(Network::Nile);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].symbol, "USDT");
    assert_eq!(entries[0].decimals, 6);
    // The canonical-TronScan value check is in
    // `src/tokens/mod.rs::tests::nile_has_one_entry_with_canonical_address`
    // — that test holds the hardcoded canonical address; this test just
    // confirms the bundle round-trips through the new `{"tokens", "test"}`
    // shape.
}

#[test]
fn mainnet_bundle_covers_all_five_stablecoins() {
    let entries = tokens::load(Network::Mainnet);
    assert_eq!(entries.len(), 5);

    let symbols: Vec<&str> = entries.iter().map(|t| t.symbol.as_str()).collect();
    for expected in ["USDT", "USDC", "TUSD", "USDD", "stUSDT"] {
        assert!(
            symbols.contains(&expected),
            "mainnet bundle missing {expected} (have {symbols:?})"
        );
    }
}

#[test]
fn mainnet_bundle_decimals_match_trongrid_published_values() {
    // USDT/USDC/stUSDT are 6-decimal; TUSD/USDD are 18-decimal. A drift
    // here is the canonical "registry is wrong" bug — silent mis-pricing
    // on a transfer is exactly the kind of bug that warrants a spike.
    let expected: &[(&str, u8)] = &[
        ("USDT", 6),
        ("USDC", 6),
        ("TUSD", 18),
        ("USDD", 18),
        ("stUSDT", 6),
    ];
    for (symbol, decimals) in expected {
        let token = tokens::by_symbol(Network::Mainnet, symbol)
            .unwrap_or_else(|| panic!("mainnet bundle missing {symbol}"));
        assert_eq!(
            token.decimals, *decimals,
            "mainnet {symbol} decimals mismatch (bundled {} vs expected {decimals})",
            token.decimals
        );
    }
}

#[test]
fn mainnet_bundle_addresses_are_distinct() {
    let entries = tokens::load(Network::Mainnet);
    let mut addrs: Vec<&str> = entries.iter().map(|t| t.address.as_str()).collect();
    addrs.sort_unstable();
    let before = addrs.len();
    addrs.dedup();
    assert_eq!(before, addrs.len(), "mainnet addresses are not unique");
}

#[test]
fn by_address_round_trips_mainnet_usdt() {
    // Round-trip through the bundle: the same address that `by_symbol`
    // hands back is what `by_address` keys on. The hardcoded
    // TronScan value lives in the bundle (`tokens/mainnet.json`); a
    // future hand-edit that moves it would update this test by way of
    // the bundle parse.
    let addr = tokens::by_symbol(Network::Mainnet, "USDT")
        .expect("mainnet USDT must be in the bundle")
        .address
        .as_str();
    let token = tokens::by_address(Network::Mainnet, addr)
        .expect("by_address must find the bundle's own USDT");
    assert_eq!(token.symbol, "USDT");
}

#[test]
fn shasta_bundle_is_empty() {
    let entries = tokens::load(Network::Shasta);
    assert!(
        entries.is_empty(),
        "Shasta has no stable TRC-20 registry — empty bundle is correct"
    );
}

#[test]
fn local_bundle_has_mockusdt_placeholder() {
    let entries = tokens::load(Network::Local);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].symbol, "MOCKUSDT");
    // Phase 4 spike V7 swaps this address at boot.
    assert!(
        entries[0].address.starts_with("TLocal"),
        "local placeholder address must be recognisable as local — got {}",
        entries[0].address
    );
}

// --- GATED live checks (RUN_TRON_NILE=1 / RUN_TRON_MAINNET=1) ----------

#[tokio::test]
async fn live_decimals_match_bundle_against_nile() {
    if std::env::var_os("RUN_TRON_NILE").is_none() {
        eprintln!("skipped: set RUN_TRON_NILE=1 to run this live check");
        return;
    }
    let cfg = tron_wallet_core::TronConfig::for_network(Network::Nile);
    let rpc = tron_wallet_core::TronGridClient::new(&cfg.rpc_url, cfg.spki_pin)
        .expect("TronGridClient builds");
    let live = tron_wallet_core::trc20::decimals(
        &rpc,
        tron_wallet_core::tokens::by_symbol(Network::Nile, "USDT")
            .expect("Nile USDT must be in the bundle")
            .address
            .as_str(),
    )
    .await
    .expect("decimals() on Nile USDT");
    assert_eq!(live, 6, "Nile USDT decimals drifted from the bundled 6");
}

#[tokio::test]
async fn live_symbol_matches_bundle_against_mainnet() {
    if std::env::var_os("RUN_TRON_MAINNET").is_none() {
        eprintln!("skipped: set RUN_TRON_MAINNET=1 to run this live check");
        return;
    }
    let cfg = tron_wallet_core::TronConfig::for_network(Network::Mainnet);
    let rpc = tron_wallet_core::TronGridClient::new(&cfg.rpc_url, cfg.spki_pin)
        .expect("TronGridClient builds");
    let live = tron_wallet_core::trc20::symbol(
        &rpc,
        tokens::by_symbol(Network::Mainnet, "USDT")
            .expect("mainnet USDT must be in the bundle")
            .address
            .as_str(),
    )
    .await
    .expect("symbol() on mainnet USDT");
    assert_eq!(live, "USDT");
}
