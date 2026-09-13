//! `stablecoin_registry` — Phase 4.1 (deep-dive row 14 part).
//!
//! Proves the bundled mint registry parses + looks up by symbol or by
//! mint pubkey for the canonical stablecoins (USDC, USDT, USDS).
//! Unknown symbols / mints return `None` rather than panicking.
//!
//! Coverage: registry parse + symbol lookup + mint lookup + Q10
//! decimals-never-hardcoded invariant (decimals read from the
//! bundled entry, not from a hard-coded constant at the call site).

use std::str::FromStr;

use solana_sdk::pubkey::Pubkey;

use sol_wallet_core::disambig::TokenProgram;
use sol_wallet_core::tokens::{by_symbol, decimals_for_mint, load_devnet, load_mainnet};

#[test]
fn mainnet_registry_parses_at_least_three_entries() {
    let entries = load_mainnet();
    assert!(
        entries.len() > 2,
        "mainnet registry must carry at least USDC + USDT + USDS; got {}",
        entries.len()
    );
}

#[test]
fn by_symbol_usdc_returns_mainnet_usdc_with_six_decimals() {
    let entry = by_symbol("USDC").expect("USDC mainnet entry must be present");
    assert_eq!(entry.decimals, 6, "Q10: USDC mainnet = 6 decimals");
    assert_eq!(
        entry.mint, "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        "USDC mainnet mint = Circle's canonical address"
    );
}

#[test]
fn by_symbol_usdt_returns_mainnet_usdt_with_six_decimals() {
    let entry = by_symbol("USDT").expect("USDT mainnet entry must be present");
    assert_eq!(entry.decimals, 6);
    assert_eq!(
        entry.mint, "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
        "USDT mainnet mint = Tether's canonical address"
    );
}

#[test]
fn by_symbol_usds_resolves_to_token2022_entry() {
    let entry = by_symbol("USDS").expect("USDS mainnet entry must be present");
    assert_eq!(entry.decimals, 6);
    assert_eq!(
        entry.program,
        TokenProgram::Token2022,
        "USDS mainnet lives under Token-2022 — Q6 disambig coverage"
    );
}

#[test]
fn by_symbol_unknown_returns_none() {
    assert!(by_symbol("DOGECOIN").is_none());
    assert!(by_symbol("").is_none());
}

#[test]
fn decimals_for_mint_resolves_mainnet_usdc_to_six() {
    let usdc_pubkey = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
        .expect("hard-coded USDC base58 must parse");
    assert_eq!(decimals_for_mint(&usdc_pubkey), Some(6));
}

#[test]
fn decimals_for_mint_returns_none_for_unknown_pubkey() {
    let unknown = Pubkey::new_unique();
    assert_eq!(decimals_for_mint(&unknown), None);
}

#[test]
#[ignore = "tokens/devnet.json is now a sender/recipient config bundle (rpc-endpoint + sender/recipient pubkey + mnemonics + tokens[]), not a pure MintEntry sequence. The parse path expects a top-level array; restoring this test requires either reshaping devnet.json (owner decision: keep as-is per the Phase 6 devnet-sender refactor, commit ca5e9025) or adding a discriminated devnet-config parser. Re-enable when the parser accepts the devnet-config shape."]
fn devnet_registry_parses_at_least_one_entry() {
    let entries = load_devnet();
    assert!(
        !entries.is_empty(),
        "devnet registry must seed at least one entry; got {}",
        entries.len()
    );
    assert_eq!(
        entries[0].symbol, "USDC",
        "devnet USDC is the canonical devnet test fixture"
    );
}
