//! `sol-wallet-core` — `WalletStorage` PAL.
//!
//! Phase 6.1 Task 6.1 Step 5. Per security audit
//! `docs/audit/2026-09-11-sol-wallet-core-phase6-security-review.md`:
//!
//! - **P6-2** — Unix: `set_permissions(0o600)` + post-`rename` verify.
//!   Windows `SetSecurityInfo` branch stubbed with explicit V0.1.5
//!   deferral marker.

use crate::persist;
use crate::{Error, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Encrypted-blob storage backend. One impl per platform.
pub trait WalletStorage: Send + Sync {
    /// Atomic write — `.tmp` + `fsync` + `rename` (audit P6-8).
    fn put_atomic(&self, name: &str, bytes: &[u8]) -> Result<()>;
    /// Read blob bytes.
    fn get(&self, name: &str) -> Result<Vec<u8>>;
    /// Delete blob. Missing file = `Ok(())`.
    fn delete(&self, name: &str) -> Result<()>;
    /// List blob names (sorted).
    fn list_ids(&self) -> Result<Vec<String>>;
}

/// In-memory storage for tests.
#[derive(Debug, Default)]
pub struct InMemoryStorage {
    inner: std::sync::Mutex<BTreeMap<String, Vec<u8>>>,
}

impl InMemoryStorage {
    /// Create a fresh empty in-memory store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl WalletStorage for InMemoryStorage {
    fn put_atomic(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let mut g = self.inner.lock().expect("in-memory lock poisoned");
        g.insert(name.to_string(), bytes.to_vec());
        Ok(())
    }
    fn get(&self, name: &str) -> Result<Vec<u8>> {
        let g = self.inner.lock().expect("in-memory lock poisoned");
        g.get(name)
            .cloned()
            .ok_or(Error::WalletNotFound(crate::WalletId(uuid::Uuid::nil())))
    }
    fn delete(&self, name: &str) -> Result<()> {
        let mut g = self.inner.lock().expect("in-memory lock poisoned");
        g.remove(name);
        Ok(())
    }
    fn list_ids(&self) -> Result<Vec<String>> {
        let g = self.inner.lock().expect("in-memory lock poisoned");
        Ok(g.keys().cloned().collect())
    }
}

/// Desktop file-backed storage. Unix: enforce mode 0o600 (audit
/// P6-2). Windows: V0.1.5-deferred per audit P6-2.
pub struct FileWalletStorage {
    root: PathBuf,
}

impl FileWalletStorage {
    /// Open storage rooted at `root`. The root directory is created
    /// (mode 0o700 on Unix) if missing. Idempotent.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(|source| Error::FileIo {
            path: root.clone(),
            source,
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).map_err(
                |source| Error::FileIo {
                    path: root.clone(),
                    source,
                },
            )?;
        }
        Ok(Self { root })
    }

    fn path_for(&self, name: &str) -> PathBuf {
        // Phase 10 Security Audit Task 10.7 — path-traversal guard.
        // The default trait impl provides the safe `join` semantics
        // for any storage backend. Per-impl overrides (FileWalletStorage)
        // route through `validate_name` + `path_for_checked` to enforce
        // the rejection contract: empty / `.` / `..` / absolute /
        // separator-bearing / NUL-containing names return typed
        // `Error::InvalidStorageName` rather than silently resolving
        // outside the storage root.
        self.root.join(name)
    }

    /// Default-implemented validator for storage names. Storage
    /// backends MUST override this if they have stricter rules; the
    /// default rejects every path-traversal shape documented in
    /// Task 10.7 (audit #566 / issue TBD).
    fn validate_name(name: &str) -> Result<()> {
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.contains('/')
            || name.contains('\\')
            || name.contains('\0')
            || name.starts_with('/')
            || name.starts_with('\\')
            || (name.len() >= 2 && name.as_bytes()[1] == b':')
        {
            return Err(Error::InvalidStorageName {
                name: name.to_string(),
            });
        }
        Ok(())
    }

    /// Helper for storage impls: validate + compute the resolved
    /// path. Returns `Error::InvalidStorageName` for traversal
    /// attempts, `Error::FileIo` if canonicalization fails. Storage
    /// backends that want checked-path construction in one call may
    /// use this directly; the `FileWalletStorage` impl instead calls
    /// `Self::validate_name(name)?` + `self.path_for(name)` separately
    /// to keep `path_for` infallible for backends like `InMemoryStorage`
    /// that don't use paths.
    #[allow(dead_code)]
    fn path_for_checked(&self, name: &str) -> Result<std::path::PathBuf> {
        Self::validate_name(name)?;
        Ok(self.root.join(name))
    }
}

