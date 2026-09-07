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

use tron_wallet_core::keys::{Language, Mnemonic, SECRET_KEY_LEN};
use tron_wallet_core::wallet::{WalletId, WalletManager};
use tron_wallet_core::WalletStorage;

use tron_wallet_core::platform::test::InMemoryStorage;
use zeroize::Zeroizing;

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
    assert!(!summary.is_private_key());
    assert_eq!(summary.kind, tron_wallet_core::wallet::WalletKind::Mnemonic);
}

#[test]
fn a_legacy_phrase_only_blob_still_unlocks() {
    // Written the way v0.1 wrote it: `{"phrase": "..."}` and nothing else.
    // The new untagged enum must decode this as `Mnemonic` (PR #545
    // review finding F) without any code knowing the field names changed.
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
    assert_eq!(
        unlocked.kind(),
        tron_wallet_core::wallet::WalletKind::Mnemonic
    );
}

#[test]
fn a_legacy_private_key_hex_blob_still_unlocks() {
    // Older v0.1 wrote `{"private_key_hex": "..."}` (camelCase). The new
    // `hex` field accepts that as an alias; this guards the alias path.
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id = WalletId::new();
    let legacy =
        r#"{"private_key_hex":"e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35"}"#;
    let blob = tron_wallet_core::crypto::encrypt(legacy.as_bytes(), b"pw").expect("encrypt");
    storage.put_atomic(&id, blob.as_bytes()).expect("put");

    let unlocked = mgr
        .unlock(id, "pw")
        .expect("a legacy private_key_hex blob must still open");
    assert_eq!(
        unlocked.kind(),
        tron_wallet_core::wallet::WalletKind::PrivateKey
    );
    assert!(
        unlocked.mnemonic().is_none(),
        "a raw-key wallet has no phrase"
    );
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
    let summary = mgr.summary(id, "pw").expect("summary");
    assert!(summary.is_private_key());
    assert_eq!(
        summary.kind,
        tron_wallet_core::wallet::WalletKind::PrivateKey
    );

    // The key must still produce a usable signing keypair via the borrowed
    // accessor (PR #545 finding G — `keypair(path)` errors for raw-key wallets).
    let path = tron_wallet_core::keys::DEFAULT_DERIVATION_PATH
        .parse()
        .expect("path");
    assert!(
        unlocked.keypair(&path).is_err(),
        "keypair(path) must reject raw-key wallets"
    );
    let keypair = unlocked.raw_keypair().expect("raw_keypair");
    let address =
        tron_wallet_core::address::Address::from_public_key(keypair.public_key()).expect("address");
    assert!(address.to_base58().starts_with('T'));

    // And the secret bytes are reachable straight from the borrowed keypair
    // for `submit::sign_prepared`.
    let secret: Zeroizing<[u8; SECRET_KEY_LEN]> = keypair.secret_bytes().clone();
    assert_eq!(secret.len(), SECRET_KEY_LEN);
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

// ─── Phase 5 Task 5.8 — gap tests (UC-4, UC-9, UC-10, UC-11, UC-12, UC-13,
//     UC-14, UC-15, UC-18, UC-19, UC-20, UC-22) ──────────────────────────────

/// `UnlockedWallet::mnemonic()` must return `&Mnemonic` (whose internal
/// `bip39::Mnemonic` wraps the phrase in `Zeroizing<String>` per Phase 1).
/// Bug it would catch: someone returns `String`/`&str` and the phrase
/// lingers in heap memory after the unlock drops.
#[test]
fn unlock_returns_zeroizing_mnemonic() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id = mgr.create(&fresh_mnemonic(), "pw").expect("create");
    let unlocked = mgr.unlock(id, "pw").expect("unlock");
    let phrase: &str = unlocked.mnemonic().expect("mnemonic variant").phrase();
    assert!(phrase.starts_with("abandon"));
}

/// End-to-end: create → rename → unlock preserves the secret.
#[test]
fn create_then_rename_then_unlock_preserves_secret() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let expected_phrase = fresh_mnemonic().phrase().to_owned();
    let id = mgr.create(&fresh_mnemonic(), "pw").expect("create");
    mgr.rename(id, "pw", "renamed-label").expect("rename");
    let unlocked = mgr.unlock(id, "pw").expect("unlock");
    assert_eq!(unlocked.mnemonic().expect("mn").phrase(), expected_phrase);
    assert_eq!(unlocked.name(), Some("renamed-label"));
}

