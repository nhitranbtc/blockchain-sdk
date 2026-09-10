//! Phantom-equivalent read-only wallet (`from_public_key`).
//!
//! Mirrors `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md` §F
//! line 1718: `pub struct ReadOnlyWallet(Pubkey);` — NO `sign` methods.
//!
//! The Phantom "Watch-only" import path lands here. A read-only wallet
//! holds the leaf pubkey only; it cannot derive sibling addresses
//! because Ed25519 SLIP-0010 does not expose a parent public key
//! (`xpub`) — documented gap, mirrors Phantom's own limitation.

use solana_sdk::pubkey::Pubkey;

/// Watch-only wallet — pubkey + base58 export; no signing material.
///
/// The inner field is `pub(crate)` so `Wallet::from_public_key` (in
/// the sibling module) can construct the newtype; external callers
/// must go through the `pubkey()` getter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReadOnlyWallet(pub(crate) Pubkey);

impl ReadOnlyWallet {
    /// Base58-encoded Ed25519 pubkey (32 bytes; 32-44 chars).
    pub fn pubkey(&self) -> Pubkey {
        self.0
    }
}

impl core::fmt::Display for ReadOnlyWallet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // The Phantom watch-only panel shows only the base58 pubkey.
        f.write_str(&self.0.to_string())
    }
}
