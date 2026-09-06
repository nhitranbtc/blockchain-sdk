//! Phase 5 — `WalletStorage` PAL trait.
//!
//! Persistent storage for encrypted wallet blobs (one blob per
//! `WalletId`; the blob is the `EncryptedWallet` envelope produced by
//! `crypto::encrypt`). Per plan §Phase 5 Task 4.1: `Send + Sync`
//! + `put / get / list / delete / put_atomic`.
//!
//! **Why a PAL around storage:**
//!
//! The core must compile for desktop (filesystem), iOS (Keychain via
//! Swift FFI), Android (EncryptedFile via JNI), and tests (in-memory
//! map). `WalletManager` takes `&dyn WalletStorage` and never reaches
//! for a concrete backend.
//!
//! **Blob format** lives in `wallet::persist` — the storage layer is
//! agnostic to contents. Each `WalletId` resolves to one opaque
//! `Vec<u8>` blob; the storage layer neither knows nor cares that the
//! bytes are Argon2id salt + AES-GCM nonce + ciphertext + tag.
//!
//! **Atomicity:** `put_atomic` is the only path the wallet code calls.
//! It writes to a sibling temp file and renames into place so a
//! SIGKILL mid-write never corrupts the encrypted wallet. The
//! non-atomic `put` exists for backends where atomicity is
//! guaranteed by the backend itself (Keychain, EncryptedFile).

use std::fmt::Debug;

use crate::error::Result;
use crate::wallet::id::WalletId;

/// Stable, comparable handle for one stored wallet blob.
///
/// **Not** a key-derivation index — the storage layer treats it as a
/// lookup key only. Generation, derivation path, and the wallet's
/// current network all live inside the encrypted blob.
pub trait WalletStorage: Send + Sync {
    /// Atomically replace (or create) the blob for `id`.
    ///
    /// On a successful return, the new blob is durable across
    /// process restart and crash recovery. Implementations MUST
    /// guarantee that a reader either sees the previous blob or
    /// the new blob in full — never a half-written mix.
    fn put_atomic(&self, id: &WalletId, blob: &[u8]) -> Result<()>;

    /// Non-atomic replace (or create).
    ///
    /// Most callers want `put_atomic`. This exists for backends
    /// (Keychain, EncryptedFile) whose own write primitives are
    /// already atomic against crash.
    fn put(&self, id: &WalletId, blob: &[u8]) -> Result<()>;

    /// Read the blob for `id`. Returns `Ok(None)` when `id` is not
    /// present — distinct from `Err` for IO failure.
    fn get(&self, id: &WalletId) -> Result<Option<Vec<u8>>>;

    /// List every wallet id the backend currently holds.
    ///
    /// Order is unspecified. Used by `WalletManager::list` and
    /// CLI `wallet list` to populate the available-wallet inventory.
    fn list(&self) -> Result<Vec<WalletId>>;

    /// Remove the blob for `id`. Returns `Ok(())` whether or not
    /// the id existed (idempotent). Genuine backend failures —
    /// backend offline, permission error — surface as `Err`.
    fn delete(&self, id: &WalletId) -> Result<()>;
}

/// Debug-only helper for storage backend error messages.
///
/// Backends are encouraged to pass `self` through `Debug` for
/// diagnostics; the trait object itself does not require `Debug`
/// (Keychain handles, file paths, etc. all impl `Debug` themselves).
pub trait WalletStorageDebug: WalletStorage + Debug {}

impl<T> WalletStorageDebug for T where T: WalletStorage + Debug {}
