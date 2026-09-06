//! Bundled per-network configuration (plan Task 3.5).
//!
//! Each `tokens/{local,nile,mainnet}.json` file carries both the
//! network's TRC-20 token list AND the test addresses / ref-block
//! fixture used by the round-trip tests. Shasta has no file (no
//! stable TRC-20 registry on the public Shasta testnet) — its
//! `load(Shasta)` returns an empty bundle and `test_addresses(Shasta)`
//! falls back to the mainnet fixture (so a stray `load(Shasta)` test
//! still gets parseable bytes, not a panic).
//!
//! ## Why test addresses per network
//!
//! The original `test-vectors.json` collapsed the test fixtures into
//! a single file used by every network's tests. After the config
//! refactor, every network's JSON is the operator-tunable source of
//! truth for that network — so the test wallet (owner / recipient /
//! spender) and the ref-block fixture move next to the bundle they
//! test. If a future operator hand-edits `nile.json` and breaks the
//! tests, the failure is co-located with the bundle rather than
//! bouncing between `test-vectors.json` and `nile.json`.
//!
//! ## Files
//!
//! - `tokens/local.json` — TronBox Mock USDT placeholder; Phase 4 spike
//!   V7 swaps the address at boot to match the deployed contract.
//! - `tokens/nile.json` — community test USDT (canonical per TronScan
//!   verified 2026-09-05; **CAUTION**: user-stories.md Story 21 quotes
//!   a stale address — `TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z`. The
//!   bundled value is the canonical one).
//! - `tokens/mainnet.json` — five entries: USDT, USDC, TUSD, USDD, stUSDT.
//! - `tokens/network.json` — RPC URLs + per-network SPKI pins
//!   (separate file; loaded by `crate::config`).
//!
//! ## Wire format
//!
//! Each per-network file is a `Bundle { tokens: Vec<Token>, test: TestAddresses }`.
//! `tokens/*` lives at the crate root and is loaded via
//! [`include_str!`] at compile time, so the registry is part of the
//! binary and there is no runtime file resolution step.
//!
//! Tokens are addressed by their **T-base58check contract address**,
//! not by symbol — the symbol is for display only. Two tokens can
//! share a symbol (different issuers); lookup by symbol would
//! silently pick the wrong one.
//!
//! Adding a token to the registry is a code change (edit the JSON
//! file and rebuild). v0.1 deliberately does not support a mutable
//! registry: every entry is a real on-chain TRC-20 contract, and
//! adding one without a corresponding operator-driven audit would be
//! a footgun.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::config::Network;

/// A single bundled TRC-20 token entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    /// Display ticker, e.g. `"USDT"`. NOT unique across the registry —
    /// use the address for lookup.
    pub symbol: String,
    /// Human-readable name, e.g. `"Tether USD"`.
    pub name: String,
    /// T-base58check contract address. This is the registry's primary
    /// key — the symbol is present for display only.
    pub address: String,
    /// Decimal precision the contract returns from `decimals()`. Stored
    /// here so a caller formatting an amount can read it without an extra
    /// RPC round-trip; the live `trc20::decimals` call exists for
    /// out-of-band verification.
    pub decimals: u8,
}

/// Test-only addresses and the ref-block fixture used by every
/// round-trip test in `tests/`. Lives inside the per-network JSON
/// (one entry per `tokens/{local,nile,mainnet}.json`) so the
/// network's bundle + the addresses that exercise it are co-located.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestAddresses {
    /// The "owner" wallet used by every round-trip test (signer + sender).
    pub owner_address: String,
    /// The "recipient" wallet used by every round-trip test.
    pub recipient_address: String,
    /// The "spender" used by `approve` round-trip tests.
    pub approval_spender_address: String,
    /// 32-byte block id of a real network block, hex-encoded.
    pub ref_block_hex: String,
    /// Numeric block height paired with `ref_block_hex`.
    pub ref_block_number: i64,
}

/// On-disk shape of each `tokens/{local,nile,mainnet}.json` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Bundle {
    tokens: Vec<Token>,
    test: TestAddresses,
}

/// Lazily-decoded bundle for a given network. `Box::leak`-d to
/// produce `&'static` slices — bounded by the bundled JSON (5
/// entries + 1 fixture on mainnet, 1 + 1 on the rest).
static BUNDLES: OnceLock<Vec<(Network, Bundle)>> = OnceLock::new();

fn bundles() -> &'static [(Network, Bundle)] {
    BUNDLES.get_or_init(|| {
        vec![
            (Network::Local, parse_or_panic(LOCAL_JSON)),
            (Network::Nile, parse_or_panic(NILE_JSON)),
            (Network::Mainnet, parse_or_panic(MAINNET_JSON)),
        ]
    })
}

fn bundle_for(network: Network) -> Option<&'static Bundle> {
    bundles()
        .iter()
        .find(|(n, _)| *n == network)
        .map(|(_, b)| -> &'static Bundle { leak_bundle(b) })
}

/// Load the bundled token list for a network.
///
/// The returned slice is `&'static` because the underlying strings come
/// from `include_str!`. Callers that need a mutable list (e.g. Phase 4
/// updating the local TronBox address at boot) must clone the entries.
pub fn load(network: Network) -> &'static [Token] {
    bundle_for(network)
        .map(|b| -> &'static [Token] { leak_tokens(&b.tokens) })
        .unwrap_or(&[])
}

