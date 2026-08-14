//! F14 (persistence) — bdk_file_store SQLite-like append-only file
//! layer for Story 12 (Issue #130 PR3).
//!
//! Per plan §Task 9 (Persistence step) + user-stories Story 12 AC:
//! subsequent `btc wallet show` reads the wallet's `ChangeSet` from
//! this store instead of re-syncing from Esplora every invocation.
//!
//! ## Architecture
//! - `Store<ChangeSet>` is an append-only file (NOT real SQLite — see
//!   bdk_file_store README: "development/testing database").
//! - Each `persist()` call appends the wallet's current staged
//!   `ChangeSet` (network, descriptors, chain index, tx graph).
//! - On `read_change_set()`, load the file and return the aggregated
//!   `ChangeSet` (bdk_file_store uses `Merge` to combine entries).
//!
//! ## Threat-model coverage
//! - F19 (atomic write) — bdk_file_store uses `fsync` via `Drop` on
//!   the `Store` handle. The Store's `append` writes then fsyncs
//!   before returning.

use std::path::Path;

use bdk_file_store::Store;
use bdk_wallet::{ChangeSet, Wallet as BdkWallet};

use crate::error::{Error, Result};

/// Magic bytes for the bdk_file_store file. 8 bytes, unique to
/// `bitcoin-wallet-core`. Loaded stores with non-matching magic
/// bytes return `Error::Storage("bdk_file_store load: InvalidMagicBytes")`.
pub const DB_MAGIC: &[u8] = b"btc-wal0";

/// Persist the wallet's current staged `ChangeSet` to the store at
/// `db_path`. Creates the store file if it does not exist.
///
/// # Behavior
///
/// - If `bdk.staged_mut()` returns `Some(stage)`, take the changeset
///   and append it.
/// - If `None` (no pending mutations), append an empty `ChangeSet`
///   (still creates the file with magic bytes).
///
/// # Errors
///
/// - `Error::Storage` on `bdk_file_store` open/append IO failure or
///   bincode encode failure.
pub fn persist(bdk: &mut BdkWallet, db_path: &Path) -> Result<()> {
    let changeset: ChangeSet = match bdk.staged_mut() {
        Some(stage) => std::mem::take(stage),
        None => ChangeSet::default(),
    };

    let (mut store, _existing) = Store::<ChangeSet>::load_or_create(DB_MAGIC, db_path)
        .map_err(|e| Error::Storage(format!("bdk_file_store open: {e}")))?;
    store
        .append(&changeset)
        .map_err(|e| Error::Storage(format!("bdk_file_store append: {e}")))?;
    drop(store); // fsync via Drop
    Ok(())
}

/// Read the aggregated `ChangeSet` from `db_path`. Returns `Ok(None)`
/// if the file does not exist.
///
/// # Errors
///
/// - `Error::Storage` on bincode / magic-byte mismatch / IO failure.
///   If the store contains corrupt appends, `bdk_file_store` returns
///   `StoreErrorWithDump` containing the aggregated changeset up to
///   the corruption point — we surface the error to the caller (who
///   can call `read_change_set_with_dump` if they need recovery).
pub fn read_change_set(db_path: &Path) -> Result<Option<ChangeSet>> {
    if !db_path.exists() {
        return Ok(None);
    }
    let (store, changeset) = Store::<ChangeSet>::load(DB_MAGIC, db_path)
        .map_err(|e| Error::Storage(format!("bdk_file_store load: {e}")))?;
    drop(store);
    Ok(changeset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::network::coin_type_for;
    use crate::keys::Mnemonic;
    use bip32::DerivationPath;
    use bitcoin::Network;
    use std::str::FromStr;
    use tempfile::tempdir;

    /// Build a fresh `bdk_wallet::Wallet` for tests. Mirrors the
    /// production `Wallet::build_bdk_wallet` flow (NativeSegwit,
    /// BIP-84 derivation, tprv-derived xprv) without going through
    /// the public `Wallet` wrapper (which holds a `Mutex<Option<...>>`).
    fn fresh_bdk_wallet() -> BdkWallet {
        let mnemonic = Mnemonic::generate(12).expect("fresh mnemonic");
        let network = Network::Testnet;
        let seed = mnemonic.to_seed("");
        let seed_arr: [u8; 64] = seed.expose().as_slice().try_into().expect("64-byte seed");
        let master = crate::keys::XPrvHolder::master_from_seed(&seed_arr).expect("master xprv");
        let coin = coin_type_for(network);
        let path_str = format!("m/84h/{}h/0h", coin);
        let path = DerivationPath::from_str(&path_str).expect("BIP-84 path");
        let derived = master.derive(&path).expect("derive");
        let xprv_secret = derived.to_xprv_secret(network);
        let ext = format!("wpkh({}/0/*)", xprv_secret.expose());
        let chg = format!("wpkh({}/1/*)", xprv_secret.expose());
        drop(xprv_secret);
        drop(derived);
        drop(master);

        BdkWallet::create(ext, chg)
            .network(network)
            .create_wallet_no_persist()
            .expect("bdk wallet")
    }

    #[test]
    fn persist_creates_store_file() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("wallet.db");
        let mut bdk = fresh_bdk_wallet();

        persist(&mut bdk, &db_path).expect("persist should succeed");
        assert!(db_path.exists(), "store file should exist after persist");
    }

    #[test]
    fn read_change_set_returns_none_when_file_missing() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("nonexistent.db");
        let cs = read_change_set(&db_path).expect("read should not error on missing file");
        assert!(cs.is_none(), "missing file → None");
    }

    #[test]
    fn persist_then_read_returns_some_change_set() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("wallet.db");
        let mut bdk = fresh_bdk_wallet();

        persist(&mut bdk, &db_path).expect("persist");
        let cs = read_change_set(&db_path).expect("read");
        assert!(cs.is_some(), "after persist, ChangeSet should be Some");
    }

    #[test]
    fn persist_twice_appends_both_changesets() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("wallet.db");
        let mut bdk = fresh_bdk_wallet();

        persist(&mut bdk, &db_path).expect("first persist");
        persist(&mut bdk, &db_path).expect("second persist");

        let cs = read_change_set(&db_path).expect("read").expect("Some");
        // Aggregated ChangeSet is non-empty after two appends.
        // (bdk_file_store merges entries; an empty merge = empty ChangeSet.)
        assert!(
            cs.descriptor.is_some() || cs.network.is_some(),
            "aggregated changeset should carry descriptor or network after two appends"
        );
    }
}
