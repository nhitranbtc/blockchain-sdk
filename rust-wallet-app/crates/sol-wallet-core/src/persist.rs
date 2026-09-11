//! `sol-wallet-core` — atomic file write.
//!
//! Phase 6.1 Task 6.1 Step 3. Per security audit
//! `docs/audit/2026-09-11-sol-wallet-core-phase6-security-review.md`
//! finding P6-8: on rename failure the `.tmp` file MUST be removed
//! so it does not accumulate on disk and shadow subsequent reads.
//!
//! Per plan §Phase 6 deep-dive row 9 acceptance: `.tmp` write →
//! `fsync` → `rename`. On Unix the destination mode is enforced by
//! the caller (`FileWalletStorage::put_atomic`); this module is
//! the mode-agnostic primitive.

use crate::{Error, Result};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Write `bytes` to `path` atomically: writes to `path.tmp`,
/// `fsync`s, then renames. On `rename` failure, removes the
/// orphaned `path.tmp` before returning the error (audit P6-8).
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = tmp_path(path);

    // 1. Open + write `.tmp`.
    let mut f = std::fs::File::create(&tmp).map_err(|source| Error::FileIo {
        path: tmp.clone(),
        source,
    })?;
    f.write_all(bytes).map_err(|source| Error::FileIo {
        path: tmp.clone(),
        source,
    })?;

    // 2. `fsync` data + parent dir so the rename is durable.
    f.sync_all().map_err(|source| Error::FileIo {
        path: tmp.clone(),
        source,
    })?;
    drop(f);
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

    // 3. Rename. On failure, clean up the `.tmp` (audit P6-8).
    if let Err(source) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(Error::FileIo {
            path: path.to_path_buf(),
            source,
        });
    }
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
