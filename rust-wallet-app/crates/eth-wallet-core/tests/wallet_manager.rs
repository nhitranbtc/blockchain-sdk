//! Integration tests for `WalletManager` — round-trip encrypted persistence.
//!
//! Uses `WalletManager::open_at(tempfile::tempdir)` for hermetic per-test
//! base_dir; no global side effects. Verifies:
//! - create_wallet round-trip: store + reload from disk + unlock yields the
//!   same mnemonic phrase (deterministic via seeded phrase below).
//! - import_wallet: BIP-39 phrase import persists correctly.
//! - import_private_key: secp256k1 hex key import persists correctly.
//! - wrong password fails to unlock (AES-GCM auth tag rejects).
//! - delete_wallet removes the on-disk file.
//! - open_at scans existing on-disk wallets back into the cache.

use std::fs;

use eth_wallet_core::wallet::{Network, WalletError, WalletManager};

/// 12-word determinate mnemonic for round-trip tests.
const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/// Password used for all derive/encrypt tests. Real CLI prompts per-call.
const TEST_PASSWORD: &[u8] = b"correct horse battery staple";

/// First-receive address (m/44'/60'/0'/0/0) of `TEST_MNEMONIC` — locks
/// the deterministic derivation contract.
const EXPECTED_ADDR: &str = "0x9858EfFD232B4033E47d90003D41EC34EcaEda94";

fn parse_addr(s: &str) -> alloy_primitives::Address {
    s.parse().expect("hardcoded address must parse")
}

#[test]
fn create_wallet_round_trip_unlock_yields_original_phrase() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let mgr = WalletManager::open_at(tmp.path().to_path_buf()).expect("open");
    let w = mgr
        .create_wallet("test-create", TEST_PASSWORD)
        .expect("create");
    assert_eq!(w.network, Network::Sepolia);
    assert_ne!(w.address, alloy_primitives::Address::ZERO);
    assert_ne!(
        w.address,
        parse_addr(EXPECTED_ADDR),
        "fresh create must NOT collide with TEST_MNEMONIC address"
    );

    let enc_path = tmp
        .path()
        .join("sepolia")
        .join(format!("{}.enc", w.wallet_id));
    assert!(
        enc_path.exists(),
        "encrypted blob must land at {enc_path:?}"
    );

    drop(mgr);
    let mgr2 = WalletManager::open_at(tmp.path().to_path_buf()).expect("re-open");
    let unlocked = mgr2.unlock(w.wallet_id, TEST_PASSWORD).expect("unlock");

    // The decrypted phrase is the freshly-generated one; verify it's a
    // valid 12-word BIP-39 English phrase AND re-derives the wallet's
    // claimed address at index 0. This locks the round-trip contract.
    let phrase_str = unlocked.to_string();
    let words: Vec<&str> = phrase_str.split_whitespace().collect();
    assert_eq!(words.len(), 12, "fresh mnemonic must be 12 words");

    let rederived = alloy_signer_local::MnemonicBuilder::english()
        .phrase(unlocked.to_string().as_str())
        .index(0)
        .expect("valid")
        .build()
        .expect("build")
        .address();
    assert_eq!(
        rederived, w.address,
        "unlocked phrase + index 0 must re-derive the wallet's claimed address"
    );
}

#[test]
fn import_wallet_round_trip() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let mgr = WalletManager::open_at(tmp.path().to_path_buf()).expect("open");
    let w = mgr
        .import_wallet("import-test", TEST_MNEMONIC, TEST_PASSWORD)
        .expect("import");
    assert_eq!(
        w.address,
        parse_addr(EXPECTED_ADDR),
        "V2 mirror: imported phrase must derive the known ETH address"
    );

    let out = mgr.unlock(w.wallet_id, TEST_PASSWORD).expect("unlock");
    assert_eq!(out.to_string(), TEST_MNEMONIC);
}

