//! F14 (persistence) — bdk_file_store SQLite layer (Story 12 / Issue #130).
//!
//! Per plan §Task 9 (Persistence step) + user-stories Story 12 AC. The
//! in-memory `bdk_wallet::Wallet` currently loses UTXO state across
//! CLI invocations (see `wallet/mod.rs` F14 note). Story 12 adds a
//! `bdk_file_store::Store` layer so `btc wallet show` after a
//! `btc wallet create` works without re-syncing from Esplora.
//!
//! **Stub status (TDD red):** this module currently exposes a
//! `persist()` stub that always returns `Err(Error::Storage(...))`.
//! Full implementation tracked by Issue #130 (PR3 of the Stories
//! 9/10/12 series). Test `persist_then_load_round_trips_descriptor`
//! is `#[ignore]`d pending the real implementation.

use std::path::Path;

use crate::error::{Error, Result};

/// Persist the wallet's current `ChangeSet` to a SQLite store at
/// `db_path`. Stub — always returns `Err` until Story 12 ships.
///
/// # Errors
///
/// - `Error::Storage("Story 12 not implemented ...")` until the full
///   implementation lands.
pub fn persist(_wallet: &super::Wallet, _db_path: &Path) -> Result<()> {
    Err(Error::Storage(
        "Story 12 not implemented (bdk_file_store persistence layer — Issue #130)".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Mnemonic;
    use crate::wallet::Wallet;
    use bitcoin::Network;
    use tempfile::tempdir;

    /// TDD red (Story 12 / Issue #130 PR3): persist a wallet to
    /// SQLite, reload from the same store, verify descriptor matches.
    ///
    /// Current behavior: the stub `persist()` returns `Err`, so the
    /// `is_err()` assertion passes (stub is intentionally failing).
    /// Once Story 12 ships, the assertion flips to `is_ok()` and the
    /// `#[ignore]` is removed.
    #[test]
    #[ignore = "Story 12 implementation pending — test stub for #130"]
    fn persist_then_load_round_trips_descriptor() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("wallet.db");
        let mnemonic = Mnemonic::generate(12).expect("fresh mnemonic");
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("wallet builds");

        // Stub: persist must return Err until implementation lands.
        let result = persist(&wallet, &db_path);
        assert!(
            result.is_err(),
            "stub persist must return Err until Story 12 ships; got {result:?}"
        );
    }
}
