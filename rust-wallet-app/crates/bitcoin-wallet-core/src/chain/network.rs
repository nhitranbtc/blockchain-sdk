//! Network selection helpers for the Bitcoin wallet core.
//!
//! Per plan §Task 8 (finding F37). The canonical network type lives
//! in `bdk_wallet::bitcoin::Network` (re-exported via
//! `bdk_wallet::bitcoin::*`); this module provides BIP-44 derivation
//! helpers that key off it.
//!
//! **Defends against:** U1-adjacent footgun where a testnet wallet is
//! handed `coin_type: u32 = 0` (the mainnet BIP-44 derivation path),
//! silently producing mainnet addresses. `coin_type_for` returns the
//! correct path from the network, never accepting a caller-supplied
//! value.
//!
//! **CONTEXT.md hard rule #1:** never default to mainnet. Match is
//! exhaustive with no wildcard — a future `bdk_wallet::bitcoin::Network`
//! variant forces a compile error here, forcing an explicit SLIP-44
//! assignment. Prevents silent-mainnet fallback on future variants.
//!
//! **Threat-model coverage:** F37 (chain sync / derivation path).

use bdk_wallet::bitcoin::Network;

/// Return the BIP-44 coin type for `n`.
///
/// Per SLIP-44:
/// - Bitcoin mainnet → `0`
/// - Bitcoin testnet / testnet4 / regtest / signet → `1`
///
/// Match is exhaustive — a future `Network` variant forces a compile
/// error so the maintainer must assign its SLIP-44 coin type
/// explicitly. Defense against the silent-mainnet footgun
/// (CONTEXT.md hard rule #1).
pub fn coin_type_for(n: Network) -> u32 {
    match n {
        Network::Bitcoin => 0,
        Network::Testnet | Network::Testnet4 | Network::Regtest | Network::Signet => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mainnet_coin_type_zero() {
        assert_eq!(coin_type_for(Network::Bitcoin), 0);
    }

    #[test]
    fn testnet_coin_type_one() {
        assert_eq!(coin_type_for(Network::Testnet), 1);
    }

    #[test]
    fn regtest_coin_type_one() {
        assert_eq!(coin_type_for(Network::Regtest), 1);
    }

    #[test]
    fn signet_coin_type_one() {
        assert_eq!(coin_type_for(Network::Signet), 1);
    }

    #[test]
    fn testnet4_coin_type_one() {
        assert_eq!(coin_type_for(Network::Testnet4), 1);
    }
}
