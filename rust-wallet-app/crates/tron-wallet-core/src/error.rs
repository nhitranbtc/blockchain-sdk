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

    /// A transaction builder was given inputs that anychain-tron refused:
    /// unparsable amounts, invalid address strings, mismatch between a contract
    /// owner and the supplied `sender`.
    #[error("transaction build failed: {0}")]
    TransactionBuild(String),

    /// A TronGrid (or other Tron fullnode) call failed at the HTTP layer or
    /// returned a non-success response.
    #[error("node call failed: {0}")]
    Node(String),

    /// The body or signature of a node response could not be decoded.
    #[error("node response parse failed: {0}")]
    NodeResponse(String),

    /// Configuration value was malformed or missing.
    #[error("config: {0}")]
    Config(String),

    /// An SPKI pin could not be decoded, or a leaf cert's SPKI did not match.
    #[error("spki pin: {0}")]
    SpkiPin(String),

    /// A cross-network guard refused an operation (e.g. mainnet → nile send).
    #[error("disambiguation guard: {0}")]
    Disambiguation(String),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = core::result::Result<T, Error>;
