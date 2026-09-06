//! Phase 5 — platform abstraction layer (PAL) root.
//!
//! Four traits live in this module:
//!
//! - [`storage::WalletStorage`] — encrypted wallet blobs at rest
//! - [`info::PlatformInfo`] — per-OS data dirs + app identity
//! - [`network::NetworkClient`] — TLS-root + timeout policy
//! - [`clock::Clock`] — wall-clock time source for tx timestamps
//!
//! Per-platform impls:
//!
//! - Desktop (`#[cfg(not(any(target_os = "ios", target_os = "android")))]`) —
//!   `desktop::FileWalletStorage`, `desktop::DesktopPlatformInfo`,
//!   `desktop::DesktopNetworkClient`, `desktop::SystemClock`
//! - iOS (`#[cfg(target_os = "ios")]`) — `ios::KeychainWalletStorage`,
//!   `ios::IosPlatformInfo`, `ios::IosNetworkClient`, `ios::IosClock`
//! - Android (`#[cfg(target_os = "android")]`) —
//!   `android::EncryptedFileWalletStorage`,
//!   `android::AndroidPlatformInfo`,
//!   `android::AndroidNetworkClient`, `android::AndroidClock`
//!
//! Per Task 4.6 — Compile-time platform selection:
//!
//! | target_os | `DefaultStorage` | `DefaultPlatformInfo` | `DefaultNetworkClient` | `DefaultClock` |
//! |-----------|-----------------|-----------------------|------------------------|---------------|
//! | `ios`     | `ios::KeychainWalletStorage` | `ios::IosPlatformInfo` | `ios::IosNetworkClient` | `ios::IosClock` |
//! | `android` | `android::EncryptedFileWalletStorage` | `android::AndroidPlatformInfo` | `android::AndroidNetworkClient` | `android::AndroidClock` |
//! | other     | `desktop::FileWalletStorage` | `desktop::DesktopPlatformInfo` | `desktop::DesktopNetworkClient` | `desktop::SystemClock` |
//!
//! `default_storage()`, `default_platform_info()`, `default_network_client()`,
//! `default_clock()` factory functions mirror the type aliases.
//!
//! `test` is a final sub-module (not gated — useful in any test
//! build) exposing `InMemoryStorage`, `StaticInfo`, `MockNetworkClient`,
//! `MockClock`.

pub mod clock;
pub mod info;
pub mod network;
pub mod storage;

pub mod android;
pub mod desktop;
pub mod ios;

/// Test-only PAL impls. Available in any build (not `cfg(test)`-
/// gated) so integration tests in `tests/wallet_persistence.rs`
/// can import them via `crate::platform::test::*`. The types
/// themselves carry `#[allow(dead_code)]` where appropriate so the
/// crate doesn't warn in production builds that simply don't use them.
pub mod test;

pub use clock::Clock;
pub use info::PlatformInfo;
pub use network::NetworkClient;
pub use storage::WalletStorage;

// ─── Type aliases ────────────────────────────────────────────────────────

#[cfg(target_os = "ios")]
pub type DefaultStorage = ios::KeychainWalletStorage;
#[cfg(target_os = "ios")]
pub type DefaultPlatformInfo = ios::IosPlatformInfo;
#[cfg(target_os = "ios")]
pub type DefaultNetworkClient = ios::IosNetworkClient;
#[cfg(target_os = "ios")]
pub type DefaultClock = ios::IosClock;

#[cfg(target_os = "android")]
pub type DefaultStorage = android::EncryptedFileWalletStorage;
#[cfg(target_os = "android")]
pub type DefaultPlatformInfo = android::AndroidPlatformInfo;
#[cfg(target_os = "android")]
pub type DefaultNetworkClient = android::AndroidNetworkClient;
#[cfg(target_os = "android")]
pub type DefaultClock = android::AndroidClock;

#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub type DefaultStorage = desktop::FileWalletStorage;
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub type DefaultPlatformInfo = desktop::DesktopPlatformInfo;
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub type DefaultNetworkClient = desktop::DesktopNetworkClient;
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub type DefaultClock = desktop::SystemClock;

// ─── Factory functions ───────────────────────────────────────────────────

#[cfg(target_os = "ios")]
pub fn default_storage() -> DefaultStorage {
    ios::KeychainWalletStorage
}
#[cfg(target_os = "ios")]
pub fn default_platform_info() -> DefaultPlatformInfo {
    ios::IosPlatformInfo::new("tron", "0.1.0")
}
#[cfg(target_os = "ios")]
pub fn default_network_client() -> DefaultNetworkClient {
    ios::IosNetworkClient
}
#[cfg(target_os = "ios")]
pub fn default_clock() -> DefaultClock {
    ios::IosClock
}

#[cfg(target_os = "android")]
pub fn default_storage() -> DefaultStorage {
    android::EncryptedFileWalletStorage
}
#[cfg(target_os = "android")]
pub fn default_platform_info() -> DefaultPlatformInfo {
    android::AndroidPlatformInfo::new("tron", "0.1.0")
}
#[cfg(target_os = "android")]
pub fn default_network_client() -> DefaultNetworkClient {
    android::AndroidNetworkClient
}
#[cfg(target_os = "android")]
pub fn default_clock() -> DefaultClock {
    android::AndroidClock
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn default_storage() -> DefaultStorage {
    desktop::FileWalletStorage::with_dir(
        default_platform_info()
            .data_dir()
            .join(default_platform_info().app_name())
            .join("wallets"),
    )
    .expect("desktop wallet storage dir")
}
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn default_platform_info() -> DefaultPlatformInfo {
    desktop::DesktopPlatformInfo::new("tron", "0.1.0")
}
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn default_network_client() -> DefaultNetworkClient {
    desktop::DesktopNetworkClient::new()
}
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn default_clock() -> DefaultClock {
    desktop::SystemClock
}
