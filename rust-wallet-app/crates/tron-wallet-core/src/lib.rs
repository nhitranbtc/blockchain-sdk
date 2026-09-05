//! TRON (TRX + TRC-20) wallet core, built on the `anychain` stack.
//!
//! Plan: `docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md`.
//!
//! Phase 1 covers the foundation — mnemonics, BIP-32 derivation, T-addresses,
//! and the sign-only path. Transaction building, TRC-20, the platform
//! abstraction layer, and the FFI surface arrive in later phases.
//!
//! ```
//! use tron_wallet_core::address::Address;
//! use tron_wallet_core::keys::{derive_keypair, Language, Mnemonic, DEFAULT_DERIVATION_PATH};
//!
//! let mnemonic = Mnemonic::from_phrase(
//!     "abandon abandon abandon abandon abandon abandon \
//!      abandon abandon abandon abandon abandon about",
//!     Language::English,
//! )?;
//!
//! let keypair = derive_keypair(&mnemonic, "", &DEFAULT_DERIVATION_PATH.parse()?)?;
//! let address = Address::from_public_key(keypair.public_key())?;
//!
//! assert!(address.to_base58().starts_with('T'));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Handling secrets
//!
//! Every secret this crate hands back lives in `zeroize::Zeroizing`, and the
//! types that hold one redact it from `Debug`. That is deliberate: `anychain`'s
//! signing entry point takes a plain `&[u8]` it never clears, so the guarantee
//! has to be maintained on this side of the boundary.

pub mod address;
pub mod error;
pub mod keys;
pub mod tx;

pub use error::{Error, Result};