#[test]
fn import_private_key_round_trip() {
    // Anvil prefunded account #0 (per alloy-node-bindings convention).
    let pk_hex = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    let expected_addr = parse_addr("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

    let tmp = tempfile::tempdir().expect("tmpdir");
    let mgr = WalletManager::open_at(tmp.path().to_path_buf()).expect("open");
    let w = mgr
        .import_private_key("pk-test", pk_hex, TEST_PASSWORD)
        .expect("import-pk");
    assert_eq!(w.address, expected_addr);

    // Private-key imports don't yield a Mnemonic on unlock — the wallet
    // stores raw bytes; unlock returns Corrupt (Task 10 will add a
    // dedicated unlock_signer path).
    let err = mgr
        .unlock(w.wallet_id, TEST_PASSWORD)
        .expect_err("private-key unlock");
    assert!(
        matches!(err, WalletError::Corrupt { .. }),
        "private-key unlock must surface Corrupt, got: {err:?}"
    );
}

#[test]
fn wrong_password_fails_to_unlock() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let mgr = WalletManager::open_at(tmp.path().to_path_buf()).expect("open");
    let w = mgr
        .create_wallet("wrong-pw-test", TEST_PASSWORD)
        .expect("create");

    let err = mgr
        .unlock(w.wallet_id, b"definitely-the-wrong-password")
        .expect_err("wrong password must fail");
    assert!(
        matches!(err, WalletError::Crypto(_)),
        "wrong password must surface Crypto error, got: {err:?}"
    );
}

#[test]
fn delete_wallet_removes_disk_file() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let mgr = WalletManager::open_at(tmp.path().to_path_buf()).expect("open");
    let w = mgr
        .create_wallet("delete-test", TEST_PASSWORD)
        .expect("create");
    let enc_path = tmp
        .path()
        .join("sepolia")
        .join(format!("{}.enc", w.wallet_id));
    assert!(enc_path.exists());

    mgr.delete_wallet(w.wallet_id).expect("delete");
    assert!(!enc_path.exists(), "delete must remove the .enc file");

    let err = mgr.delete_wallet(w.wallet_id).expect_err("second delete");
    assert!(matches!(err, WalletError::NotFound { .. }));
}

#[test]
fn list_wallets_reflects_create_and_delete() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let mgr = WalletManager::open_at(tmp.path().to_path_buf()).expect("open");
    let initial = mgr.list_wallets().expect("list").len();

    let w1 = mgr.create_wallet("a", TEST_PASSWORD).expect("a");
    let _w2 = mgr.create_wallet("b", TEST_PASSWORD).expect("b");
    assert_eq!(mgr.list_wallets().expect("list").len(), initial + 2);

    mgr.delete_wallet(w1.wallet_id).expect("delete a");
    assert_eq!(mgr.list_wallets().expect("list").len(), initial + 1);
}

#[test]
fn empty_password_rejected() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let mgr = WalletManager::open_at(tmp.path().to_path_buf()).expect("open");
    let err = mgr
        .create_wallet("empty-pw", b"")
        .expect_err("empty password must reject");
    assert!(matches!(err, WalletError::Crypto(_)));
}

#[test]
fn re_open_scans_existing_disk_wallets() {
    let tmp = tempfile::tempdir().expect("tmpdir");

    let mgr = WalletManager::open_at(tmp.path().to_path_buf()).expect("open #1");
    let w1 = mgr.create_wallet("alpha", TEST_PASSWORD).expect("a");
    let w2 = mgr.create_wallet("beta", TEST_PASSWORD).expect("b");
    drop(mgr);

    let mgr2 = WalletManager::open_at(tmp.path().to_path_buf()).expect("open #2");
    let listed = mgr2.list_wallets().expect("list");
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().any(|w| w.wallet_id == w1.wallet_id));
    assert!(listed.iter().any(|w| w.wallet_id == w2.wallet_id));

    let _ = fs::metadata(tmp.path().join("sepolia"));
}
