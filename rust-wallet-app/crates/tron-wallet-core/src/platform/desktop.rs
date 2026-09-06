//! Phase 5 — desktop PAL impls.
//!
//! Per plan §Phase 5 Task 4.2:
//!
//! - `FileWalletStorage` — `<data_dir>/<app_name>/wallets/<id>.bin`,
//!   mode 0600 on Unix (directories set 0700), atomic write via temp
//!   + rename. On Windows the mode is irrelevant; the directory ACL
//!     keeps non-owner readers out.
//! - `DesktopPlatformInfo` — uses the `directories` crate to resolve
//!   `data_dir()` per OS (Linux: `~/.local/share/`, macOS:
//!   `~/Library/Application Support/`, Windows: `%APPDATA%`).
//! - `DesktopNetworkClient` — `reqwest::Client::builder().timeout(30s)`
//!   (system trust store loaded via `rustls-native-certs` per
//!   Round-1 grill Q6 mobile CI matrix — desktop uses OS trust
//!   store, mobile uses webpki).
//! - `SystemClock` — wall-clock `SystemTime::now()` + `Thread::sleep`.
//!
//! Per Task 4.6, these are exposed as the default `DefaultStorage /
//! DefaultPlatformInfo / DefaultNetworkClient / DefaultClock` type
//! aliases on non-`ios`/`android` targets via `platform::mod`.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::platform::storage::WalletStorage;
use crate::platform::{Clock, NetworkClient, PlatformInfo};
use crate::wallet::id::WalletId;

// ─── PlatformInfo ────────────────────────────────────────────────────────

/// Desktop platform info. Wraps `directories::ProjectDirs` so the
/// per-OS paths are conventional (XDG on Linux, Apple's spec on
/// macOS, `APPDATA` on Windows).
///
/// **Plan deviation:** the plan sketched `SystemDirsInfo` as a
/// struct holding per-OS data dirs. We collapse that into a
/// `PlatformInfo` trait impl that lazily computes `data_dir()`
/// each call — keeps the contract uniform with the test impl and
/// avoids caching a directory handle that the rest of the crate
/// never reads.
pub struct DesktopPlatformInfo {
    app_name: &'static str,
    app_version: &'static str,
}

impl DesktopPlatformInfo {
    /// Construct with conventional app identity. The binary
    /// (`tron` CLI) injects name + version at startup; tests
    /// construct directly.
    pub const fn new(app_name: &'static str, app_version: &'static str) -> Self {
        Self {
            app_name,
            app_version,
        }
    }
}

impl PlatformInfo for DesktopPlatformInfo {
    fn data_dir(&self) -> PathBuf {
        directories::ProjectDirs::from("io", "", self.app_name)
            .map(|p| p.data_dir().to_path_buf())
            .unwrap_or_else(|| {
                // Fallback for sandboxed environments where
                // `directories` returns None (rare; e.g. stripped
                // HOME). $HOME or /tmp work as last-resort.
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join(format!(".{}", self.app_name))
            })
    }

    fn app_version(&self) -> &'static str {
        self.app_version
    }

    fn app_name(&self) -> &'static str {
        self.app_name
    }

    fn is_mobile(&self) -> bool {
        false
    }
}

// ─── WalletStorage ───────────────────────────────────────────────────────

/// Per-blob file in `<data_dir>/<app_name>/wallets/<id>.bin`.
///
/// Atomic write: `write_to_temp → fsync → rename` over the existing
/// blob. `rename(2)` is atomic on every POSIX target we ship to;
/// Windows uses `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`
/// which Rust's `std::fs::rename` selects automatically.
pub struct FileWalletStorage {
    base_dir: PathBuf,
}

impl FileWalletStorage {
    /// Construct from a `PlatformInfo`. Resolves
    /// `<data_dir>/<app_name>/wallets/` and creates it (0700 on Unix)
    /// if absent.
    pub fn from_info(info: &dyn PlatformInfo) -> Result<Self> {
        let dir = crate::platform::info::subdir(info, "wallets");
        Self::with_dir(dir)
    }

    /// Construct at an explicit directory. Tests use this to point
    /// at `tempfile::tempdir()`; production wires through
    /// `from_info`.
    pub fn with_dir(base_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_dir)
            .map_err(|e| Error::Wallet(format!("create wallet dir {}: {e}", base_dir.display())))?;
        // Restrict directory permissions on Unix. Windows ignores
        // `mode` (no POSIX mode bits); the user's profile ACL is
        // what governs access on that OS.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o700);
            fs::set_permissions(&base_dir, perms).map_err(|e| {
                Error::Wallet(format!("chmod 0700 wallet dir {}: {e}", base_dir.display()))
            })?;
        }
        Ok(Self { base_dir })
    }

    /// Resolve the per-wallet file path for `id`. Used by every
    /// method; centralizing prevents divergent path layouts across
    /// `put` / `get` / `delete`.
    fn path_for(&self, id: &WalletId) -> PathBuf {
        self.base_dir.join(format!("{}.bin", id.to_hex()))
    }
}

impl WalletStorage for FileWalletStorage {
    fn put_atomic(&self, id: &WalletId, blob: &[u8]) -> Result<()> {
        let final_path = self.path_for(id);
        let tmp_path = self.base_dir.join(format!(".{}.tmp", id.to_hex()));

        // Write to temp, fsync, rename. If we crash between write
        // and rename, the previous `final_path` is intact.
        {
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp_path)
                .map_err(|e| Error::Wallet(format!("open temp {}: {e}", tmp_path.display())))?;
            f.write_all(blob)
                .map_err(|e| Error::Wallet(format!("write temp {}: {e}", tmp_path.display())))?;
            f.sync_all()
                .map_err(|e| Error::Wallet(format!("fsync temp {}: {e}", tmp_path.display())))?;
        }
        fs::rename(&tmp_path, &final_path).map_err(|e| {
            Error::Wallet(format!(
                "rename {} -> {}: {e}",
                tmp_path.display(),
                final_path.display()
            ))
        })?;

