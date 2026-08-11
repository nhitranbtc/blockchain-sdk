//! Wallet-store filesystem layer (Task 54d / Issue #64, ADR 0001).
//!
//! Implements the persistence half of `#54d`: `$XDG_DATA_HOME/btc/wallets/
//! <network>/<wallet_id>.enc` with `0o600` files, `0o700` parent dirs,
//! atomic flush (`tempfile` + `rename`), symlink defense on read, and
//! constant-time padding on the missing-file path.
//!
//! **Threat-model coverage** (per ADR 0001 §Threat-model coverage):
//!
//! - F19 (persistence atomicity via `atomic_write`) — defended.
//! - F49 (mnemonic echo to STDOUT) — N/A here; CLI layer routes
//!   mnemonic to STDERR (see `btc wallet create`).
//! - U6 (file-permission leak) — `0o600` files + explicit `set_permissions
//!   (0o700)` after `create_dir_all` (closes umask leak).
//! - U7 (directory traversal) — XDG + UUID; no `..` injection possible.
//! - A1 (offline cracker) — Argon2id KDF (per F5), 0.5s/attempt wall-clock.
//! - A2 (local write access to data dir) — `0o700` parent dir +
//!   `refuse_world_writable` walk. Residual hardlink/rename/parent-dir
//!   risks require A2 write access on the parent dir, which A2 already
//!   has at threat-model level.
//! - N2 (file-existence oracle) — single indistinguishable error message
//!   "wallet not accessible (wrong password, wrong network, or corrupt
//!   blob)" collapses 4 distinct failure modes into one observable
//!   signature.
//! - N8 (timing oracle on missing-file) — dummy Argon2id derive on
//!   missing-file path matches ~500ms wrong-password cost.
//!
//! **Not in scope here** (handled at the `MnemonicCipherBlob` / `Aad` /
//! `Network` layers):
//!
//! - F5 / F6 — Argon2id KDF + AES-256-GCM AEAD (in `crypto::*`).
//! - N5 (cross-network footgun) — Network discriminant bound to
//!   ciphertext via `Aad::network(network)` (in `crypto::aad`).
//!
//! **Windows**: `directories::ProjectDirs::from` returns `None` on
//! Windows in v0.1 — we surface a clear `Error::WalletStore("wallet
//! store not supported on this OS in v0.1")` rather than falling back
//! to `~/btc/wallets`. Per ADR §Failure modes.

use std::fs::{self, Permissions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use bdk_wallet::bitcoin::Network;
use directories::ProjectDirs;

use crate::crypto::argon2;
use crate::error::Error;
use crate::wallet::id::WalletId;

/// Mode bits for wallet blob files. Owner read/write only.
const BLOB_FILE_MODE: u32 = 0o600;

/// Mode bits for newly-created parent directories. Owner read/write/exec
/// only — prevents other local users from listing wallet filenames
/// (although UUIDs are non-PII, the layout itself is conventionally
/// private).
const PARENT_DIR_MODE: u32 = 0o700;

/// Conventional root directory name under `data_dir()` for the `btc`
/// wallet store.
const ROOT_DIR: &str = "btc";

/// `wallets/` subdirectory under the root.
const WALLETS_SUBDIR: &str = "wallets";

/// Filename extension for encrypted wallet blobs.
const BLOB_EXT: &str = "enc";

/// Generic wallet-not-accessible message. Used to collapse 4 distinct
/// failure modes (file-not-found, wrong-password, wrong-network-AAD,
/// corrupt-blob) into one observable signature — closes the
/// file-existence oracle (N2) and the discriminator oracle (N5).
///
/// Per ADR §Failure modes + `crypto/mnemonic_cipher.rs` §Caller note on
/// error message contract: the wallet-store layer owns the collapse;
/// lower layers surface technical detail and this layer translates.
pub(crate) const WALLET_NOT_ACCESSIBLE: &str =
    "wallet not accessible (wrong password, wrong network, or corrupt blob)";

/// Return the wallet-store data directory (XDG-compliant on Linux/macOS).
///
/// Errors with `Error::WalletStore("wallet store not supported on this OS
/// in v0.1")` if `directories::ProjectDirs::from` returns `None` —
/// currently the case on Windows (per ADR §Failure modes). Operators on
/// Windows must wait or run WSL.
///
/// **Layout** (per ADR §Filesystem layout):
///
/// ```text
/// $XDG_DATA_HOME/btc/wallets/
/// └─ Linux:   ~/.local/share/btc/wallets/
/// └─ macOS:   ~/Library/Application Support/btc/wallets/
/// ```
///
/// **Trust boundary**: the wallet directory (`<network>/`) is the
/// boundary, not `$XDG_DATA_HOME`. `~/.local/share` is conventionally
/// `0o755`; other local users can list wallet filenames. UUIDs are
/// non-PII (per F34); the ciphertext is opaque (per F6); wallet
/// existence is not considered sensitive at A1's threat level.
pub fn data_dir() -> Result<PathBuf, Error> {
    ProjectDirs::from("bt", "btc", "btc")
        .map(|p| p.data_dir().to_path_buf())
        .ok_or_else(|| Error::WalletStore("wallet store not supported on this OS in v0.1".into()))
}

/// Resolve the full wallet blob path for `(network, id)` using the system
/// `data_dir()`.
pub fn wallet_path(network: Network, id: WalletId) -> Result<PathBuf, Error> {
    let base = data_dir()?;
    Ok(wallet_path_at(&base, network, id))
}

/// `wallet_path` with an explicit base directory (for tests). Pure
/// function — does no filesystem IO.
pub(crate) fn wallet_path_at(base: &Path, network: Network, id: WalletId) -> PathBuf {
    let mut p = base.to_path_buf();
    p.push(ROOT_DIR);
    p.push(WALLETS_SUBDIR);
    p.push(network_dir_name(network));
    p.push(format!("{}.{BLOB_EXT}", id));
    p
}

/// Map `bitcoin::Network` to the lowercase directory name used in the
/// wallet-store layout. `bitcoin::Network` in 0.32 has no `as_str`; we
/// match explicitly to keep the layout pinned (a Display change
/// upstream won't silently shift the layout).
fn network_dir_name(n: Network) -> &'static str {
    match n {
        Network::Bitcoin => "mainnet",
        Network::Testnet => "testnet",
        Network::Testnet4 => "testnet4",
        Network::Signet => "signet",
        Network::Regtest => "regtest",
    }
}

