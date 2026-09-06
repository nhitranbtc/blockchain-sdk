//! BIP-39 mnemonics and BIP-32 key derivation, wrapping `anychain-kms`.
//!
//! Plan: Phase 1 Tasks 1.1, 1.2, and 1.6.
//!
//! # Secret hygiene, and its limits
//!
//! `anychain-kms` handles more of this than the plan's Risk Register assumed.
//! `bip39::Mnemonic` stores its phrase and entropy in `Zeroizing`, and
//! `bip39::Seed` has an explicit `Drop` that zeroes its bytes.
//!
//! What it does not cover is the derived secret. `ExtendedPrivateKey` holds a
//! `libsecp256k1::SecretKey`, and neither type has a zeroizing `Drop`. So the
//! boundary this module draws is: extended keys are built, read once, and
//! dropped inside a single function, and the only secret that outlives a call
//! is the 32-byte scalar in [`KeyPair`], wrapped in `Zeroizing`.
//!
//! Be precise about what that buys. Dropping the `XprvSecp256k1` does not wipe
//! the scalar it held — those bytes stay in the function's stack frame until
//! something else reuses the memory. The same is true inside
//! `anychain_kms::secp256k1_sign`, which copies the secret into its own
//! `SecretKey` before signing. Closing either window means forking
//! `anychain-kms` or hand-rolling BIP-32, neither of which is in v0.1 scope.
//!
//! So the accurate claim is: every secret **this crate holds** is zeroed when
//! dropped. It is not that the scalar exists nowhere else in the process.

mod derivation;
mod mnemonic;
mod xpub;

pub use derivation::{derive_keypair, KeyPair, SECRET_KEY_LEN};
pub use mnemonic::{Mnemonic, SEED_LEN};
pub use xpub::xpub;

// Re-exported so callers can name a language, word count, or path without
// depending on `anychain-kms` directly — the same reason our errors carry
// strings rather than upstream error types.
pub use anychain_kms::bip32::DerivationPath;
pub use anychain_kms::bip39::{Language, MnemonicType};

/// SLIP-44 registered coin type for TRON.
pub const TRON_COIN_TYPE: u32 = 195;

/// Default BIP-44 account-0 receive path, `m/44'/195'/0'/0/0`.
pub const DEFAULT_DERIVATION_PATH: &str = "m/44'/195'/0'/0/0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_uses_the_registered_coin_type() {
        assert!(DEFAULT_DERIVATION_PATH.contains(&format!("/{TRON_COIN_TYPE}'/")));
    }

    #[test]
    fn default_path_parses() {
        let path: DerivationPath = DEFAULT_DERIVATION_PATH.parse().expect("valid path");
        assert_eq!(path.to_string(), DEFAULT_DERIVATION_PATH);
    }
}
