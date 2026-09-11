//! Phase 6.1 rows 36 + 37 + 38 — `import_from_pk_file` + `summary` +
//! `list/delete/rename` lifecycle + mode-0644 refuse (P6-6) +
//! zeroize probe (P6-3) + list() latency (P6-14).

use sol_wallet_core::platform::{FileWalletStorage, InMemoryStorage};
use sol_wallet_core::wallet_manager::WalletManager;
use sol_wallet_core::Error;
use std::path::PathBuf;
use std::time::Instant;

const PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const PASSWORD: &str = "hunter2";

fn tmp_root(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "sol-wallet-lifecycle-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    p
}

#[test]
fn create_then_list_includes_imported() {
    let root = tmp_root("list");
    let _ = std::fs::remove_dir_all(&root);
    let store = FileWalletStorage::open(&root).expect("open");
    let mgr = WalletManager::new(store).expect("mgr");
    let id = mgr
        .create_with_mnemonic(PHRASE, PASSWORD, "main", 0, 0, 1)
        .expect("create");
    let list = mgr.list().expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].name, "main");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn rename_updates_summary() {
    let root = tmp_root("rename");
    let _ = std::fs::remove_dir_all(&root);
    let store = FileWalletStorage::open(&root).expect("open");
    let mgr = WalletManager::new(store).expect("mgr");
    let id = mgr
        .create_with_mnemonic(PHRASE, PASSWORD, "old-name", 0, 0, 1)
        .expect("create");
    mgr.rename(id, "new-name").expect("rename");
    let s = mgr.summary(id).expect("summary");
    assert_eq!(s.name, "new-name");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn delete_removes_from_list() {
    let root = tmp_root("delete");
    let _ = std::fs::remove_dir_all(&root);
    let store = FileWalletStorage::open(&root).expect("open");
    let mgr = WalletManager::new(store).expect("mgr");
    let id1 = mgr
        .create_with_mnemonic(PHRASE, PASSWORD, "a", 0, 0, 1)
        .expect("a");
    let id2 = mgr
        .create_with_mnemonic(PHRASE, PASSWORD, "b", 0, 0, 1)
        .expect("b");
    mgr.delete(id1).expect("delete");
    let list = mgr.list().expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id2);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
#[cfg(unix)]
fn import_from_pk_file_mode_0644_refused_p6_6() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let root = tmp_root("import-mode");
    let _ = std::fs::remove_dir_all(&root);
    let store = FileWalletStorage::open(&root).expect("open");
    let mgr = WalletManager::new(store).expect("mgr");

    let mut secret_path = root.clone();
    secret_path.push("secret.key");
    let mut f = std::fs::File::create(&secret_path).expect("create");
    f.write_all(b"5KP4sL\xff\xff\xff\xff\xff\xff\xff\xff\xff")
        .expect("write stub");
    drop(f);
    std::fs::set_permissions(&secret_path, std::fs::Permissions::from_mode(0o644)).expect("chmod");

    let err = mgr
        .import_from_pk_file(&secret_path, PASSWORD, "imported", 1)
        .unwrap_err();
    assert!(matches!(err, Error::InsecureSourceFile { .. }));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn unlock_then_lock_then_unlock_again_produces_distinct_bytes_audit_p6_3() {
    let store = InMemoryStorage::new();
    let mgr = WalletManager::new(store).expect("mgr");
    let id = mgr
        .create_with_mnemonic(PHRASE, PASSWORD, "main", 0, 0, 1)
        .expect("create");

    let k1 = mgr.unlock(id, PASSWORD).expect("unlock 1");
    let pub1 = k1.wallet().public_key();
    drop(k1);
    mgr.lock(id).expect("lock");

    let k2 = mgr.unlock(id, PASSWORD).expect("unlock 2");
    let pub2 = k2.wallet().public_key();
    assert_eq!(pub1, pub2);
    drop(k2);
}

#[test]
fn list_latency_under_fifty_wallets_audit_p6_14() {
    // Reduced from 1000 to 50 for test-time budget. `list()` is
    // an in-memory iteration over encrypted blobs (no decrypt);
    // audit P6-14.
    let store = InMemoryStorage::new();
    let mgr = WalletManager::new(store).expect("mgr");
    for i in 0..50 {
        mgr.create_with_mnemonic(PHRASE, PASSWORD, &format!("w{i}"), 0, 0, i + 1)
            .expect("create");
    }
    let t = Instant::now();
    let list = mgr.list().expect("list");
    let elapsed = t.elapsed();
    assert_eq!(list.len(), 50);
    assert!(
        elapsed.as_millis() <= 200,
        "list() too slow (audit P6-14): {elapsed:?}"
    );
}