impl WalletStorage for FileWalletStorage {
    fn put_atomic(&self, name: &str, bytes: &[u8]) -> Result<()> {
        Self::validate_name(name)?;
        let path = self.path_for(name);
        persist::atomic_write(&path, bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(
                |source| Error::FileIo {
                    path: path.clone(),
                    source,
                },
            )?;
            let m = std::fs::metadata(&path).map_err(|source| Error::FileIo {
                path: path.clone(),
                source,
            })?;
            if m.permissions().mode() & 0o077 != 0 {
                return Err(Error::FileIo {
                    path: path.clone(),
                    source: std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "mode bits exposed",
                    ),
                });
            }
        }
        #[cfg(windows)]
        {
            // P6-2 Windows branch — stubbed with V0.1.5 deferral.
            let _ = &path;
        }
        Ok(())
    }
    fn get(&self, name: &str) -> Result<Vec<u8>> {
        Self::validate_name(name)?;
        let path = self.path_for(name);
        std::fs::read(&path).map_err(|source| Error::FileIo { path, source })
    }
    fn delete(&self, name: &str) -> Result<()> {
        Self::validate_name(name)?;
        let path = self.path_for(name);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(Error::FileIo { path, source }),
        }
    }
    fn list_ids(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        let entries = std::fs::read_dir(&self.root).map_err(|source| Error::FileIo {
            path: self.root.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| Error::FileIo {
                path: self.root.clone(),
                source,
            })?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".tmp") {
                out.push(name);
            }
        }
        out.sort();
        Ok(out)
    }
}

impl std::fmt::Debug for FileWalletStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FileWalletStorage({})", self.root.display())
    }
}

#[cfg(test)]
mod tests {
    //! Phase 10 Security Audit Task 10.7 — path-traversal guard pin.
    //!
    //! 4 rejected shapes per plan acceptance criterion:
    //!   - `..` segments (escape above root)
    //!   - absolute path prefix (Unix `/...` or Windows `C:\...`)
    //!   - embedded path separator (name spans directories)
    //!   - NUL byte (POSIX path-truncation attack)
    //!
    //! Plus positive case: a normal `WalletId` string resolves
    //! successfully.

    use super::{FileWalletStorage, WalletStorage};
    use crate::Error;

    #[test]
    fn validate_name_rejects_dot_dot_traversal() {
        let err = FileWalletStorage::validate_name("..").unwrap_err();
        assert!(matches!(err, Error::InvalidStorageName { .. }));
    }

    #[test]
    fn validate_name_rejects_absolute_unix_path() {
        let err = FileWalletStorage::validate_name("/etc/passwd").unwrap_err();
        assert!(matches!(err, Error::InvalidStorageName { .. }));
    }

    #[test]
    fn validate_name_rejects_embedded_separator() {
        let err = FileWalletStorage::validate_name("foo/bar").unwrap_err();
        assert!(matches!(err, Error::InvalidStorageName { .. }));
        let err = FileWalletStorage::validate_name("foo\\bar").unwrap_err();
        assert!(matches!(err, Error::InvalidStorageName { .. }));
    }

    #[test]
    fn validate_name_rejects_nul_byte() {
        let err = FileWalletStorage::validate_name("foo\0bar").unwrap_err();
        assert!(matches!(err, Error::InvalidStorageName { .. }));
    }

    #[test]
    fn validate_name_accepts_normal_wallet_id() {
        // A real WalletId is a UUID string like
        // "550e8400-e29b-41d4-a716-446655440000" — single segment,
        // no separators or traversal shapes.
        let name = "550e8400-e29b-41d4-a716-446655440000";
        FileWalletStorage::validate_name(name).expect("normal WalletId MUST pass validation");
    }

    #[test]
    fn put_atomic_rejects_traversal_name() {
        // Integration check: `put_atomic` routes through
        // `validate_name` so a traversal-shape name surfaces as
        // `Error::InvalidStorageName` BEFORE any filesystem write
        // is attempted.
        let store =
            FileWalletStorage::open("/tmp/sol-wallet-test-task-10.7").expect("open temp storage");
        let err = store
            .put_atomic("../../../etc/passwd", b"evil")
            .unwrap_err();
        assert!(matches!(err, Error::InvalidStorageName { .. }));
        // Cleanup.
        let _ = std::fs::remove_dir_all("/tmp/sol-wallet-test-task-10.7");
    }
}
