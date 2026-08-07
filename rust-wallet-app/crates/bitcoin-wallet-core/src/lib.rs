//! bitcoin-wallet-core: standalone Bitcoin wallet engine.
//!
//! See spec at docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-design.md.
//! Threat model: docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-threat-model.md.
//!
//! One permitted `unsafe` exception: keys/secret.rs Secret::into_inner uses
//! ptr::read + mem::forget to move out before ZeroizeOnDrop fires.
//! Tracked by `cargo geiger` in CI per F53.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod address;
pub mod chain;
pub mod config;
pub mod crypto;
pub mod error;
pub mod keys;
pub mod script;
pub mod threat;
pub mod util;
pub mod wallet;

pub use error::{Error, Result};