/// Delete then recreate with the same passphrase yields a new id, not
/// the deleted one. Idempotency guard.
#[test]
fn delete_then_recreate_with_same_password_yields_new_id() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id1 = mgr.create(&fresh_mnemonic(), "pw").expect("create 1");
    mgr.delete(id1).expect("delete");
    let id2 = mgr.create(&fresh_mnemonic(), "pw").expect("create 2");
    assert_ne!(id1, id2);
    assert!(mgr.unlock(id1, "pw").is_err(), "old id must not resurrect");
    assert!(mgr.unlock(id2, "pw").is_ok());
}

/// After delete, unlock on the deleted id surfaces `Error::Wallet`.
#[test]
fn delete_then_unlock_returns_wallet_not_found() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id = mgr.create(&fresh_mnemonic(), "pw").expect("create");
    mgr.delete(id).expect("delete");
    let err = mgr.unlock(id, "pw").expect_err("must not find deleted");
    assert!(matches!(err, tron_wallet_core::Error::Wallet(_)));
}

/// Imported raw-key wallet rejects `rename` to empty/whitespace label.
#[test]
fn imported_raw_key_cannot_be_renamed_to_empty_label() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let hex = "e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35";
    let id = mgr
        .import_private_key(hex, "pw", Some("paper"), None)
        .expect("import");
    let err = mgr.rename(id, "pw", "").expect_err("must reject empty");
    assert!(matches!(err, tron_wallet_core::Error::Wallet(_)));
    let err = mgr
        .rename(id, "pw", "   ")
        .expect_err("must reject whitespace");
    assert!(matches!(err, tron_wallet_core::Error::Wallet(_)));
}

/// `summary` reports `WalletKind::PrivateKey` for raw-key imports.
#[test]
fn imported_raw_key_summary_kind_is_private_key() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let hex = "e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35";
    let id = mgr
        .import_private_key(hex, "pw", Some("paper"), Some("mainnet"))
        .expect("import");
    let sum = mgr.summary(id, "pw").expect("summary");
    assert!(sum.is_private_key());
    assert_eq!(sum.kind, tron_wallet_core::wallet::WalletKind::PrivateKey);
    assert_eq!(sum.name.as_deref(), Some("paper"));
    assert_eq!(sum.network.as_deref(), Some("mainnet"));
}

/// `list()` returns every persisted id, regardless of insertion order.
#[test]
fn list_returns_ids_in_independent_order() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let mut ids = Vec::new();
    for _ in 0..5 {
        ids.push(mgr.create(&fresh_mnemonic(), "pw").expect("create"));
    }
    let listed = mgr.list().expect("list");
    assert_eq!(listed.len(), 5);
    // Set equality, not order equality — storage is a map.
    let a: std::collections::HashSet<_> = listed.iter().copied().collect();
    let b: std::collections::HashSet<_> = ids.iter().copied().collect();
    assert_eq!(a, b);
}

/// `list_summaries` with a wrong password returns only decryptable
/// wallets — never panics on the wrong-pw ones.
#[test]
fn list_summaries_with_wrong_password_returns_empty_or_decryptable_only() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    mgr.create_with_meta(&fresh_mnemonic(), "pw-correct", None, None)
        .expect("a");
    mgr.create_with_meta(&fresh_mnemonic(), "pw-other", None, None)
        .expect("b");
    let visible = mgr.list_summaries("pw-correct").expect("list");
    assert_eq!(visible.len(), 1, "wrong-pw wallet must not appear");
}

/// Empty passphrase is allowed at create (the user is choosing to
/// store an unprotected blob — their call).
#[test]
fn create_with_meta_empty_pw_is_allowed() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id = mgr
        .create_with_meta(&fresh_mnemonic(), "", Some("nopw"), None)
        .expect("empty pw is allowed");
    let unlocked = mgr.unlock(id, "").expect("unlock with empty pw");
    assert_eq!(unlocked.name(), Some("nopw"));
}

