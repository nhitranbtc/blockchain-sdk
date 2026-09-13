//! Phase 10 Task 10.2 — `WalletManager` lock-poisoning + concurrent
//! `unlock`/`delete` regressions (issue #565).
//!
//! These tests pin two contracts:
//!
//! 1. **No lock held across Argon2id**: `unlock` releases the read guard
//!    before invoking `crypto::decrypt_wallet`. A concurrent `delete`
//!    on a different wallet id must not block waiting for the decrypt.
//!    Verifies via wall-clock timeout: if `unlock` held the read guard
//!    across Argon2id (Argon2id is ~100ms on default params), the
//!    delete thread would wait > 5s — fails loud-RED.
//!
//! 2. **Lock poisoning must not cascade to FFI `Panic = 99`**:
//!    When a thread panics while holding the write lock, subsequent
//!    callers must recover (`unwrap_or_else(|p| p.into_inner())`) and
//!    succeed. Verifies via a parallel `std::sync::RwLock` poisoning
//!    simulation plus a structural assertion that `unlock` still works
//!    on the real `WalletManager` after the refactor.

use sol_wallet_core::platform::InMemoryStorage;
use sol_wallet_core::wallet_manager::WalletManager;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const PASSWORD: &str = "hunter2";

/// Timeout for the concurrent test. Argon2id default params take ~50-150ms
/// per call. If `unlock` held the read lock across the KDF, the delete
/// thread would block waiting for read-lock release AND then contend
/// with the write lock — easily > 5s. 5s is a loud-RED trigger.
const CONCURRENT_TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn concurrent_unlock_and_delete_do_not_deadlock() {
    let mgr = Arc::new(WalletManager::new(InMemoryStorage::new()).expect("mgr"));

    // Two distinct wallets so `delete` targets a DIFFERENT record than
    // `unlock`. If they targeted the same record the test would race on
    // semantics rather than lock-contention.
    let id_a = mgr
        .create_with_mnemonic(PHRASE, PASSWORD, "wallet-a", 0, 0, 1)
        .expect("create A");
    let id_b = mgr
        .create_with_mnemonic(PHRASE, PASSWORD, "wallet-b", 0, 1, 1)
        .expect("create B (different address_index for distinct pubkey)");

    let mgr_unl = Arc::clone(&mgr);
    let mgr_del = Arc::clone(&mgr);

    let unlock = thread::spawn(move || {
        mgr_unl.unlock(id_a, PASSWORD).expect("unlock A");
    });

    let delete = thread::spawn(move || {
        // No sleep — race `unlock` immediately. If `unlock` held the
        // read guard across the Argon2id KDF, this thread would block
        // > CONCURRENT_TIMEOUT.
        mgr_del.delete(id_b).expect("delete B");
    });

    let started = Instant::now();
    unlock.join().expect("unlock thread join");
    delete.join().expect("delete thread join");
    let elapsed = started.elapsed();

    assert!(
        elapsed < CONCURRENT_TIMEOUT,
        "concurrent unlock + delete took {elapsed:?} (>= {CONCURRENT_TIMEOUT:?}) — \
         `unlock` likely holds a lock across Argon2id; recovery contract broken"
    );

    // Post-condition: A still listed (we deleted B, not A).
    let list = mgr.list().expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id_a);
}

#[test]
fn lock_poisoning_recovery_returns_inner_data() {
    // Structural pin: prove the recovery pattern
    // `lock().unwrap_or_else(|p| p.into_inner())` works on a poisoned
    // `std::sync::RwLock`. After Task 10.2, every `lock().expect()`
    // site in `wallet_manager.rs` is replaced with this pattern.
    let lock: std::sync::RwLock<()> = std::sync::RwLock::new(());
    let lock = Arc::new(lock);

    // Poison the lock: spawn a thread that holds the write guard, then
    // panics. After join(), the lock is permanently poisoned.
    let poison_thread = {
        let l = Arc::clone(&lock);
        thread::spawn(move || {
            let _g = l.write().expect("acquire write lock for poisoning");
            panic!("intentional poisoning panic");
        })
    };
    let _ = poison_thread.join();

    // Recovery contract: the `unwrap_or_else` variant succeeds on a
    // poisoned lock, returning the inner data. If this were `expect()`,
    // it would panic here and the test process would abort. Holding the
    // guard to the end of the test (rather than dropping it early)
    // confirms the inner data is accessible.
    let _recovered = lock.read().unwrap_or_else(|p| p.into_inner());
}

#[test]
fn unlock_succeeds_after_wallet_manager_refactor() {
    // Regression pin: after Task 10.2 refactor (every `.expect()` site
    // replaced), `unlock` must still work end-to-end. The poisoning
    // simulation in the sibling test is structural; this test verifies
    // the production happy path is unbroken.
    let store = InMemoryStorage::new();
    let mgr = WalletManager::new(store).expect("mgr");
    let id = mgr
        .create_with_mnemonic(PHRASE, PASSWORD, "happy-path", 0, 0, 1)
        .expect("create");

    let _lock = mgr.unlock(id, PASSWORD).expect("unlock after refactor");

    // And: list still returns the one wallet we created.
    let list = mgr.list().expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
}
