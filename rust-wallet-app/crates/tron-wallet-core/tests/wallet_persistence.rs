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
    assert_eq!(
        unlocked
            .mnemonic()
            .expect("a mnemonic-backed record must yield a phrase")
            .phrase(),
        PHRASE
    );
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

// ---------------------------------------------------------------------------
// Plan §Phase 6 — record metadata, rename, and raw-key import.
//
// These close the Phase 5 carry-over the CLI's `wallet rename` / `--name` /
// `--private-key-file` flags depend on. The backward-compatibility test is the
// important one: the record shape changed, and a v0.1 blob is the only copy of
// its wallet's funds.
// ---------------------------------------------------------------------------

/// A valid secp256k1 scalar (the BIP-32 test vector's master key).
const RAW_KEY_HEX: &str = "e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35";

#[test]
fn create_with_meta_round_trips_name_and_network() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);

    let id = mgr
        .create_with_meta(&fresh_mnemonic(), "pw", Some("cold"), Some("nile"))
        .expect("create_with_meta");

    let unlocked = mgr.unlock(id, "pw").expect("unlock");
    assert_eq!(unlocked.name(), Some("cold"));
    assert_eq!(unlocked.network(), Some("nile"));

    let summary = mgr.summary(id, "pw").expect("summary");
    assert_eq!(summary.name.as_deref(), Some("cold"));
    assert_eq!(summary.network.as_deref(), Some("nile"));
    assert!(!summary.is_private_key);
}

#[test]
fn a_legacy_phrase_only_blob_still_unlocks() {
    // Written the way v0.1 wrote it: `{"phrase": "..."}` and nothing else.
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id = WalletId::new();
    let legacy = format!(r#"{{"phrase":"{PHRASE}"}}"#);
    let blob = tron_wallet_core::crypto::encrypt(legacy.as_bytes(), b"pw").expect("encrypt");
    storage.put_atomic(&id, blob.as_bytes()).expect("put");

    let unlocked = mgr
        .unlock(id, "pw")
        .expect("a pre-metadata blob must still open");
    assert_eq!(unlocked.mnemonic().expect("phrase").phrase(), PHRASE);
    assert_eq!(unlocked.name(), None);
    assert_eq!(unlocked.network(), None);
}

#[test]
fn rename_rewrites_the_label_without_touching_the_secret() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id = mgr
        .create_with_meta(&fresh_mnemonic(), "pw", Some("old"), Some("nile"))
        .expect("create");

    mgr.rename(id, "pw", "new").expect("rename");

    let unlocked = mgr.unlock(id, "pw").expect("unlock");
    assert_eq!(unlocked.name(), Some("new"));
    assert_eq!(
        unlocked.mnemonic().expect("phrase").phrase(),
        PHRASE,
        "rename must not disturb the stored secret"
    );
    assert_eq!(unlocked.network(), Some("nile"), "network must survive");
}

#[test]
fn rename_with_the_wrong_passphrase_is_refused() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id = mgr.create(&fresh_mnemonic(), "right").expect("create");

    let err = mgr.rename(id, "wrong", "new").expect_err("must reject");
    assert!(matches!(err, tron_wallet_core::Error::Encryption(_)));

    // And the record must be untouched — a failed rename that corrupted the
    // blob would be worse than one that just errored.
    assert!(mgr.unlock(id, "right").is_ok());
}

#[test]
fn rename_rejects_an_empty_label() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id = mgr.create(&fresh_mnemonic(), "pw").expect("create");
    assert!(matches!(
        mgr.rename(id, "pw", "   "),
        Err(tron_wallet_core::Error::Wallet(_))
    ));
}

#[test]
fn imported_raw_key_unlocks_with_no_phrase() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id = mgr
        .import_private_key(RAW_KEY_HEX, "pw", Some("paper"), Some("mainnet"))
        .expect("import");

    let unlocked = mgr.unlock(id, "pw").expect("unlock");
    assert!(
        unlocked.mnemonic().is_none(),
        "a raw-key wallet has no recovery phrase to hand back"
    );
    assert!(mgr.summary(id, "pw").expect("summary").is_private_key);

    // The key must still produce a usable signing keypair.
    let path = tron_wallet_core::keys::DEFAULT_DERIVATION_PATH
        .parse()
        .expect("path");
    let keypair = unlocked.keypair(&path).expect("keypair");
    let address =
        tron_wallet_core::address::Address::from_public_key(keypair.public_key()).expect("address");
    assert!(address.to_base58().starts_with('T'));
}

#[test]
fn import_accepts_an_0x_prefix_and_rejects_a_bad_scalar() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);

    assert!(mgr
        .import_private_key(&format!("0x{RAW_KEY_HEX}"), "pw", None, None)
        .is_ok());

    // All-zero is not a valid scalar: it yields an address nobody can spend
    // from, so it must fail before anything is written.
    let err = mgr
        .import_private_key(&"0".repeat(64), "pw", None, None)
        .expect_err("zero scalar must be refused");
    assert!(matches!(err, tron_wallet_core::Error::Derivation(_)));

    assert!(mgr.import_private_key("not-hex", "pw", None, None).is_err());
}

#[test]
fn list_summaries_skips_wallets_under_a_different_passphrase() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    mgr.create_with_meta(&fresh_mnemonic(), "pw-a", Some("a"), None)
        .expect("a");
    mgr.create_with_meta(&fresh_mnemonic(), "pw-b", Some("b"), None)
        .expect("b");

    let visible = mgr.list_summaries("pw-a").expect("list");
    assert_eq!(visible.len(), 1, "only the pw-a wallet should decrypt");
    assert_eq!(visible[0].name.as_deref(), Some("a"));
    // Both ids are still enumerable — skipping is not hiding.
    assert_eq!(mgr.list().expect("list ids").len(), 2);
}