        // Tighten to 0600 on Unix (Windows ignores).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            fs::set_permissions(&final_path, perms).map_err(|e| {
                Error::Wallet(format!(
                    "chmod 0600 wallet blob {}: {e}",
                    final_path.display()
                ))
            })?;
        }

        Ok(())
    }

    fn put(&self, id: &WalletId, blob: &[u8]) -> Result<()> {
        let path = self.path_for(id);
        fs::write(&path, blob)
            .map_err(|e| Error::Wallet(format!("write {}: {e}", path.display())))?;
        Ok(())
    }

    fn get(&self, id: &WalletId) -> Result<Option<Vec<u8>>> {
        let path = self.path_for(id);
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::Wallet(format!("read {}: {e}", path.display()))),
        }
    }

    fn list(&self) -> Result<Vec<WalletId>> {
        let mut ids = Vec::new();
        let entries = fs::read_dir(&self.base_dir)
            .map_err(|e| Error::Wallet(format!("read_dir {}: {e}", self.base_dir.display())))?;
        for entry in entries {
            let entry = entry.map_err(|e| Error::Wallet(format!("read_dir entry: {e}")))?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(hex_id) = name.strip_suffix(".bin") else {
                // Skip stray temp files (`.id.tmp`) or anything else
                // that doesn't match our naming.
                continue;
            };
            if let Ok(id) = hex_id.parse::<WalletId>() {
                ids.push(id);
            }
        }
        ids.sort();
        Ok(ids)
    }

    fn delete(&self, id: &WalletId) -> Result<()> {
        let path = self.path_for(id);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Error::Wallet(format!("remove {}: {e}", path.display()))),
        }
    }
}

// ─── NetworkClient ───────────────────────────────────────────────────────

/// Default TronGrid endpoint the CLI uses when no operator override
/// is set. Per plan §Phase 5 Task 4.2.
pub const DEFAULT_MAINNET_RPC_URL: &str = "https://api.trongrid.io";

/// Desktop HTTP client. System CAs via `rustls-native-certs`, 30-second
/// timeout. No SPKI pinning — the caller's choice between
/// `pinned://<spki>@host` and a plain URL is decided at the
/// `chain::TronGridClient` boundary per Phase 2 SPKI convention.
pub struct DesktopNetworkClient {
    rpc_url: &'static str,
}

impl DesktopNetworkClient {
    pub const fn new() -> Self {
        Self {
            rpc_url: DEFAULT_MAINNET_RPC_URL,
        }
    }

    /// Custom default RPC URL (test rigs, private nets). The
    /// `&'static str` constraint is intentional: `reqwest::Client`
    /// construction outlives `build_client`'s arguments and we
    /// don't want to clone a `String` per call.
    pub const fn with_rpc(rpc_url: &'static str) -> Self {
        Self { rpc_url }
    }
}

impl Default for DesktopNetworkClient {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkClient for DesktopNetworkClient {
    fn build_client(&self) -> Result<reqwest::Client> {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Node(format!("build reqwest client: {e}")))
    }

    fn default_rpc_url(&self) -> &'static str {
        self.rpc_url
    }
}

// ─── Clock ───────────────────────────────────────────────────────────────

/// Wall-clock time source for production. `now_millis` reads
/// `SystemTime`; `sleep` delegates to `std::thread::sleep` (the
/// trait is sync by design — see `platform::clock::Clock`).
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_millis(&self) -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            // System time before epoch is a clock problem the
            // wallet can't fix; fall back to 0 so callers don't
            // panic. Transactions would fail TronGrid validation
            // and the operator would notice.
            .unwrap_or(0)
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn file_storage_create_list_get_delete() {
        let dir = tempdir().expect("tempdir");
        let store = FileWalletStorage::with_dir(dir.path().to_path_buf()).expect("storage");

        let id = WalletId::new();
        assert!(store.get(&id).expect("get").is_none());

        store.put_atomic(&id, b"hello world").expect("put_atomic");
        let got = store.get(&id).expect("get").expect("present");
        assert_eq!(got, b"hello world");

        let listed = store.list().expect("list");
        assert_eq!(listed, vec![id]);

        store.delete(&id).expect("delete");
        assert!(store.get(&id).expect("get").is_none());

        // Subsequent delete is idempotent.
        store.delete(&id).expect("delete idempotent");
    }

    #[test]
    fn file_storage_put_atomic_overwrites_cleanly() {
        let dir = tempdir().expect("tempdir");
        let store = FileWalletStorage::with_dir(dir.path().to_path_buf()).expect("storage");
        let id = WalletId::new();

        store.put_atomic(&id, b"first").expect("put 1");
        store.put_atomic(&id, b"second").expect("put 2");
        let got = store.get(&id).expect("get").expect("present");
        assert_eq!(got, b"second");
    }

    #[test]
    fn desktop_info_is_not_mobile() {
        let info = DesktopPlatformInfo::new("tron", "0.1.0");
        assert!(!info.is_mobile());
        assert_eq!(info.app_name(), "tron");
        assert_eq!(info.app_version(), "0.1.0");
    }

    #[test]
    fn system_clock_advances() {
        let clock = SystemClock;
        let a = clock.now_millis();
        clock.sleep(Duration::from_millis(5));
        let b = clock.now_millis();
        assert!(b >= a);
    }
}