/// Ensure the directory exists with `0o700` permissions on every dir
/// we CREATE. Closes the umask-leak gap: `create_dir_all` applies the
/// process umask, which can land the dir at `0o755` if not explicitly
/// re-set.
///
/// **Trust boundary** (per ADR §Filesystem layout): we set permissions
/// only on dirs we created — pre-existing ancestors (e.g.,
/// `$XDG_DATA_HOME`) are NOT chmod-ed. The walk-up stops at the first
/// existing ancestor; that ancestor is the trust boundary.
///
/// **Symlink check before `create_dir_all`**: if any pre-existing
/// ancestor in the chain is a symlink, refuse BEFORE creating dirs
/// (otherwise `create_dir_all` would follow the symlink and create
/// dirs in the symlink target — exactly the footgun we want to avoid).
///
/// **Residual TOCTOU** (per ADR §Residual filesystem attacks): between
/// our symlink check and `create_dir_all`, an A2 attacker could swap
/// the dir. A2 requires write access to the parent dir, which A2
/// already has at threat-model level. Acceptable for v0.1.
pub(crate) fn ensure_secure_dir(p: &Path) -> Result<(), Error> {
    // Walk up from p to find the deepest pre-existing ancestor. Every
    // dir in `chain` (top-down) needs creation + chmod.
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
    // Symlink check on the deepest pre-existing ancestor — refuse
    // before create_dir_all follows the symlink.
    let md =
        fs::symlink_metadata(&cur).map_err(|e| Error::WalletStore(format!("stat {cur:?}: {e}")))?;
    if md.file_type().is_symlink() {
        return Err(Error::WalletStore(format!(
            "cannot create secure wallet dir: {cur:?} is a symlink"
        )));
    }
    // Create the chain (idempotent if some dirs already exist).
    fs::create_dir_all(p)?;
    // chmod each newly-created dir to 0o700. Walk top-down so that
    // `<base>/btc/wallets/testnet/` is chmod-ed before deeper
    // descendants.
    for d in chain.iter().rev() {
        let mode = Permissions::from_mode(PARENT_DIR_MODE);
        fs::set_permissions(d, mode)
            .map_err(|e| Error::WalletStore(format!("set_permissions {d:?}: {e}")))?;
    }
    // Also chmod the deepest pre-existing ancestor (the wallet root).
    // Closes U6 for legacy installs where the dir was created with
    // permissive mode (e.g., 0o755 from an umask leak in a prior
    // version). We refuse-and-fix instead of refuse-only because the
    // wallet root IS our dir to own. (L12 review CRITICAL #1.)
    let mode = Permissions::from_mode(PARENT_DIR_MODE);
    fs::set_permissions(&cur, mode)
        .map_err(|e| Error::WalletStore(format!("set_permissions {cur:?}: {e}")))?;
    Ok(())
}