/// Very long passphrases are allowed (KDF is bounded only by Argon2id params).
#[test]
fn create_with_meta_long_pw_is_allowed() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let long_pw: String = "x".repeat(10_000);
    let id = mgr.create(&fresh_mnemonic(), &long_pw).expect("long pw ok");
    let unlocked = mgr.unlock(id, &long_pw).expect("unlock long pw");
    assert!(unlocked.mnemonic().is_some());
}

/// `unlock` is read-only — concurrent calls do not block each other.
#[test]
fn unlock_does_not_block_concurrent_calls() {
    use std::sync::Arc;
    use std::thread;
    let storage = Arc::new(InMemoryStorage::new());
    let id = {
        let mgr = WalletManager::new(&*storage);
        mgr.create(&fresh_mnemonic(), "pw").expect("create")
    };
    let mut handles = Vec::new();
    for _ in 0..4 {
        let storage = Arc::clone(&storage);
        let id = id;
        handles.push(thread::spawn(move || {
            let mgr = WalletManager::new(&*storage);
            mgr.unlock(id, "pw").is_ok()
        }));
    }
    for h in handles {
        assert!(
            h.join().expect("join"),
            "concurrent unlock must not deadlock"
        );
    }
}

/// `create` writes atomically: storage ends with exactly one entry,
/// never a partial/corrupt half-blob on the success path.
#[test]
fn create_writes_atomically() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id = mgr.create(&fresh_mnemonic(), "pw").expect("create");
    assert_eq!(mgr.list().expect("list").len(), 1);
    assert!(mgr.unlock(id, "pw").is_ok());
}

/// Truncated blob (cut mid-ciphertext) is rejected with `Error::Encryption`.
#[test]
fn unlock_after_partial_truncate_errors() {
    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id = mgr.create(&fresh_mnemonic(), "pw").expect("create");
    let original = storage.get(&id).expect("get").expect("blob present");
    let blob_bytes: Vec<u8> = original.to_vec();
    let half = blob_bytes.len() / 2;
    let truncated_bytes = blob_bytes[..half].to_vec();
    drop(original);
    let truncated =
        tron_wallet_core::EncryptedWallet::from_blob(truncated_bytes).expect("from_blob");
    storage
        .put_atomic(&id, truncated.as_bytes())
        .expect("put_atomic");
    let err = mgr.unlock(id, "pw").expect_err("must reject truncated");
    assert!(matches!(err, tron_wallet_core::Error::Encryption(_)));
}

/// Desktop PAL end-to-end: `FileWalletStorage` round-trips a wallet
/// through the real disk-backed backend. Linux-only smoke (path differs
/// per OS; CI runs Linux for v0.1).
#[cfg(target_os = "linux")]
#[test]
fn file_wallet_storage_round_trip() {
    use std::env;
    use tron_wallet_core::platform::desktop::FileWalletStorage;

    let dir = env::temp_dir().join(format!(
        "tron-wallet-test-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let storage = FileWalletStorage::with_dir(dir.clone()).expect("with_dir");
    let mgr = WalletManager::new(&storage);
    let id = mgr.create(&fresh_mnemonic(), "pw").expect("create");
    let unlocked = mgr.unlock(id, "pw").expect("unlock");
    assert!(unlocked.mnemonic().is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

/// iOS PAL contract: `KeychainWalletStorage` exposes the `WalletStorage`
/// trait surface used by `WalletManager`. Trait-level smoke only — no
/// FFI bridge in v0.1. Cross-compile gate.
#[cfg(target_os = "ios")]
#[test]
fn keychain_wallet_storage_contract_test() {
    use tron_wallet_core::platform::ios::KeychainWalletStorage;
    let _storage: Box<dyn WalletStorage> = Box::new(KeychainWalletStorage::new());
}

/// Android PAL contract: `EncryptedFileWalletStorage` exposes the
/// `WalletStorage` trait surface used by `WalletManager`. Trait-level
/// smoke only — no JNI bridge in v0.1. Cross-compile gate.
#[cfg(target_os = "android")]
#[test]
fn encrypted_file_wallet_storage_contract_test() {
    use tron_wallet_core::platform::android::EncryptedFileWalletStorage;
    let _storage: Box<dyn WalletStorage> = Box::new(EncryptedFileWalletStorage::new());
}
