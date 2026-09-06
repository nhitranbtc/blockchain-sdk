//! Error surface for `tron-wallet-core`.
//!
//! Variants carry owned strings rather than `anychain` error types on purpose.
//! The plan pins `anychain-*` at exact versions precisely because upstream may
//! change under us (Risk Register #1); keeping their types out of our public
//! signatures means a pin bump is a private change, not a breaking one.

/// Anything that can go wrong in this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A BIP-39 phrase failed word-list or checksum validation.
    #[error("invalid mnemonic: {0}")]
    Mnemonic(String),

    /// BIP-32 derivation failed — bad path, bad seed length, or a child index
    /// that produced an invalid scalar.
    #[error("key derivation failed: {0}")]
    Derivation(String),

    /// An address could not be parsed or built from a public key.
    #[error("invalid address: {0}")]
    Address(String),

    /// Signing failed, or produced a recovery id TRON will not accept.
    #[error("signing failed: {0}")]
    Signing(String),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = core::result::Result<T, Error>;
