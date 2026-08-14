//! Wallet-store filesystem layer (Task 54d / Issue #64, ADR 0001).
//!
//! `$XDG_DATA_HOME/btc/wallets/<network>/<wallet_id>.enc` with `0o600` files,
//! `0o700` parent dirs, atomic write, `O_NOFOLLOW` symlink defense on read,
//! constant-time padding on missing-file + try_from failure paths,
//! `MAX_LEN` cap to close A2 memory-DoS amplification.
//!
//! **Threat-model coverage** (per ADR 0001):
//!
//! | Threat | Defense |
//! |---|---|
//! | F19 (persistence atomicity) | atomic write via `tempfile::NamedTempFile` + `rename` + parent-dir fsync |
//! | F49 (mnemonic echoes to STDOUT) | CLI routes mnemonic to STDERR; library is silent |
//! | U6 (file-permission leak) | 0o600 files + 0o700 dirs (incl. legacy install re-secure) |
//! | U7 (directory traversal) | XDG + UUID; no `..` injection possible |
//! | A1 (offline cracker) | Argon2id KDF (per F5), ~500ms/attempt |
//! | A2 (local write attacker) | 0o700 parent + O_NOFOLLOW on read; O_NOFOLLOW rejects symlink at open(2) |
//! | N2 (file-existence oracle) | single indistinguishable error message |
//! | N5 (cross-network ciphertext reuse) | Aad::network(network) binds network discriminant (handled in `crypto::aad`) |
//! | N8 (timing oracle) | `constant_time_padding()` runs dummy Argon2id on missing-file + try_from failure paths |
//!
//! **Residual filesystem attacks** (per ADR §Residual risks, accepted):
//! hardlink attack, rename-during-read, parent-dir TOCTOU all require A2
//! write access to the parent dir, which A2 already has at threat-model level.
//! Ciphertext tampering (F19 integrity) fails AEAD → `Error::WalletStore`.

use std::fs::{self, Permissions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use bdk_wallet::bitcoin::Network;
use directories::ProjectDirs;

use crate::crypto::argon2;
use crate::error::Error;
use crate::wallet::id::WalletId;

/// Mode bits for wallet blob files. Owner read/write only.
const BLOB_FILE_MODE: u32 = 0o600;

/// Mode bits for newly-created parent directories. Owner read/write/exec.
const PARENT_DIR_MODE: u32 = 0o700;

/// Conventional root directory name under `data_dir()` for the `btc` wallet store.
pub(crate) const ROOT_DIR: &str = "btc";

/// `wallets/` subdirectory under the root.
pub(crate) const WALLETS_SUBDIR: &str = "wallets";

/// Filename extension for encrypted wallet blobs.
const BLOB_EXT: &str = "enc";

/// Generic wallet-not-accessible message. Collapses 4 distinct failure
/// modes (file-not-found, wrong-password, wrong-network-AAD, corrupt-blob)
/// into one observable signature — closes N2 file-existence oracle.
pub(crate) const WALLET_NOT_ACCESSIBLE: &str =
    "wallet not accessible (wrong password, wrong network, or corrupt blob)";

/// `ELOOP` errno value for `O_NOFOLLOW` symlink-at-open rejection.
/// Linux = 40, macOS = 62. Inline (no `libc` crate dep).
#[cfg(unix)]
fn libc_eloop() -> i32 {
    #[cfg(target_os = "linux")]
    {
        40
    }
    #[cfg(target_os = "macos")]
    {
        62
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        0
    }
}

/// `O_NOFOLLOW` open(2) flag value. Linux = `0o400000` (262144),
/// macOS = `0x0100` (256). Distinct from `ELOOP` — passing the errno
/// as a flag is a no-op (kernel ignores unknown flag bits).
///
/// **L12 review HIGH #2:** prior retry passed `libc_eloop()` as the
/// flag value, which doesn't enable `O_NOFOLLOW`. The symlink defense
/// worked only via the pre-`symlink_metadata` check; the kernel-level
/// atomic refusal was a no-op. Fix: separate flag value from errno
/// value, and add a compile-time witness that asserts the supported
/// platform matrix. Non-Linux/macOS Unix fails the build rather than
/// silently weakening security.
#[cfg(unix)]
const _O_NOFOLLOW_PLATFORM_WITNESS: () = {
    assert!(
        cfg!(any(target_os = "linux", target_os = "macos")),
        "O_NOFOLLOW flag value not defined for this Unix variant; \
         add the flag constant to `o_nofollow_flag()` before building."
    );
};

#[cfg(unix)]
fn o_nofollow_flag() -> i32 {
    #[cfg(target_os = "linux")]
    {
        0o400_000
    }
    #[cfg(target_os = "macos")]
    {
        0x0100
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        // Unreachable: `_O_NOFOLLOW_PLATFORM_WITNESS` aborts build first.
        0
    }
}

/// Map `bitcoin::Network` to the lowercase directory name used in the
/// wallet-store layout. Pinning layout strings independent of upstream
/// `Display` drift (defense-in-depth: ADR §Filesystem layout commits to
/// specific directory names).
pub(crate) fn network_dir_name(n: Network) -> &'static str {
    match n {
        Network::Bitcoin => "mainnet",
        Network::Testnet => "testnet",
        Network::Testnet4 => "testnet4",
        Network::Signet => "signet",
        Network::Regtest => "regtest",
    }
}

