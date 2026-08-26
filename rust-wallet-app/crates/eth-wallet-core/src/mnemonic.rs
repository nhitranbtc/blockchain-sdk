//! Mnemonic generation + BIP-44 derivation for Ethereum wallets.
//!
//! Mirrors Bitcoin wallet-core path; identical BIP-39/BIP-32 primitives, only
//! the coin type differs (`60' = ETH` per SLIP-44).
//!
//! Issue #295 + Plan Task 1.

use alloy_primitives::Address;
use alloy_signer_local::MnemonicBuilder;
use bip39::{Language, Mnemonic};
use zeroize::Zeroizing;

/// Generate a fresh 12-word BIP-39 mnemonic (English wordlist).
///
/// Wrapped in `Zeroizing<Mnemonic>` so the entropy buffer is wiped on drop
/// per F47 zeroize treatment (BTC Task 30 mirror).
pub fn generate_12_word() -> Zeroizing<Mnemonic> {
    Zeroizing::new(
        Mnemonic::generate_in(Language::English, 12)
            .expect("12-word BIP-39 generation is non-fallible in bip39 2.2"),
    )
}

/// Derive the EIP-55 Address at `m/44'/60'/0'/0/<index>` (Ledger-style path
/// per Q3 resolution).
///
/// `phrase` is the BIP-39 mnemonic; `index` is the address-slot index
/// (BIP-44 5th position).
pub fn derive_address(phrase: &Mnemonic, index: u32) -> Address {
    MnemonicBuilder::english()
        .phrase(phrase.to_string().as_str())
        .index(index)
        .expect("valid account index")
        .build()
        .expect("mnemonic build")
        .address()
}
