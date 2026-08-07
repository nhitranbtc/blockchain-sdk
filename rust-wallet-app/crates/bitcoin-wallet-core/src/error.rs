//! bitcoin-wallet-core error type.
//!
//! Stub for Task 1, Step 12. The full enum is defined in Task 2.

use thiserror::Error;

/// All errors returned by `bitcoin-wallet-core` public APIs.
///
/// Task 1 stub: only `NotImplemented`. Task 2 expands to the full per-domain
/// variant set used across subsequent tasks.
#[derive(Debug, Error)]
pub enum Error {
    /// Stub variant. Replaced by Task 2's full per-domain enum.
    #[error("not implemented yet")]
    NotImplemented,
}

/// Result alias used by every public function in this crate.
pub type Result<T> = std::result::Result<T, Error>;
