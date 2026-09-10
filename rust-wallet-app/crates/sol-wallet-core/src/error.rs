//! `sol-wallet-core` error type.
//!
//! Phase 0 ships a single placeholder variant. The full 21-variant enum
//! (covering wallet construction, address surface, tx builder, RPC,
//! persistence, FFI panic-utf8 boundary) lands in Phase 5/6/7 — see plan
//! §Phase 6 Task 6.3 and §Phase 7 Task 7.4 for the per-variant breakdown.

use thiserror::Error;

/// Crate-wide error.
#[derive(Debug, Error)]
pub enum Error {
    /// Placeholder. Replaced by per-domain variants in Phase 5+.
    #[error("sol-wallet-core: placeholder (Phase 0)")]
    Placeholder,
}

/// Crate-wide result alias.
pub type Result<T> = core::result::Result<T, Error>;
