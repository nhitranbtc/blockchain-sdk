//! `sol-wallet-core` — `PlatformInfo` PAL.

use std::path::PathBuf;

/// Platform-level metadata used to locate data dirs, app identity,
/// mobile-vs-desktop branch.
pub trait PlatformInfo: Send + Sync {
    /// Directory where wallet files + SolanaConfig live.
    fn data_dir(&self) -> PathBuf;
    /// Application name (e.g. `"sol-wallet"`).
    fn app_name(&self) -> &'static str;
    /// Application version (e.g. `"0.1.0"`).
    fn app_version(&self) -> &'static str;
    /// True when running on iOS / Android (mobile PAL).
    fn is_mobile(&self) -> bool;
}

/// Test impl — caller injects data_dir + version.
#[derive(Debug, Clone)]
pub struct StaticInfo {
    data_dir: PathBuf,
    app_name: &'static str,
    app_version: &'static str,
    is_mobile: bool,
}

impl StaticInfo {
    /// Construct with explicit fields.
    pub fn new(
        data_dir: impl Into<PathBuf>,
        app_name: &'static str,
        app_version: &'static str,
        is_mobile: bool,
    ) -> Self {
        Self {
            data_dir: data_dir.into(),
            app_name,
            app_version,
            is_mobile,
        }
    }
}

impl Default for StaticInfo {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("/tmp/sol-wallet-test"),
            app_name: "sol-wallet-test",
            app_version: "0.1.0-test",
            is_mobile: false,
        }
    }
}

impl PlatformInfo for StaticInfo {
    fn data_dir(&self) -> PathBuf {
        self.data_dir.clone()
    }
    fn app_name(&self) -> &'static str {
        self.app_name
    }
    fn app_version(&self) -> &'static str {
        self.app_version
    }
    fn is_mobile(&self) -> bool {
        self.is_mobile
    }
}

/// Desktop default — `~/.local/share/<app_name>` on Linux,
/// `~/Library/Application Support/<app_name>` on macOS,
/// `%APPDATA%/<app_name>` on Windows.
#[derive(Debug, Clone)]
pub struct SystemDirsInfo {
    app_name: &'static str,
    app_version: &'static str,
}

impl SystemDirsInfo {
    /// Construct with explicit app identity.
    pub fn new(app_name: &'static str, app_version: &'static str) -> Self {
        Self {
            app_name,
            app_version,
        }
    }
}

impl Default for SystemDirsInfo {
    fn default() -> Self {
        Self {
            app_name: "sol-wallet",
            app_version: "0.1.0",
        }
    }
}

impl PlatformInfo for SystemDirsInfo {
    fn data_dir(&self) -> PathBuf {
        #[cfg(unix)]
        {
            match std::env::var("XDG_DATA_HOME") {
                Ok(xdg) => PathBuf::from(xdg).join(self.app_name),
                Err(_) => match std::env::var("HOME") {
                    Ok(home) => PathBuf::from(home)
                        .join(".local")
                        .join("share")
                        .join(self.app_name),
                    Err(_) => PathBuf::from("/tmp").join(self.app_name),
                },
            }
        }
        #[cfg(windows)]
        {
            match std::env::var("APPDATA") {
                Ok(appdata) => PathBuf::from(appdata).join(self.app_name),
                Err(_) => PathBuf::from("C:/temp").join(self.app_name),
            }
        }
    }
    fn app_name(&self) -> &'static str {
        self.app_name
    }
    fn app_version(&self) -> &'static str {
        self.app_version
    }
    fn is_mobile(&self) -> bool {
        false
    }
}
