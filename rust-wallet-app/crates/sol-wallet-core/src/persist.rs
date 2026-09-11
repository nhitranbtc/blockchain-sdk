//! `sol-wallet-core` — atomic file write.
//!
//! Phase 6.1 Task 6.1 Step 3. Per security audit
//! `docs/audit/2026-09-11-sol-wallet-core-phase6-security-review.md`
//! finding P6-8: on rename failure the `.tmp` file MUST be removed
//! so it does not accumulate on disk and shadow subsequent reads.
//! Per L13 step 10 Sept 11 finding: `.tmp` mode 0o600 must be set
//! BEFORE rename so the rename-preserves-mode TOCTOU window is closed.
//! Per L13 step 10 Sept 11 finding: any early return (write failure,
//! fsync failure) must also clean up `.tmp` — Drop guard handles this.

use crate::{Error, Result};
use std::io::Write;
use std::path::{Path, PathBuf};

/// RA- guard for the `.tmp` file. Holds the path; on Drop,
/// `remove_file`s it unless `disarm()` was called. Ensures the
/// `.tmp` is cleaned up on EVERY early return (write fail, fsync
/// fail, fsync-on-parent fail, rename fail, panic-unwind).
struct TmpGuard {
    path: PathBuf,
    armed: bool,
}

impl TmpGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }
    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for TmpGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Write `bytes` to `path` atomically: writes to `path.tmp`,
/// `fsync`s, sets mode 0o600 on `.tmp` (Unix), `fsync`s the
/// parent dir, then renames. On ANY early return, the `.tmp` is
/// removed via `TmpGuard::Drop`.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = tmp_path(path);
    let guard = TmpGuard::new(tmp.clone());

    // 1. Open + write `.tmp` with mode 0o600 ATOMICALLY (Unix).
    // Per L13 post-push security review: closes the umask-0o644
    // window between `File::create` and a later `chmod`. The
    // `mode()` on `OpenOptions` runs through `O_CREAT` with the
    // given mode bits — no subsequent chmod needed.
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp).map_err(|source| Error::FileIo {
        path: tmp.clone(),
        source,
    })?;
    f.write_all(bytes).map_err(|source| Error::FileIo {
        path: tmp.clone(),
        source,
    })?;

    // 2. `fsync` data.
    f.sync_all().map_err(|source| Error::FileIo {
        path: tmp.clone(),
        source,
    })?;
    drop(f);

    // 3. `fsync` parent dir so the rename is durable.
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let dir = std::fs::File::open(parent).map_err(|source| Error::FileIo {
                path: parent.to_path_buf(),
                source,
            })?;
            dir.sync_all().map_err(|source| Error::FileIo {
                path: parent.to_path_buf(),
                source,
            })?;
        }
    }

    // 4. Rename. On success, disarm the guard so Drop doesn't
    // remove the renamed file.
    std::fs::rename(&tmp, path).map_err(|source| Error::FileIo {
        path: path.to_path_buf(),
        source,
    })?;
    guard.disarm();
    Ok(())
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_owned();
    p.push(".tmp");
    PathBuf::from(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmp_path_appends_dot_tmp() {
        let p = tmp_path(Path::new("/tmp/wallet.bin"));
        assert_eq!(p, PathBuf::from("/tmp/wallet.bin.tmp"));
    }
}