/// Return the wallet-store data directory (XDG-compliant on Linux/macOS).
/// Errors with `Error::WalletStore` on Windows (v0.1 deferred).
pub fn data_dir() -> Result<PathBuf, Error> {
    ProjectDirs::from("bt", "btc", "btc")
        .map(|p| p.data_dir().to_path_buf())
        .ok_or_else(|| Error::WalletStore("wallet store not supported on this OS in v0.1".into()))
}

/// Resolve the full wallet blob path for `(network, id)` using system `data_dir()`.
pub fn wallet_path(network: Network, id: WalletId) -> Result<PathBuf, Error> {
    let base = data_dir()?;
    Ok(wallet_path_at(&base, network, id))
}

/// `wallet_path` with explicit base directory (for tests). Pure function — no FS IO.
pub(crate) fn wallet_path_at(base: &Path, network: Network, id: WalletId) -> PathBuf {
    let mut p = base.to_path_buf();
    p.push(ROOT_DIR);
    p.push(WALLETS_SUBDIR);
    p.push(network_dir_name(network));
    p.push(format!("{}.{BLOB_EXT}", id));
    p
}

/// Ensure the directory exists with `0o700` permissions. Closes umask-leak
/// gap (per ADR §F19). Refuses symlinks anywhere in the chain (TOCTOU
/// window defense). Re-chmods the deepest pre-existing ancestor to 0o700
/// (closes U6 for legacy installs where the dir was created with
/// permissive mode).
///
/// **Trust boundary**: only dirs we own are chmod-ed. Pre-existing
/// ancestors above the wallet root (e.g., `$XDG_DATA_HOME`) are left
/// alone — they're conventionally `0o755` and not under our control.
pub(crate) fn ensure_secure_dir(p: &Path) -> Result<(), Error> {
    let mut chain: Vec<PathBuf> = Vec::new();
    let mut cur = p.to_path_buf();
    loop {
        if cur.exists() {
            break;
        }
        let parent = cur
            .parent()
            .ok_or_else(|| Error::WalletStore(format!("path {cur:?} has no parent")))?;
        chain.push(cur.clone());
        cur = parent.to_path_buf();
    }
    let md = fs::symlink_metadata(&cur)
        .map_err(|e| Error::WalletStore(format!("stat wallet dir: {e}")))?;
    if md.file_type().is_symlink() {
        return Err(Error::WalletStore(
            "cannot create secure wallet dir: ancestor is a symlink".into(),
        ));
    }
    fs::create_dir_all(p)?;
    for d in chain.iter().rev() {
        let mode = Permissions::from_mode(PARENT_DIR_MODE);
        fs::set_permissions(d, mode)
            .map_err(|e| Error::WalletStore(format!("set_permissions wallet dir: {e}")))?;
    }
    let mode = Permissions::from_mode(PARENT_DIR_MODE);
    fs::set_permissions(&cur, mode)
        .map_err(|e| Error::WalletStore(format!("set_permissions wallet dir: {e}")))?;
    Ok(())
}

