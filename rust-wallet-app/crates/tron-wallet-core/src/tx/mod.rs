//! Transaction construction, signing, and broadcast.
//!
//! Module layout:
//!
//! - [`sign`] — Phase 1. Recoverable secp256k1 signatures, dual-SHA256 txid.
//! - [`builder`] — Phase 2. Anychain-tron parameter builders + thin wrappers
//!   over the `anychain_tron::trx::*_contract` family.
//! - [`broadcast`] — Phase 2. Envelope shapes for TronGrid HTTP responses.
//! - [`summary`] — Phase 2. Receipt summary shape.

pub mod broadcast;
pub mod builder;
pub mod sign;
pub mod summary;