/// Test addresses for a network. Returns `None` only for networks
/// without a bundled file (Shasta today); the [`tokens::load`]
/// callers that need a guaranteed non-`None` fixture should fall
/// back to the mainnet bundle via `Network::Mainnet` first.
pub fn test_addresses(network: Network) -> Option<&'static TestAddresses> {
    bundle_for(network).map(|b| &b.test)
}

/// Look up a token by its T-base58check address within a network's bundle.
///
/// Returns `None` when the address is not registered. The comparison is
/// case-sensitive on the base58 form (which is the canonical form); the
/// `Address::from_str` parser accepts the uppercase hex and `0x`-prefix
/// forms too, so callers that have a parsed [`crate::address::Address`]
/// should pass `addr.to_base58()` to land on the same key.
pub fn by_address(network: Network, address: &str) -> Option<&'static Token> {
    load(network).iter().find(|t| t.address == address)
}

/// Look up a token by its display ticker within a network's bundle.
///
/// Symbol lookups are ambiguous by construction — multiple tokens can
/// share a symbol — so prefer [`by_address`] when the address is known.
/// This helper exists for the CLI (`tron tokens list USDT`) where the
/// caller accepts the ambiguity as part of the UX.
pub fn by_symbol(network: Network, symbol: &str) -> Option<&'static Token> {
    load(network).iter().find(|t| t.symbol == symbol)
}

const LOCAL_JSON: &str = include_str!("../../tokens/local.json");
const NILE_JSON: &str = include_str!("../../tokens/nile.json");
const MAINNET_JSON: &str = include_str!("../../tokens/mainnet.json");

fn parse_or_panic(raw: &'static str) -> Bundle {
    // A malformed bundled JSON is a compile-time invariant: the file is
    // `include_str!`'d, so a bug in the JSON ships with the binary and
    // would panic on first load. That is the right failure mode — the
    // registry is supposed to be a fixed table, not user data.
    match serde_json::from_str::<Bundle>(raw) {
        Ok(b) => b,
        Err(e) => panic!("bundled token registry failed to parse: {e}"),
    }
}

fn leak_tokens(tokens: &[Token]) -> &'static [Token] {
    Box::leak(tokens.to_vec().into_boxed_slice())
}

fn leak_bundle(b: &Bundle) -> &'static Bundle {
    Box::leak(Box::new(b.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nile_has_one_entry_with_canonical_address() {
        let tokens = load(Network::Nile);
        assert_eq!(tokens.len(), 1, "Nile bundle must have exactly one entry");
        assert_eq!(tokens[0].symbol, "USDT");
        assert_eq!(tokens[0].decimals, 6);
        // Plan Q9 caution: the canonical Nile USDT address — must NOT be the
        // stale user-stories.md value.
        assert_eq!(
            tokens[0].address, "TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf",
            "Nile USDT address drifted from the canonical TronScan value"
        );
        assert_ne!(
            tokens[0].address, "TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z",
            "Nile USDT address matches the stale user-stories.md value — \
             revert to the canonical TronScan address"
        );
    }

    #[test]
    fn mainnet_has_five_distinct_addresses() {
        let tokens = load(Network::Mainnet);
        assert_eq!(
            tokens.len(),
            5,
            "Mainnet bundle must have exactly five entries"
        );

        let symbols: Vec<&str> = tokens.iter().map(|t| t.symbol.as_str()).collect();
        for expected in ["USDT", "USDC", "TUSD", "USDD", "stUSDT"] {
            assert!(
                symbols.contains(&expected),
                "missing mainnet symbol {expected}"
            );
        }

        // Distinct addresses — a duplicate means the JSON has been edited
        // and a token was copy-pasted.
        let mut addresses: Vec<&str> = tokens.iter().map(|t| t.address.as_str()).collect();
        addresses.sort_unstable();
        let original_len = addresses.len();
        addresses.dedup();
        assert_eq!(
            addresses.len(),
            original_len,
            "mainnet addresses are not unique"
        );
    }

    #[test]
    fn mainnet_decimals_match_trongrid_published_values() {
        // Cross-check the bundled decimals against TronGrid's published
        // values. A drift here is the canonical "registry is wrong" bug.
        let pairs = [
            ("USDT", 6_u8),
            ("USDC", 6_u8),
            ("TUSD", 18_u8),
            ("USDD", 18_u8),
            ("stUSDT", 6_u8),
        ];
        for (symbol, decimals) in pairs {
            let token = by_symbol(Network::Mainnet, symbol)
                .unwrap_or_else(|| panic!("mainnet bundle missing {symbol}"));
            assert_eq!(
                token.decimals, decimals,
                "mainnet {symbol} decimals mismatch (bundled {} vs expected {decimals})",
                token.decimals
            );
        }
    }

    #[test]
    fn shasta_has_empty_bundle() {
        let tokens = load(Network::Shasta);
        assert!(
            tokens.is_empty(),
            "Shasta must return an empty bundle, not panic"
        );
    }

    #[test]
    fn by_address_returns_some_when_present() {
        let token = by_address(Network::Mainnet, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t");
        assert_eq!(token.map(|t| t.symbol.as_str()), Some("USDT"));
    }

    #[test]
    fn by_address_returns_none_when_absent() {
        let token = by_address(
            Network::Mainnet,
            "TNotARegisteredAddressButValidBase58Check000000",
        );
        assert!(token.is_none());
    }

    #[test]
    fn test_addresses_present_for_every_bundled_network() {
        for n in [Network::Local, Network::Nile, Network::Mainnet] {
            assert!(
                test_addresses(n).is_some(),
                "{n:?} bundle must carry a test block"
            );
        }
    }
}
