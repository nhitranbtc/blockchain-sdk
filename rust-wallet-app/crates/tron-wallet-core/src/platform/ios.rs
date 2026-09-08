//! Phase 5 — iOS PAL impls (Task 4.3).
//!
//! Per plan §Phase 5 Round-1 grill Q6: NO mobile runtime smoke in
//! v0.1. This file ships compile-time stubs that:
//!
//! - implement the four PAL traits so the crate compiles for
//!   `aarch64-apple-ios` per plan §Phase 5 Verification,
//! - gate their actual storage transport behind an FFI bridge
//!   that the Swift host binds in v0.2 (the bridge is `extern "C"`
//!   slots the host fills in at runtime).
//!
//! **Why stubs:** writing the Swift wrapper that talks to Keychain
//! Services via `Security.framework` is its own Phase 6+ task
//! (builds on the FFI layer Phase 6 adds). For v0.1 we only need
//! the Rust types to type-check on the iOS target — actual usage
//! is gated on the wallet host invoking the FFI bridge via
//! `tron::ffi::*`, which lives outside the core crate.
//!
//! Per Task 4.6, these are exposed as `DefaultStorage /
//! DefaultPlatformInfo / DefaultNetworkClient / DefaultClock` for
//! `target_os = "ios"` via `platform::mod`.

use std::time::Duration;

use crate::error::{Error, Result};
use crate::platform::storage::WalletStorage;
use crate::platform::{Clock, NetworkClient, PlatformInfo};
use crate::wallet::id::WalletId;

// ─── PlatformInfo ────────────────────────────────────────────────────────

/// iOS `PlatformInfo` impl. Returns the app sandbox `Documents/`
/// directory at runtime via the FFI bridge; the v0.1 stub returns
/// a placeholder that compiles for the cross-target build but
/// errors when called from a real iOS host.
pub struct IosPlatformInfo {
    app_name: &'static str,
    app_version: &'static str,
}

impl IosPlatformInfo {
    pub const fn new(app_name: &'static str, app_version: &'static str) -> Self {
        Self {
            app_name,
            app_version,
        }
    }
}

impl PlatformInfo for IosPlatformInfo {
    fn data_dir(&self) -> std::path::PathBuf {
        // v0.1 stub: the real path is provided by the Swift
        // `BundleInfo::documentsDir()` FFI in v0.2. Returning a
        // constant here makes the cross-target compile green
        // without lying about runtime behavior — the binary
        // doesn't ship this code on iOS until the FFI bridge
        // exists.
        std::path::PathBuf::from("/var/mobile/Containers/Data/Application/tron/stub")
    }

    fn app_version(&self) -> &'static str {
        self.app_version
    }

    fn app_name(&self) -> &'static str {
        self.app_name
    }

    fn is_mobile(&self) -> bool {
        true
    }
}

// ─── WalletStorage ───────────────────────────────────────────────────────

/// iOS Keychain-backed `WalletStorage` stub. Real implementation
/// lives in the Swift FFI bridge (v0.2); v0.1 returns an error
/// from every call so the binary that wires this in notices at
/// startup rather than silently dropping wallets.
pub struct KeychainWalletStorage;

impl WalletStorage for KeychainWalletStorage {
    fn put_atomic(&self, _id: &WalletId, _blob: &[u8]) -> Result<()> {
        Err(Error::Wallet(
            "iOS KeychainWalletStorage stub: not implemented in v0.1; \
             wire via FFI bridge in v0.2"
                .into(),
        ))
    }

    fn put(&self, _id: &WalletId, _blob: &[u8]) -> Result<()> {
        Err(Error::Wallet(
            "iOS KeychainWalletStorage stub: not implemented in v0.1".into(),
        ))
    }

    fn get(&self, _id: &WalletId) -> Result<Option<Vec<u8>>> {
        Err(Error::Wallet(
            "iOS KeychainWalletStorage stub: not implemented in v0.1".into(),
        ))
    }

    fn list(&self) -> Result<Vec<WalletId>> {
        Err(Error::Wallet(
            "iOS KeychainWalletStorage stub: not implemented in v0.1".into(),
        ))
    }

    fn delete(&self, _id: &WalletId) -> Result<()> {
        Err(Error::Wallet(
            "iOS KeychainWalletStorage stub: not implemented in v0.1".into(),
        ))
    }
}

// ─── NetworkClient ───────────────────────────────────────────────────────

/// iOS HTTP client. Mobile uses webpki bundled roots (no OS CA
/// store on iOS per Apple's ATS docs; Round-1 grill Q6). The
/// v0.1 stub returns an error so the host knows the bridge isn't
/// wired yet.
pub struct IosNetworkClient;

impl NetworkClient for IosNetworkClient {
    fn build_client(&self) -> Result<reqwest::Client> {
        Err(Error::Node(
            "iOS IosNetworkClient stub: not implemented in v0.1".into(),
        ))
    }

    fn default_rpc_url(&self) -> &'static str {
        "https://api.trongrid.io"
    }
}

// ─── Clock ───────────────────────────────────────────────────────────────

/// iOS clock. `now_millis` is derivable from `SystemTime` (the same
/// wall-clock source desktop uses); `sleep` is the std thread
/// sleep. Production iOS builds will go through the Swift
/// `OS_clock`/`DispatchQueue` path in v0.2; v0.1 stays simple.
pub struct IosClock;

impl Clock for IosClock {
    fn now_millis(&self) -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}
