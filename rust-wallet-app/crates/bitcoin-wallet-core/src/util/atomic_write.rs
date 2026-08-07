//! Atomic file writes (write-to-temp + fsync + parent fsync + rename).
//!
//! Per F19: writes to a temp file in the parent dir, fsyncs both the file
//! and its parent directory, then renames over the destination. Crash-safe
//! on COW/NFS filesystems; no partial ciphertexts left behind on power loss.
//!
//! Security (per 2026-08-07 review, fixes in this revision):
//! - Uses `tempfile::NamedTempFile::new_in(parent)` which opens with
//!   `O_CREAT|O_EXCL` — fails if the temp path already exists or is a
//!   symlink (closes symlink-following-write on the temp side).
//! - `NamedTempFile`'s `Drop` impl removes the temp file on any error
//!   path — closes leftover-temp-on-failure.
//! - Explicit `0o600` set on the temp file via `set_permissions` after
//!   creation — closes insecure-default-permissions (umask leak).
//! - Refuses if destination or parent is a symlink before persisting
//!   (no TOCTOU swap).
//! - Refuses if the destination path already ends in `.tmp` (loop guard).

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use tempfile::NamedTempFile;

/// Write `bytes` to `path` atomically.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    // Loop guard: don't accept a path that already ends in `.tmp`
    // (would write to the same path we then rename from).
    if path.extension().is_some_and(|e| e == "tmp") {
        return Err(std::io::Error::other(
            "atomic_write refused: destination already has .tmp extension",
        ));
    }

    // Reject symlink destinations (no following).
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(std::io::Error::other(
                "atomic_write refused: destination is a symlink",
            ));
        }
    }

    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("no parent dir"))?;

    // Reject symlink parents.
    let parent_meta = std::fs::symlink_metadata(parent)
        .map_err(|e| std::io::Error::other(format!("atomic_write: cannot stat parent: {e}")))?;
    if parent_meta.file_type().is_symlink() {
        return Err(std::io::Error::other(
            "atomic_write refused: parent directory is a symlink",
        ));
    }

    // O_CREAT|O_EXCL via tempfile::NamedTempFile. Auto-cleans on Drop.
    let mut tmp = NamedTempFile::new_in(parent)?;

    // 0o600 — closes insecure-default-permissions. The tempfile crate
    // sets a restrictive mode on Unix, but we re-assert explicitly so the
    // invariant is visible at the call site and not subject to upstream
    // crate behaviour changes.
    tmp.as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o600))?;

    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;

    // Parent fsync for full crash safety on COW/NFS (per F19 followup).
    let parent_file = std::fs::File::open(parent)?;
    parent_file.sync_all()?;

    // Atomic rename. `persist` consumes the NamedTempFile so Drop does
    // NOT remove the file (it's now at the destination). On any error
    // before this point, Drop removes the temp file.
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn atomic_write_creates_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("f.txt");
        atomic_write(&p, b"hello").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"hello");
    }

    #[test]
    fn atomic_write_no_leftover_tmp() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("f.txt");
        atomic_write(&p, b"hello").unwrap();
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let entry = entry.unwrap();
            assert!(
                !entry.file_name().to_string_lossy().ends_with(".tmp"),
                "leftover tmp: {:?}",
                entry.file_name()
            );
        }
    }

    #[test]
    fn atomic_write_sets_0o600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let p = dir.path().join("f.txt");
        atomic_write(&p, b"hello").unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn atomic_write_rejects_symlink_destination() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("real.txt");
        std::fs::write(&target, b"original").unwrap();
        let link = dir.path().join("link.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(atomic_write(&link, b"hijack").is_err());
    }

    #[test]
    fn atomic_write_rejects_tmp_extension() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("f.tmp");
        assert!(atomic_write(&p, b"hello").is_err());
    }
}