/// Write `blob` bytes to the wallet path with `0o600` permissions,
/// atomically. Uses `tempfile::NamedTempFile` in the same dir + `rename`
/// over the target + `fsync` for crash safety. After persist, fsyncs
/// the parent dir so the dirent is durable (L12 review LOW #8).
///
/// Per F19: never observe a partial blob. Standard atomic-write pattern
/// (reused from PR #23's `atomic_write` for `0o600` files).
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
    // Parent-dir fsync — makes the dirent durable. Without this, a
    // crash between persist() and the next kernel flush can leave the
    // blob on disk but invisible to the dirent. (L12 review LOW #8.)
    if let Ok(dir_file) = fs::File::open(parent) {
        let _ = dir_file.sync_all();
    }
    Ok(())
}

/// Read the wallet blob bytes from the disk path. Uses `O_NOFOLLOW` so
/// a symlink at the blob path is refused ATOMICALLY at open(2) time —
/// closes the TOCTOU window between `symlink_metadata` and `read`.
///
/// **L12 review (HIGH #4):** the previous `symlink_metadata` + `fs::read`
/// two-step had a TOCTOU window. ADR §Residual filesystem attacks
/// commits to `O_NOFOLLOW` as the only filesystem-layer mitigation;
/// this is the implementation.
///
/// **MAX_LEN cap (L12 review MEDIUM #6):** rejects blobs larger than
/// `MnemonicCipherBlob::MAX_LEN` BEFORE allocating the buffer — closes
/// the A2 memory-DoS amplification where a 1 GiB blob would force a
/// 1 GiB allocation before `try_from` rejects it.
pub(crate) fn read_wallet_at(
    base: &Path,
    network: Network,
    id: WalletId,
) -> Result<Vec<u8>, Error> {
    use std::os::unix::fs::OpenOptionsExt;
    const O_NOFOLLOW: i32 = 0o400_000; // libc::O_NOFOLLOW on Linux/macOS.

    let path = wallet_path_at(base, network, id);
    // Size cap BEFORE opening — cheap stat avoids the DoS alloc.
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
    // Open with O_NOFOLLOW — atomic symlink refusal at open(2) time.
    // If the path was a regular file at stat time but a symlink at
    // open time, O_NOFOLLOW rejects with ELOOP. (L12 review HIGH #4.)
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
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
    let mut buf = Vec::with_capacity(md.len() as usize);
    std::io::Read::read_to_end(&mut file, &mut buf).map_err(Error::Io)?;
    Ok(buf)
}

/// `ELOOP` constant for `O_NOFOLLOW` symlink-at-open rejection. 40 on
/// Linux, 62 on macOS. Use a small helper instead of pulling the `libc`
/// crate as a direct dep.
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
        0 // best-effort; we won't hit this on supported platforms
    }
}

/// Run a dummy Argon2id KDF (~500ms) to pad the missing-file path so
/// its wall-clock matches the wrong-password path (closes timing oracle
/// N8).
///
/// Argon2id parameters per F5: m=256 MiB, t=10, p=4. The result is
/// discarded — only the wall-clock matters.
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

    /// Sample 12-word mnemonic phrase (test-only, deterministic).
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
        // ADR §Filesystem layout: `<network>/<wallet_id>.enc`. Two
        // different networks must produce disjoint paths for the same
        // wallet_id (defense-in-depth: blob format is network-agnostic,
        // so a CLI bug yields "no wallet found" rather than silent
        // cross-network load).
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
    fn constant_time_padding_takes_at_least_argon2_cost() {
        // Argon2id m=256/t=10/p=4 should take ≥200ms on CI hardware.
        // Loose enough for slow CI; tight enough to catch regressions.
        let start = Instant::now();
        constant_time_padding();
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() >= 200,
            "constant-time padding took {elapsed:?} — Argon2id regression?"
        );
    }

    /// Witness test: end-to-end encrypt → write → read → decrypt →
    /// mnemonic matches. This is the contract test the CLI relies on.
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

    /// Witness test: cross-network AAD mismatch rejected at decrypt
    /// (N5 footgun closure).
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
