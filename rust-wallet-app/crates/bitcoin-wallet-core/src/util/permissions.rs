//! Permission checks for sensitive directories.
//!
//! Per F19: reject data directories that are group- or world-accessible
//! before writing any secret material (mnemonic ciphertext, descriptors).

use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Refuse if `path` is group- or world-accessible. Requires mode `0o700`
/// (dirs) or `0o600` (files).
///
/// Returns `io::Result` so this module is self-contained — Task 2 will
/// add `Error::Storage` and callers can `.map_err` at the boundary.
pub fn refuse_world_writable(path: &Path) -> io::Result<()> {
    let md = std::fs::metadata(path)?;
    let mode = md.permissions().mode();
    if mode & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "path {} is group/other-accessible (mode {:o}); refusing (require 0o700/0o600)",
                path.display(),
                mode
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn refuse_world_writable_catches_755() {
        let dir = tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(refuse_world_writable(dir.path()).is_err());
    }

    #[test]
    fn refuse_world_writable_allows_0700() {
        let dir = tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(refuse_world_writable(dir.path()).is_ok());
    }
}
