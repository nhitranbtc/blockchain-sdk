//! Atomic file writes (write-to-temp + fsync + parent fsync + rename).
//!
//! Per F19: writes to a `.tmp` file, fsyncs both the file and its parent
//! directory, then renames over the destination. Crash-safe on COW/NFS
//! filesystems; no partial ciphertexts left behind on power loss.

use std::io;
use std::path::Path;

/// Write `bytes` to `path` atomically.
///
/// Sequence: write to `path.tmp` → `sync_all` on the tmp file →
/// `sync_all` on the parent directory → rename tmp over path.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    let f = std::fs::File::open(&tmp)?;
    f.sync_all()?;
    // Per F19 followup: fsync parent dir for full crash safety on COW/NFS.
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("no parent dir"))?;
    let parent_file = std::fs::File::open(parent)?;
    parent_file.sync_all()?;
    std::fs::rename(&tmp, path)?;
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
        assert!(!dir.path().join("f.tmp").exists());
    }
}