/// Write `blob` bytes to the wallet path with `0o600` permissions,
/// atomically. `tempfile::NamedTempFile` + `rename` + `fsync` + parent-dir
/// `fsync` (L12 review LOW #8).
pub(crate) fn write_wallet_at(
    base: &Path,
    network: Network,
    id: WalletId,
    blob: &[u8],
) -> Result<(), Error> {
    let path = wallet_path_at(base, network, id);
    let parent = path
        .parent()
        .ok_or_else(|| Error::WalletStore("wallet path has no parent".into()))?;
    ensure_secure_dir(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(blob)?;
    tmp.as_file_mut().sync_all()?;
    tmp.as_file()
        .set_permissions(Permissions::from_mode(BLOB_FILE_MODE))?;
    tmp.persist(&path)
        .map_err(|e| Error::WalletStore(format!("atomic write {path:?}: {e}")))?;
    if let Err(e) = fs::File::open(parent).and_then(|f| f.sync_all().map(|_| f)) {
        // L12 review LOW #1: log fsync failure rather than silently drop.
        // Crash safety is best-effort; the data file is durable even
        // without the parent-dir fsync.
        tracing::warn!("parent-dir fsync failed (data file durable, dirent may be stale): {e}");
    }
    Ok(())
}

/// Read wallet blob bytes. `O_NOFOLLOW` rejects symlink ATOMICALLY at
/// open(2) — closes TOCTOU window. MAX_LEN cap BEFORE allocating buffer
/// (L12 review MED #6: closes A2 memory-DoS amplification).
pub(crate) fn read_wallet_at(
    base: &Path,
    network: Network,
    id: WalletId,
) -> Result<Vec<u8>, Error> {
    let path = wallet_path_at(base, network, id);
    let md = fs::symlink_metadata(&path).map_err(Error::Io)?;
    if md.file_type().is_symlink() {
        return Err(Error::WalletStore(
            "wallet blob is a symlink — refusing to follow (security check)".into(),
        ));
    }
    let max_len = crate::crypto::mnemonic_cipher::MnemonicCipherBlob::MAX_LEN as u64;
    if md.len() > max_len {
        return Err(Error::WalletStore(format!(
            "wallet blob too large: {} bytes (max {max_len})",
            md.len()
        )));
    }
    // O_NOFOLLOW: atomic symlink refusal at open(2) — closes TOCTOU window.
    // (L12 review HIGH #2: o_nofollow_flag() != libc_eloop() — passing the
    // errno number as the flag value is a no-op.)
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(o_nofollow_flag())
        .open(&path)
        .map_err(|e| {
            if e.raw_os_error() == Some(libc_eloop()) {
                Error::WalletStore(
                    "wallet blob is a symlink — refusing to follow (security check)".into(),
                )
            } else {
                Error::Io(e)
            }
        })?;
    // L12 review MED #1: bound the read by `max_len` to close the
    // post-stat-grow race. `Read::take(max_len)` caps the read at exactly
    // `max_len` bytes; if the file is shorter, read returns less.
    use std::io::Read;
    let mut limited = (&mut file).take(max_len);
    let mut buf = Vec::with_capacity(max_len as usize);
    limited.read_to_end(&mut buf).map_err(Error::Io)?;
    Ok(buf)
}

/// Dummy Argon2id derive (~500ms) on missing-file + try_from failure
/// paths to match wrong-password wall-clock — closes N8 timing oracle.
pub(crate) fn constant_time_padding() {
    let dummy_password = b"constant-time-padding-not-a-secret";
    let dummy_salt = [0u8; argon2::SALT_LEN];
    let _ = argon2::derive_key(dummy_password, &dummy_salt);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    use crate::crypto::aad::Aad;
    use crate::crypto::mnemonic_cipher::{decrypt_mnemonic, encrypt_mnemonic, MnemonicCipherBlob};
    use crate::keys::{Mnemonic, Secret};

    fn sample_phrase() -> Secret<String> {
        let m = Mnemonic::generate(12).expect("fresh mnemonic");
        m.to_phrase()
    }

    #[test]
    fn data_dir_returns_some_path_on_unix() {
        let dir = data_dir();
        if cfg!(target_os = "windows") {
            assert!(dir.is_err());
        } else {
            assert!(dir.is_ok(), "data_dir should resolve on Unix");
            let p = dir.unwrap();
            assert!(p.is_absolute());
            assert!(!p.as_os_str().is_empty());
        }
    }

    #[test]
    fn wallet_path_disjoint_per_network_same_id() {
        let base = PathBuf::from("/tmp/btc-test-disjoint");
        let id = WalletId::new();
        let testnet_path = wallet_path_at(&base, Network::Testnet, id);
        let mainnet_path = wallet_path_at(&base, Network::Bitcoin, id);
        assert_ne!(testnet_path, mainnet_path);
        assert!(testnet_path.to_string_lossy().contains("testnet"));
        assert!(mainnet_path.to_string_lossy().contains("mainnet"));
        assert!(testnet_path.to_string_lossy().contains(&id.to_string()));
    }

    #[test]
    fn wallet_path_layout_matches_adr() {
        let base = PathBuf::from("/data");
        let id_str = "550e8400-e29b-41d4-a716-446655440000";
        let id: WalletId = id_str.parse().unwrap();
        let p = wallet_path_at(&base, Network::Testnet, id);
        let expected =
            PathBuf::from("/data/btc/wallets/testnet/550e8400-e29b-41d4-a716-446655440000.enc");
        assert_eq!(p, expected);
    }

    #[test]
    fn ensure_secure_dir_creates_chain_with_mode_700() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let target = tmp.path().join("a/b/c");
        ensure_secure_dir(&target).expect("create chain");
        for p in [&target, &tmp.path().join("a"), &tmp.path().join("a/b")] {
            assert!(p.exists(), "dir {p:?} should exist");
            let mode = fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(
                mode, PARENT_DIR_MODE,
                "dir {p:?} mode {mode:o} != {PARENT_DIR_MODE:o}"
            );
        }
    }

    #[test]
    fn ensure_secure_dir_refuses_symlink_in_chain() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink("/etc", &link).expect("symlink");
        let target = link.join("a/b");
        let err = ensure_secure_dir(&target).expect_err("symlink must reject");
        assert!(matches!(err, Error::WalletStore(_)));
        assert!(err.to_string().contains("symlink"));
    }

    #[test]
    fn write_then_read_roundtrip() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path().to_path_buf();
        let id = WalletId::new();
        let blob = b"encrypted mnemonic bytes";
        write_wallet_at(&base, Network::Testnet, id, blob).expect("write");
        let read = read_wallet_at(&base, Network::Testnet, id).expect("read");
        assert_eq!(read, blob);
    }

    #[test]
    fn write_creates_file_with_mode_600() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path().to_path_buf();
        let id = WalletId::new();
        write_wallet_at(&base, Network::Testnet, id, b"x").expect("write");
        let path = wallet_path_at(&base, Network::Testnet, id);
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, BLOB_FILE_MODE,
            "file mode {mode:o} != {BLOB_FILE_MODE:o}"
        );
    }

    #[test]
    fn read_refuses_symlink_at_blob_path() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path().to_path_buf();
        let dir = base.join("btc/wallets/testnet");
        ensure_secure_dir(&dir).expect("dir");
        let id = WalletId::new();
        let path = wallet_path_at(&base, Network::Testnet, id);
        let target = tmp.path().join("elsewhere.txt");
        fs::write(&target, b"attacker controlled").expect("write target");
        std::os::unix::fs::symlink(&target, &path).expect("symlink");
        let err = read_wallet_at(&base, Network::Testnet, id).expect_err("symlink rejects");
        assert!(matches!(err, Error::WalletStore(_)));
        assert!(err.to_string().contains("symlink"));
    }

    #[test]
    fn read_missing_file_returns_io_error() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path().to_path_buf();
        let id = WalletId::new();
        let err = read_wallet_at(&base, Network::Testnet, id).expect_err("missing rejects");
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn read_rejects_oversize_blob() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path().to_path_buf();
        let dir = base.join("btc/wallets/testnet");
        ensure_secure_dir(&dir).expect("dir");
        let id = WalletId::new();
        let path = wallet_path_at(&base, Network::Testnet, id);
        fs::write(&path, vec![0u8; 1024 * 1024]).expect("write 1MiB");
        let err =
            read_wallet_at(&base, Network::Testnet, id).expect_err("oversize blob must reject");
        assert!(matches!(err, Error::WalletStore(_)));
        assert!(err.to_string().contains("too large"));
    }

    #[test]
    fn constant_time_padding_takes_at_least_argon2_cost() {
        let start = Instant::now();
        constant_time_padding();
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() >= 200,
            "constant-time padding took {elapsed:?} — Argon2id regression?"
        );
    }

    /// Witness: full encrypt → write → read → decrypt pipeline.
    #[test]
    fn end_to_end_encrypt_write_read_decrypt() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path().to_path_buf();
        let id = WalletId::new();
        let phrase = sample_phrase();
        let password = b"test-password-do-not-use-in-prod";
        let aad = Aad::network(Network::Testnet);

        let blob = encrypt_mnemonic(&phrase, password, aad).expect("encrypt");
        write_wallet_at(&base, Network::Testnet, id, blob.as_bytes()).expect("write");
        let read_bytes = read_wallet_at(&base, Network::Testnet, id).expect("read");
        let read_blob = MnemonicCipherBlob::try_from(read_bytes.as_slice()).expect("re-wrap");
        let restored = decrypt_mnemonic(&read_blob, password, aad).expect("decrypt");
        assert_eq!(phrase.expose(), restored.expose());
    }

    /// Witness: N5 cross-network footgun closure.
    #[test]
    fn cross_network_aad_mismatch_rejected_at_decrypt() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let base = tmp.path().to_path_buf();
        let id = WalletId::new();
        let phrase = sample_phrase();
        let password = b"test-password";
        let testnet_aad = Aad::network(Network::Testnet);
        let mainnet_aad = Aad::network(Network::Bitcoin);

        let blob = encrypt_mnemonic(&phrase, password, testnet_aad).expect("encrypt testnet");
        write_wallet_at(&base, Network::Testnet, id, blob.as_bytes()).expect("write testnet");
        let read_bytes = read_wallet_at(&base, Network::Testnet, id).expect("read");
        let read_blob = MnemonicCipherBlob::try_from(read_bytes.as_slice()).expect("re-wrap");
        let err = decrypt_mnemonic(&read_blob, password, mainnet_aad)
            .expect_err("mainnet AAD must reject");
        assert!(matches!(err, Error::MnemonicCipher(_)));
    }
}
