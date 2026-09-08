//! Phase 5 — `PlatformInfo` PAL trait.
//!
//! Per plan §Phase 5 Task 4.1: `data_dir / app_version / app_name /
//! is_mobile`. The core asks `&dyn PlatformInfo` for the OS-conventional
//! place to drop files (data dir), the user-visible app identity
//! (used in RPC `User-Agent` headers + CLI prompts), and whether the
//! runtime is a mobile host (drives transport-root choice — see
//! `NetworkClient`).
//!
//! **Why a PAL around platform info:** the data dir resolution
//! differs on every OS:
//!
//! - Linux: `~/.local/share/<app_name>/` (XDG)
//! - macOS: `~/Library/Application Support/<app_name>/`
//! - Windows: `%APPDATA%\<app_name>\`
//! - iOS: app sandbox `Documents/` (or `Library/Application Support/`)
//! - Android: `Context.getFilesDir()`
//!
//! The core never reaches for `directories` directly; the platform
//! impl returns whatever the OS expects.

use std::path::PathBuf;

/// Per-platform runtime metadata.
pub trait PlatformInfo: Send + Sync {
    /// Filesystem root the wallet binary should use for user data
    /// (wallets, node cache, logs).
    ///
    /// Implementations MAY create the directory on first read; they
    /// MUST NOT panic if it already exists.
    fn data_dir(&self) -> std::path::PathBuf;

    /// Human-readable app version (`"0.1.0"`).
    ///
    /// Used in HTTP `User-Agent` headers (`tron-wallet-core/0.1.0`)
    /// and in CLI `--version`. The plan tracks this for V0.1.5
    /// telemetry-friendly request tagging.
    fn app_version(&self) -> &'static str;

    /// Human-readable app name (`"tron"`).
    ///
    /// Used in CLI prompts and as the data-dir leaf. Must be stable
    /// across versions; renaming breaks every existing install's
    /// wallet lookup.
    fn app_name(&self) -> &'static str;

    /// `true` when the runtime is a mobile host (iOS or Android).
    ///
    /// Drives `NetworkClient::build_client` choosing OS trust roots
    /// vs webpki roots — per Round-1 grill Q6 mobile CI matrix.
    /// Defaults to `false` on every desktop impl.
    fn is_mobile(&self) -> bool;
}

/// Convenience: `data_dir` joined with `<app_name>/<sub>`. Returns
/// `<data_dir>/<app_name>/<sub>` (does NOT create the directory).
pub fn subdir(info: &dyn PlatformInfo, sub: &str) -> PathBuf {
    info.data_dir().join(info.app_name()).join(sub)
}
