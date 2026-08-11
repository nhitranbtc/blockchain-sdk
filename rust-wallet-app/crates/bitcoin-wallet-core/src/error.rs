//! bitcoin-wallet-core error type.
//!
//! Per-plan §Task 2 + drift closures:
//! - From Task 1.5: `From<io::Error>` so call sites of `refuse_world_writable`
//!   (which returns `io::Result`) can use `?` without `map_err` boilerplate.
//! - From Task 1.5 review: `Bdk(String)` (not `#[from] bdk_wallet::Error`)
//!   because bdk_wallet 3.1 has no single unified error type — it has
//!   `wallet::LoadError`, `descriptor::Error`, `wallet::CreateTxError`, etc.
//!   Call sites wrap the relevant bdk error at the boundary.
//! - Post-review: `#[non_exhaustive]` so adding variants in v0.2 (per
//!   threat-model roadmap) doesn't break downstream `match` arms in
//!   the `btc` CLI.

use thiserror::Error;

/// All errors returned by `bitcoin-wallet-core` public APIs.
///
/// Variants mapped to threat-model U-cases (per L10):
/// - `InvalidMnemonic`, `InvalidDerivationPath`, `AddressDerivation`,
///   `ScriptBuild` — key generation failures
/// - `Network`, `Esplora`, `Electrum` — F20 / A3 / A4 coverage
/// - `InsufficientFunds`, `TxBuild`, `Sign`, `Psbt` — F25 / U1 coverage
/// - `Storage`, `NotInitialized` — F19 / U6 / U7 coverage
/// - `Encryption` — F5 / F6 / U3 coverage (deferred `mlock` to v0.2)
/// - `Bitcoin`, `Bdk`, `Io` — upstream library errors
#[allow(missing_docs)]
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),

    #[error("invalid derivation path: {0}")]
    InvalidDerivationPath(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("esplora error: {0}")]
    Esplora(String),

    #[error("electrum error: {0}")]
    Electrum(String),

    #[error("insufficient funds: needed {needed} sat, have {available} sat")]
    InsufficientFunds { needed: u64, available: u64 },

    #[error("transaction build error: {0}")]
    TxBuild(String),

    #[error("signing error: {0}")]
    Sign(String),

    #[error("psbt error: {0}")]
    Psbt(String),

    #[error("address derivation error: {0}")]
    AddressDerivation(String),

    #[error("script build error: {0}")]
    ScriptBuild(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("not initialized: {0}")]
    NotInitialized(String),

    #[error("encryption error: {0}")]
    Encryption(String),

    /// Upstream `bitcoin` consensus encoding error.
    #[error("bitcoin: {0}")]
    Bitcoin(#[from] bitcoin::consensus::encode::Error),

    /// Upstream `bdk_wallet` error (stringified; no single `bdk_wallet::Error` type in 3.1).
    #[error("bdk: {0}")]
    Bdk(String),

    /// Upstream `std::io::Error` (per drift closure from Task 1.5).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// BIP-137 message-signing protocol error (Task 6). Per F43
    /// pattern (per-protocol variant): kept distinct from `Sign`
    /// (transaction signing) so callers can match message-signing
    /// failures separately. Defends F7 (U5) and F50 (constant-time
    /// verify).
    #[error("bip137: {0}")]
    Bip137(String),

    /// SPKI pin parse/validation error (Task 7). Per F43 pattern
    /// (per-protocol variant): kept distinct from `Esplora` (HTTP
    /// runtime) so callers can distinguish "config is malformed"
    /// (fix the file) from "chain backend is unreachable" (retry).
    /// Defends F20 (SPKI pinning).
    #[error("spki pin: {0}")]
    SpkiPin(String),

    /// Mnemonic encryption/decryption error (Task 5 / issue #28). Per
    /// F43 pattern (per-protocol variant): kept distinct from
    /// `Encryption` (the underlying KDF/AEAD primitives) so callers can
    /// distinguish "blob malformed or wrong password" (caller error)
    /// from "KDF/AEAD library bug". Defends F5 (Argon2id KDF) + F6
    /// (AES-256-GCM AEAD) + F47 (zeroize-on-drop of the phrase).
    #[error("mnemonic cipher: {0}")]
    MnemonicCipher(String),

    /// Wallet-store persistence error (Task 54d / Issue #64, ADR 0001).
    /// Per F43 pattern (per-protocol variant): kept distinct from
    /// `Storage` (generic IO) so callers can distinguish wallet-persistence
    /// failures (filesystem layout, symlink defense, AAD verification)
    /// from generic disk errors. The Display message is intentionally
    /// generic for failures that would otherwise leak whether a wallet
    /// exists (oracle-attack mitigation per N2): "wallet not accessible
    /// (wrong password, wrong network, or corrupt blob)" collapses
    /// file-not-found, wrong-password, wrong-network-AAD, and
    /// corrupt-blob into one observable signature.
    #[error("wallet store: {0}")]
    WalletStore(String),
}

/// Result alias used by every public function in this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_mnemonic_displays_message() {
        let err = Error::InvalidMnemonic("bad words".into());
        assert_eq!(err.to_string(), "invalid mnemonic: bad words");
    }

    #[test]
    fn insufficient_funds_displays_amounts() {
        let err = Error::InsufficientFunds {
            needed: 1000,
            available: 500,
        };
        assert_eq!(
            err.to_string(),
            "insufficient funds: needed 1000 sat, have 500 sat"
        );
    }

    #[test]
    fn storage_displays_message() {
        let err = Error::Storage("disk full".into());
        assert_eq!(err.to_string(), "storage error: disk full");
    }

    #[test]
    fn encryption_displays_message() {
        let err = Error::Encryption("argon2 failed".into());
        assert_eq!(err.to_string(), "encryption error: argon2 failed");
    }

    #[test]
    fn from_io_error_works() {
        // Drift closure from Task 1.5: refuse_world_writable returns
        // io::Result; call sites need `?` to work without map_err.
        let io_err = std::io::Error::other("test io failure");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
        assert!(err.to_string().contains("test io failure"));
    }

    #[test]
    fn bdk_error_wraps_message() {
        // bdk_wallet 3.1 has no single `bdk_wallet::Error` type — it has
        // `wallet::LoadError`, `descriptor::Error`, `wallet::CreateTxError`,
        // etc. Per drift decision, `Bdk(String)` accepts a formatted
        // message from any of them. Call sites wrap at the boundary.
        let err = Error::Bdk("test bdk failure".into());
        assert_eq!(err.to_string(), "bdk: test bdk failure");
    }
}
