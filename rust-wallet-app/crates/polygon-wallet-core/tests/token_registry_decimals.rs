//! `tokens` tests — Phase 3 Task 4 of #425 (sub-task of #416).
//!
//! V6 mirror: tokens/mainnet.json 3 entries load + USDC decimals = 6 + DAI
//! decimals = 18 verified. Decimals are read from the bundled JSON
//! (`Token.decimals`) per Q5 of the plan — decimals are cached at
//! registry load time, not query-ed per balance call.

use polygon_wallet_core::tokens::{load_amoy, load_mainnet};

#[test]
fn load_mainnet_returns_three_tokens() {
    let tokens = load_mainnet().expect("mainnet JSON must parse");
    let symbols: Vec<&str> = tokens.iter().map(|t| t.symbol.as_str()).collect();
    assert_eq!(
        tokens.len(),
        3,
        "mainnet.json must contain exactly 3 tokens; got {tokens:?}"
    );
    assert!(symbols.iter().any(|s| s.eq_ignore_ascii_case("USDC")));
    assert!(symbols.iter().any(|s| s.eq_ignore_ascii_case("USDT")));
    assert!(symbols.iter().any(|s| s.eq_ignore_ascii_case("DAI")));
}

#[test]
fn load_amoy_returns_one_token() {
    let tokens = load_amoy().expect("amoy JSON must parse");
    let symbols: Vec<&str> = tokens.iter().map(|t| t.symbol.as_str()).collect();
    assert_eq!(
        tokens.len(),
        1,
        "amoy.json must contain exactly 1 token; got {tokens:?}"
    );
    assert!(
        symbols.iter().any(|s| s.eq_ignore_ascii_case("USDC")),
        "amoy.json must include USDC; got: {symbols:?}"
    );
    let usdc = tokens
        .iter()
        .find(|t| t.symbol.eq_ignore_ascii_case("USDC"))
        .expect("USDC amoy entry must exist");
    assert_eq!(
        usdc.decimals, 6,
        "USDC amoy decimals must be 6 per EIP-20 + Circle docs"
    );
    assert_eq!(usdc.chain_id, 80002, "USDC amoy must be on chain 80002");
}

#[test]
fn usdc_mainnet_decimals_is_6() {
    let tokens = load_mainnet().expect("mainnet JSON must parse");
    let usdc = tokens
        .iter()
        .find(|t| t.symbol.eq_ignore_ascii_case("USDC"))
        .expect("USDC mainnet entry must exist");
    assert_eq!(
        usdc.decimals, 6,
        "USDC mainnet decimals must be 6 per EIP-20 + Circle docs"
    );
    assert_eq!(usdc.chain_id, 137, "USDC mainnet must be on chain 137");
}

#[test]
fn dai_mainnet_decimals_is_18() {
    let tokens = load_mainnet().expect("mainnet JSON must parse");
    let dai = tokens
        .iter()
        .find(|t| t.symbol.eq_ignore_ascii_case("DAI"))
        .expect("DAI mainnet entry must exist");
    assert_eq!(dai.decimals, 18, "DAI mainnet decimals must be 18");
    assert_eq!(dai.chain_id, 137, "DAI mainnet must be on chain 137");
}
