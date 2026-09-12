//! FFI mnemonic helpers — excluded from cbindgen header emit.
//!
//! Phase 8.1 Steps 2/3 (`sol_wallet_create_mnemonic` +
//! `sol_wallet_import_mnemonic`) need BIP-39 phrase generation +
//! validation. Per audit H7 these helpers are excluded from cbindgen
//! to keep the emitted header minimal (only the 16 `sol_wallet_*`
//! exports surface; helper internals stay in the Rust crate).

#![allow(unknown_lints, clippy::manual_is_ascii_check)]

use crate::wallet_manager::WalletManager;
use crate::{Error, Result, WalletId};
use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a fresh 12-word BIP-39 English mnemonic from OS entropy.
pub fn generate_12_word_english() -> Result<String> {
    let mnemonic = bip39::Mnemonic::generate_in(bip39::Language::English, 12)
        .map_err(|_e| Error::Unimplemented("BIP-39 generate failed"))?;
    Ok(mnemonic.to_string())
}

/// Validate a BIP-39 English mnemonic phrase (parse-only).
pub fn validate_english(phrase: &str) -> Result<()> {
    bip39::Mnemonic::parse_in(bip39::Language::English, phrase)
        .map(|_| ())
        .map_err(|_| Error::Unimplemented("invalid BIP-39 phrase"))
}

/// Current Unix epoch in seconds.
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Generate a fresh wallet id (Uuid v4).
pub fn fresh_wallet_id() -> Result<WalletId> {
    WalletId::new()
}

/// Helper: import a freshly-generated or caller-supplied mnemonic into
/// the wallet manager. Returns the new wallet id.
pub fn import_into_manager<S: crate::platform::WalletStorage + 'static>(
    mgr: &WalletManager<S>,
    phrase: &str,
    password: &str,
    name: &str,
    account: u32,
    address_index: u32,
) -> Result<WalletId> {
    mgr.create_with_mnemonic(phrase, password, name, account, address_index, now_unix())
}
