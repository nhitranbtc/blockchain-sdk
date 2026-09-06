//! Phase 5 — Android PAL impls (Task 4.4).
//!
//! Same plan as `ios.rs`: Round-1 grill Q6 says NO mobile runtime
//! smoke in v0.1. This file ships compile-time stubs that implement
//! the four PAL traits so the crate compiles for
//! `aarch64-linux-android` per plan §Phase 5 Verification. Real
//! transport (EncryptedFile via Android Keystore via JNI bridge)
//! lives in v0.2.

use std::time::Duration;

use crate::error::{Error, Result};
use crate::platform::storage::WalletStorage;
use crate::platform::{Clock, NetworkClient, PlatformInfo};
use crate::wallet::id::WalletId;

// ─── PlatformInfo ────────────────────────────────────────────────────────

/// Android `PlatformInfo` impl. Returns the app's `filesDir()` at
/// runtime via the JNI bridge (`Context.getFilesDir()`); v0.1 stub
/// returns a placeholder path.
pub struct AndroidPlatformInfo {
    app_name: &'static str,
    app_version: &'static str,
}

impl AndroidPlatformInfo {
    pub const fn new(app_name: &'static str, app_version: &'static str) -> Self {
        Self {
            app_name,
            app_version,
        }
    }
}

impl PlatformInfo for AndroidPlatformInfo {
    fn data_dir(&self) -> std::path::PathBuf {
        // v0.1 stub — real path comes from Kotlin
        // `ContextInfo::filesDir()` JNI in v0.2.
        std::path::PathBuf::from("/data/user/0/io.tron.wallet/stub")
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

/// Android `EncryptedFile`-backed `WalletStorage` stub. Real impl
/// lives in the Kotlin JNI bridge (v0.2); v0.1 returns an error
/// from every call.
pub struct EncryptedFileWalletStorage;

impl WalletStorage for EncryptedFileWalletStorage {
    fn put_atomic(&self, _id: &WalletId, _blob: &[u8]) -> Result<()> {
        Err(Error::Wallet(
            "Android EncryptedFileWalletStorage stub: not implemented in v0.1; \
             wire via JNI bridge in v0.2"
                .into(),
        ))
    }

    fn put(&self, _id: &WalletId, _blob: &[u8]) -> Result<()> {
        Err(Error::Wallet(
            "Android EncryptedFileWalletStorage stub: not implemented in v0.1".into(),
        ))
    }

    fn get(&self, _id: &WalletId) -> Result<Option<Vec<u8>>> {
        Err(Error::Wallet(
            "Android EncryptedFileWalletStorage stub: not implemented in v0.1".into(),
        ))
    }

    fn list(&self) -> Result<Vec<WalletId>> {
        Err(Error::Wallet(
            "Android EncryptedFileWalletStorage stub: not implemented in v0.1".into(),
        ))
    }

    fn delete(&self, _id: &WalletId) -> Result<()> {
        Err(Error::Wallet(
            "Android EncryptedFileWalletStorage stub: not implemented in v0.1".into(),
        ))
    }
}

// ─── NetworkClient ───────────────────────────────────────────────────────

/// Android HTTP client. Mobile uses webpki bundled roots (Round-1
/// grill Q6). v0.1 stub returns an error.
pub struct AndroidNetworkClient;

impl NetworkClient for AndroidNetworkClient {
    fn build_client(&self) -> Result<reqwest::Client> {
        Err(Error::Node(
            "Android AndroidNetworkClient stub: not implemented in v0.1".into(),
        ))
    }

    fn default_rpc_url(&self) -> &'static str {
        "https://api.trongrid.io"
    }
}

// ─── Clock ───────────────────────────────────────────────────────────────

/// Android clock. Same `SystemTime` source as desktop/iOS; v0.2
/// will route through Kotlin's `SystemClock.elapsedRealtime()`.
pub struct AndroidClock;

impl Clock for AndroidClock {
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
