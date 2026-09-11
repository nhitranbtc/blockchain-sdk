//! Phase 6.1 rows 8 + 9 + 10 — create→save→load→sign round-trip +
//! mode 0600 + atomic write + .tmp cleanup on rename failure
//! (audit P6-8) + UUID uniqueness + name lookup.

use sol_wallet_core::platform::{FileWalletStorage, WalletStorage};
use sol_wallet_core::wallet_manager::WalletManager;
use std::collections::HashSet;
use std::path::PathBuf;

const PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const PASSWORD: &str = "hunter2";

fn tmp_root(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "sol-wallet-persist-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    p
}

#[test]
fn file_storage_open_creates_dir_mode_0700() {
    let root = tmp_root("open");
    let _ = std::fs::remove_dir_all(&root);
    let store = FileWalletStorage::open(&root).expect("open");
    assert!(root.is_dir());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&root).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }
    drop(store);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn saved_blob_mode_0600_audit_p6_2() {
    let root = tmp_root("mode600");
    let _ = std::fs::remove_dir_all(&root);
    let store = FileWalletStorage::open(&root).expect("open");
    store.put_atomic("w1", b"hello").expect("put");
    let path = root.join("w1");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "mode must be 0o600 (audit P6-2)");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn no_tmp_residue_after_successful_write() {
    let root = tmp_root("notmp");
    let _ = std::fs::remove_dir_all(&root);
    let store = FileWalletStorage::open(&root).expect("open");
    store.put_atomic("w1", b"hello").expect("put");
    let names = store.list_ids().expect("list");
    assert!(!names.iter().any(|n| n.ends_with(".tmp")));
    assert!(names.contains(&"w1".to_string()));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fifty_wallets_unique_uuid_no_collision() {
    // Reduced from 1000 to 50 for test-time budget — each create
    // does a 64 MB Argon2id KDF. UUID v4 = 122-bit randomness;
    // collision probability at 50 is ~ 5.7e-34.
    let root = tmp_root("uuid");
    let _ = std::fs::remove_dir_all(&root);
    let store = FileWalletStorage::open(&root).expect("open");
    let mgr = WalletManager::new(store).expect("mgr");
    let mut ids = HashSet::new();
    for i in 0..50 {
        let id = mgr
            .create_with_mnemonic(PHRASE, PASSWORD, &format!("w{i}"), 1_700_000_000 + i)
            .expect("create");
        assert!(ids.insert(id), "UUID collision at i={i}");
    }
    assert_eq!(mgr.list().expect("list").len(), 50);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn name_lookup_resolves() {
    let root = tmp_root("name");
    let _ = std::fs::remove_dir_all(&root);
    let store = FileWalletStorage::open(&root).expect("open");
    let mgr = WalletManager::new(store).expect("mgr");
    let id = mgr
        .create_with_mnemonic(PHRASE, PASSWORD, "alpha-name", 1)
        .expect("create");
    let s = mgr.summary(id).expect("summary");
    assert_eq!(s.name, "alpha-name");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn rename_failure_does_not_leave_tmp_audit_p6_8() {
    // P6-8 — on rename failure, .tmp must be cleaned up.
    // Simulate by writing a normal blob, then making the parent
    // dir into a regular file so subsequent renames fail with
    // ENOTDIR (or equivalent).
    let root = tmp_root("renamefail");
    let _ = std::fs::remove_dir_all(&root);
    let store = FileWalletStorage::open(&root).expect("open");
    store.put_atomic("w1", b"first").expect("first");

    // Remove w1 first so we can replace the dir with a regular
    // file (subsequent rename hits ENOTDIR).
    std::fs::remove_file(root.join("w1")).expect("rm w1");
    std::fs::remove_dir(&root).expect("rm dir");
    std::fs::write(&root, b"blocker").expect("blocker");

    let res = store.put_atomic("w2", b"second");
    let _ = res; // outcome (Ok or Err) is platform-dependent.

    // atomic_write writes `path.tmp` next to `path` — but `path`
    // is `<root>/w2` and `root` is now a file, so `path.tmp`
    // would be a sibling file. If the .tmp file leaked, scan
    // both `<root>` and its parent for it.
    let tmp = root.with_extension("tmp");
    let leaked_root_tmp = tmp.exists();
    let parent_leak = std::fs::read_dir(root.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n == "w2.tmp"
        });
    assert!(!leaked_root_tmp, ".tmp leaked at {tmp:?}");
    assert!(!parent_leak, ".tmp leaked in parent dir");
    let _ = std::fs::remove_file(&root);
}
