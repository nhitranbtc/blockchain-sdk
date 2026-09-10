//! `sol-wallet-core` error type.
//!
//! Phase 1 extends the placeholder with the per-domain variants the
//! `Wallet` keypair constructors need (mnemonic parse / seed stretch /
//! SLIP-0010 derivation / base58 secret parse). The remaining variants
//! for tx builder, RPC, persistence, FFI panic-utf8 boundary land in
//! Phase 5/6/7 — see plan §Phase 6 Task 6.3 and §Phase 7 Task 7.4 for
//! the full 21-variant breakdown.

use thiserror::Error;

/// Crate-wide error.
#[derive(Debug, Error)]
pub enum Error {
    /// Placeholder. Replaced by per-domain variants in Phase 5+.
    #[error("sol-wallet-core: placeholder (Phase 0)")]
    Placeholder,

    /// BIP-39 phrase failed to parse — wrong word count, unknown word,
    /// non-English wordlist, or checksum failure.
    ///
    /// Wraps `bip39::Error`. The `Phantom` UX surfaces the same error
    /// for all four sub-failures (the wallet app owns the human-readable
    /// message); here we keep them coalesced so callers don't pattern-match
    /// a private crate's enum.
    #[error(
        "sol-wallet-core: invalid mnemonic — wrong word count, unknown word, or checksum failure"
    )]
    InvalidMnemonic,

    /// BIP-39 mnemonic parse succeeded but the SLIP-0010 derivation path
    /// (`m/44'/501'/{account}'/0'/{address_index}'`) was rejected by
    /// `ed25519-bip32`. Surfaces only when the seed bytes are unusable
    /// (e.g. zero-length) — never for valid Phantom-shaped inputs.
    #[error("sol-wallet-core: SLIP-0010 derivation rejected seed/path: {0}")]
    DerivationFailed(String),

    /// `solana_sdk::Keypair::try_from(seed_bytes)` rejected the derived
    /// 32-byte Ed25519 seed. Ed25519 keys of all-zero bytes are
    /// mathematically valid but disallowed by Solana's wire format to
    /// avoid the off-curve ambiguity.
    #[error("sol-wallet-core: derived seed produced no usable Ed25519 keypair")]
    InvalidSeed,

    /// Base58 secret passed to `Wallet::fromBase58` was not 32+32 = 64
    /// bytes after decoding, or the base58 alphabet was malformed.
    #[error("sol-wallet-core: base58 secret must decode to 64 bytes (32-byte secret + 32-byte pubkey); got {0} bytes")]
    InvalidBase58Secret(usize),

    /// User-supplied Solana address failed to parse — malformed base58,
    /// wrong length, or the resulting bytes are NOT on the Ed25519
    /// curve (i.e. a PDA-shaped address used as a wallet recipient).
    ///
    /// The Phantom wallet refuses to send to off-curve addresses
    /// because no private key exists for a PDA — sending to one would
    /// burn the funds. V0.1 mirrors that guard at the parser
    /// boundary so callers see `Error::InvalidAddress` instead of an
    /// `Err(PubkeyError)` from `solana_sdk`.
    #[error("sol-wallet-core: invalid Solana address — {0}")]
    InvalidAddress(String),

    /// User-supplied SOL amount failed to parse — NaN, ±Inf,
    /// negative, or beyond `u64::MAX` lamports.
    ///
    /// Wraps `f64`-to-`u64` overflow at the parser boundary so the
    /// wallet UI surfaces a single error rather than the validator's
    /// opaque `InvalidLamports` rejection. Mirrors Phantom's "Send"
    /// form's input validator.
    #[error("sol-wallet-core: invalid SOL amount — {0}")]
    InvalidAmount(String),
}

/// Crate-wide result alias.
pub type Result<T> = core::result::Result<T, Error>;
