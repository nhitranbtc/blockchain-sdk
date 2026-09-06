//! Phase 5 — test PAL impls (Task 4.5).
//!
//! - `InMemoryStorage` — `Arc<Mutex<BTreeMap<WalletId, Vec<u8>>>>`,
//!   no atomicity machinery (the storage trait's atomicity guarantee
//!   applies to crash-safety, not thread-safety; this impl is
//!   process-local and is exercised by single-thread test code).
//! - `StaticInfo` — compile-time `app_name` + `app_version` +
//!   `data_dir` triple; `is_mobile` is hard-coded.
//! - `MockNetworkClient` — returns a `reqwest::Client` configured
//!   against the operator-supplied URL; tests that need to inject
//!   a custom transport implement `NetworkClient` themselves.
//! - `MockClock` — pinned-time clock with `advance` for tests that
//!   exercise expiration windows.
//!
//! These types live under `crate::platform::test::*` (gated behind
//! `#[cfg(test)]` blocks where the impl matters) so production
//! builds don't carry test-only dependency edges.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::error::{Error, Result};
use crate::platform::storage::WalletStorage;
use crate::platform::{Clock, NetworkClient, PlatformInfo};
use crate::wallet::id::WalletId;

// ─── InMemoryStorage ──────────────────────────────────────────────────────

/// Thread-safe in-memory wallet blob store. `BTreeMap` rather than
/// `HashMap` so `list()` returns a deterministic order — easier to
/// assert against in snapshot-style tests.
#[derive(Clone, Default)]
pub struct InMemoryStorage {
    inner: Arc<Mutex<BTreeMap<WalletId, Vec<u8>>>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of blobs currently held. Test-only helper.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.inner.lock().expect("poisoned").len()
    }

    /// `true` when no blobs are held.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl WalletStorage for InMemoryStorage {
    fn put_atomic(&self, id: &WalletId, blob: &[u8]) -> Result<()> {
        self.put(id, blob)
    }

    fn put(&self, id: &WalletId, blob: &[u8]) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| Error::Wallet(format!("in-memory lock poisoned: {e}")))?;
        guard.insert(*id, blob.to_vec());
        Ok(())
    }

    fn get(&self, id: &WalletId) -> Result<Option<Vec<u8>>> {
        let guard = self
            .inner
            .lock()
            .map_err(|e| Error::Wallet(format!("in-memory lock poisoned: {e}")))?;
        Ok(guard.get(id).cloned())
    }

    fn list(&self) -> Result<Vec<WalletId>> {
        let guard = self
            .inner
            .lock()
            .map_err(|e| Error::Wallet(format!("in-memory lock poisoned: {e}")))?;
        Ok(guard.keys().copied().collect())
    }

    fn delete(&self, id: &WalletId) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| Error::Wallet(format!("in-memory lock poisoned: {e}")))?;
        guard.remove(id);
        Ok(())
    }
}

impl std::fmt::Debug for InMemoryStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let guard = self.inner.lock().expect("poisoned");
        f.debug_struct("InMemoryStorage")
            .field("count", &guard.len())
            .finish()
    }
}

// ─── StaticInfo ──────────────────────────────────────────────────────────

/// Compile-time platform metadata. Used by tests that need a
/// `PlatformInfo` without pulling in `directories` (which on the
/// test runner is fine; we keep the type for parity + small-CI benefits).
#[derive(Copy, Clone, Debug)]
pub struct StaticInfo {
    pub app_name: &'static str,
    pub app_version: &'static str,
    pub data_dir: &'static std::path::Path,
    pub is_mobile: bool,
}

impl PlatformInfo for StaticInfo {
    fn data_dir(&self) -> std::path::PathBuf {
        self.data_dir.to_path_buf()
    }

    fn app_version(&self) -> &'static str {
        self.app_version
    }

    fn app_name(&self) -> &'static str {
        self.app_name
    }

    fn is_mobile(&self) -> bool {
        self.is_mobile
    }
}

// ─── MockNetworkClient ───────────────────────────────────────────────────

/// Minimal `NetworkClient` for tests. Returns a default
/// `reqwest::Client` (real HTTP) plus a configurable default URL.
#[derive(Copy, Clone, Debug)]
pub struct MockNetworkClient {
    pub rpc_url: &'static str,
}

impl MockNetworkClient {
    pub const fn new(rpc_url: &'static str) -> Self {
        Self { rpc_url }
    }
}

impl Default for MockNetworkClient {
    fn default() -> Self {
        Self::new("https://api.trongrid.io")
    }
}

impl NetworkClient for MockNetworkClient {
    fn build_client(&self) -> Result<reqwest::Client> {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Node(format!("build mock client: {e}")))
    }

    fn default_rpc_url(&self) -> &'static str {
        self.rpc_url
    }
}

// ─── MockClock ───────────────────────────────────────────────────────────

/// Pinned-time clock. Production code uses `SystemClock`; tests
/// inject `MockClock { millis: 0, .. }` (or any other anchor) so
/// time-dependent assertions hold exactly.
#[derive(Copy, Clone, Debug)]
pub struct MockClock {
    /// Current `now_millis()` value.
    pub millis: i64,
}

impl MockClock {
    pub const fn new(millis: i64) -> Self {
        Self { millis }
    }

    /// Advance the pinned time. Negative values rewind the clock —
    /// production wouldn't, but tests may want to simulate NTP
    /// correction.
    pub fn advance(&mut self, delta_millis: i64) {
        self.millis = self.millis.saturating_add(delta_millis);
    }
}

impl Default for MockClock {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Clock for MockClock {
    fn now_millis(&self) -> i64 {
        self.millis
    }

    fn sleep(&self, _duration: Duration) {
        // Tests do not advance time by waiting; advance() is the
        // explicit knob. Calling sleep in a test runs a real
        // thread sleep, which most tests want to avoid — so we
        // no-op. Production falls back to `SystemClock::sleep`.
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_storage_round_trip() {
        let store = InMemoryStorage::new();
        let id = WalletId::new();
        assert!(store.get(&id).unwrap().is_none());

        store.put(&id, b"hello").unwrap();
        assert_eq!(
            store.get(&id).unwrap().as_deref(),
            Some(b"hello".as_slice())
        );

        let listed = store.list().unwrap();
        assert_eq!(listed, vec![id]);

        store.delete(&id).unwrap();
        assert!(store.get(&id).unwrap().is_none());
    }

    #[test]
    fn mock_clock_advance() {
        let mut clock = MockClock::new(1_000);
        assert_eq!(clock.now_millis(), 1_000);
        clock.advance(500);
        assert_eq!(clock.now_millis(), 1_500);
        clock.advance(-200);
        assert_eq!(clock.now_millis(), 1_300);
    }

    #[test]
    fn static_info_round_trip() {
        let info = StaticInfo {
            app_name: "tron-test",
            app_version: "0.1.0",
            data_dir: std::path::Path::new("/tmp/tron-test"),
            is_mobile: false,
        };
        assert_eq!(info.app_name(), "tron-test");
        assert_eq!(info.app_version(), "0.1.0");
        assert!(!info.is_mobile());
        assert_eq!(info.data_dir(), std::path::PathBuf::from("/tmp/tron-test"));
    }
}
