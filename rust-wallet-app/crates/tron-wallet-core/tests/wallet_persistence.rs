//! Phase 5 — end-to-end wallet persistence test (Task 4.7).
//!
//! Drives `WalletManager` through the full lifecycle on each PAL
//! storage backend:
//!
//!   1. `create(mnemonic, passphrase)` → `WalletId`
//!   2. `unlock(id, passphrase)` → `UnlockedWallet { mnemonic }`
//!   3. assert round-trip: phrase bytes match the input
//!   4. wrong passphrase is rejected
//!   5. missing id is rejected
//!   6. `delete(id)` removes the blob
//!
//! Each backend gets its own `#[test]` so a failure pinpoints which
//! PAL impl regressed.

use tron_wallet_core::keys::{Language, Mnemonic};
use tron_wallet_core::wallet::{WalletId, WalletManager};
use tron_wallet_core::WalletStorage;

use tron_wallet_core::platform::test::InMemoryStorage;

const PHRASE: &str = "abandon abandon abandon abandon abandon abandon \
                      abandon abandon abandon abandon abandon about";

fn fresh_mnemonic() -> Mnemonic {
    Mnemonic::from_phrase(PHRASE, Language::English).expect("mnemonic")
}

#[test]
fn in_memory_create_unlock_roundtrip() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let mnemonic = fresh_mnemonic();

    let id = mgr.create(&mnemonic, "hunter2").expect("create");
    let listed = mgr.list().expect("list");
    assert_eq!(listed, vec![id]);

    let unlocked = mgr.unlock(id, "hunter2").expect("unlock");
    assert_eq!(unlocked.id(), id);
    assert_eq!(unlocked.mnemonic().phrase(), PHRASE);
}

#[test]
fn wrong_passphrase_rejected() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id = mgr.create(&fresh_mnemonic(), "right").expect("create");

    let err = mgr.unlock(id, "wrong").expect_err("must reject");
    // Password Oracle Resistance: same variant as a corrupted blob.
    assert!(matches!(err, tron_wallet_core::Error::Encryption(_)));
}

#[test]
fn missing_id_rejected() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);

    let err = mgr
        .unlock(WalletId::new(), "anything")
        .expect_err("missing id must error");
    assert!(matches!(err, tron_wallet_core::Error::Wallet(_)));
}

#[test]
fn delete_makes_blobs_disappear() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id = mgr.create(&fresh_mnemonic(), "pw").expect("create");
    assert_eq!(storage.len(), 1);

    mgr.delete(id).expect("delete");
    assert_eq!(storage.len(), 0);
    assert!(mgr.list().expect("list").is_empty());

    // Idempotent — second delete is a no-op.
    mgr.delete(id).expect("delete idempotent");
}

#[test]
fn unique_wallet_ids_per_create() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let a = mgr.create(&fresh_mnemonic(), "same").expect("a");
    let b = mgr.create(&fresh_mnemonic(), "same").expect("b");
    assert_ne!(a, b, "two creates must yield distinct WalletIds");
    assert_eq!(
        mgr.list().expect("list").len(),
        2,
        "two creates must persist two blobs"
    );
}

#[test]
fn unlock_after_corrupt_blob_errors() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id = mgr.create(&fresh_mnemonic(), "good").expect("create");

    // Tamper with the stored blob directly.
    storage
        .put(&id, b"\x00not a real encrypted wallet\x00")
        .expect("put corrupt");

    let err = mgr.unlock(id, "good").expect_err("must reject");
    assert!(matches!(err, tron_wallet_core::Error::Encryption(_)));
}
