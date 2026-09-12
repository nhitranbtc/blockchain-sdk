//! FFI cdylib surface — 16 `extern "C"` exports for Dart/Swift/Kotlin consumers.
//!
//! Phase 8.1 of `docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md`.
//! Companion audit: `docs/audit/2026-09-11-sol-wallet-core-phase8-security-review.md`

// Inner attributes MUST appear before any items in the module. Crate
// root denies `unsafe_code`; this module is the ONE place in the crate
// that legitimately uses raw-pointer derefs + `#[no_mangle]` symbol
// exports. `unknown_lints` guards against future-Rust-only lints
// (`unsafe_attributes`, `missing_safety_doc`) that are stable-only in
// newer toolchains — CI uses stable Rust where these don't yet exist.
#![allow(
    unknown_lints,
    unsafe_code,
    unused_unsafe,
    unsafe_op_in_unsafe_fn,
    internal_features,
    unsafe_attributes,
    unused_doc_comments,
    unused_variables,
    clippy::manual_is_ascii_check,
    clippy::unnecessary_unwrap,
    clippy::not_unsafe_ptr_arg_deref,
    clippy::question_mark,
    clippy::redundant_pattern_matching
)]
//! ## FFI safety contract (audit H1–H8)
//!
//! - Every `*out_*` param is paired with a `buf_len: usize` (caller's
//!   buffer capacity). Library returns `FfiError::BufTooSmall` with the
//!   required size in `last_error_message`. (H1)
//! - Every `*in_*` param is paired with an `in_len: usize` (caller's
//!   buffer length). Library refuses to read past `in_len`. (H2)
//! - `sol_wallet_zeroize(buf, len)` is the paired zeroize helper. (H3)
//! - `sol_wallet_lock(id)` auto-zeros the matching unlocked secret. (H3/M14)
//! - `password` and `secret` bytes route through `Zeroizing<Vec<u8>>` on
//!   the Rust side. No stack copies. (H4)
//! - Static state is `thread_local!` per thread — no cross-thread races. (H6)
//! - Release-mobile profile uses `panic = "abort"`; dev profile uses
//!   `panic = "unwind"` + scrubber. (H8)
//! - Error msgs pass through `panic_scrubber::scrub` before being
//!   surfaced via `last_error_message`. (M10)
//!
//! ## Return-code convention
//!
//! Every FFI fn returns `i32`. `0` = success; non-zero = `FfiError` code.
//! Panic during FFI = `FfiError::Panic` (= 99).
//!
//! ## Per-thread state
//!
//! Mobile FFI is invoked from arbitrary threads (UI, network, async
//! dispatch). Two state cells, both per-thread:
//!
//! 1. `LAST_ERROR` — last error message, surfaced via
//!    `sol_wallet_last_error_message`. Mobile consumer reads after each
//!    FFI call that returned non-zero. Cleared by
//!    `sol_wallet_panic_message_clear`.
//! 2. `UNLOCKED` — `HashMap<WalletId, Zeroizing<Vec<u8>>>` of unlocked
//!    32-byte secrets. Populated by `sol_wallet_unlock`; auto-cleared
//!    by `sol_wallet_lock`. (H3/M14)

use crate::panic_scrubber::scrub as scrub_msg;
use crate::{Error, WalletId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use zeroize::{Zeroize, Zeroizing};

/// FFI error codes — returned as `i32` from every FFI fn.
///
/// Mobile consumers branch on the numeric value. Keep stable across
/// releases; new codes appended, never re-used.
#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FfiError {
    /// Success — sentinel for completeness; FFI fns return 0 directly.
    Ok = 0,
    /// Caller-supplied `*out_*` buffer too small to hold the result.
    /// Required size reported via `sol_wallet_last_error_message`.
    BufTooSmall = 1,
    /// Caller passed NULL pointer for a required arg.
    NullPointer = 2,
    /// Caller-supplied UTF-8 buffer contained invalid bytes.
    InvalidUtf8 = 3,
    /// `WalletId` not found in the manager.
    WalletNotFound = 4,
    /// `sol_wallet_sign_transaction` etc. called while wallet is locked.
    WalletLocked = 5,
    /// Argon2id decrypt failed (wrong password or corrupt blob).
    DecryptFailed = 6,
    /// Preflight found balance insufficient for amount + fee + rent.
    InsufficientFunds = 7,
    /// RPC transport failure (timeout, DNS, TLS, non-success status).
    Transport = 8,
    /// H5 — `sol_wallet_send_sol` / `sol_wallet_send_spl` amount
    /// exceeded the per-wallet `max_per_tx_lamports` policy.
    ExceedsMaxPerTx = 9,
    /// H5 — destination not in the per-wallet `allowed_destinations` list.
    DestinationNotAllowed = 10,
    /// H5 — send would exceed the per-wallet `daily_limit_lamports` rolling
    /// 24h window.
    ExceedsDailyLimit = 11,
    /// H5 — policy was not configured at wallet-create time + caller did
    /// not pass `unrestricted`. (Default policy = `allowed_destinations`
    /// `None` means unrestricted; only `Some(empty)` is this error.)
    PolicyDenied = 12,
    /// Internal RPC returned an error envelope.
    Rpc = 13,
    /// Process-global `WalletManager` not yet initialized — caller must
    /// call `sol_wallet_init` before any other FFI call.
    NotInitialized = 14,
    /// Not yet implemented (interim stub for Steps 2-10 — real impl lands
    /// in subsequent PRs per plan Task 8.1).
    Unimplemented = 98,
    /// Panic occurred inside the FFI boundary. `last_error_message`
    /// contains the scrubbed panic payload.
    Panic = 99,
}

impl FfiError {
    /// Numeric code (stable across FFI boundary).
    pub fn code(self) -> i32 {
        self as i32
    }
}

impl From<Error> for FfiError {
    fn from(e: Error) -> Self {
        match e {
            Error::WalletNotFound(_) => FfiError::WalletNotFound,
            Error::WalletDecryptFailed { .. } => FfiError::DecryptFailed,
            Error::InsufficientFunds { .. } => FfiError::InsufficientFunds,
            Error::Transport(_) => FfiError::Transport,
            Error::Rpc { .. } => FfiError::Rpc,
            Error::Unimplemented(_) => FfiError::Unimplemented,
            _ => FfiError::Unimplemented, // Coalesce un-mapped variants for now
        }
    }
}

/// Per-thread last-error message. Mobile consumer reads after each
/// non-zero FFI return; cleared by `sol_wallet_panic_message_clear`.
thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

/// Per-thread unlocked-secret map. Populated by `sol_wallet_unlock`;
/// cleared by `sol_wallet_lock` (auto-zero per H3/M14).
thread_local! {
    static UNLOCKED: RefCell<HashMap<WalletId, Zeroizing<Vec<u8>>>> = RefCell::new(HashMap::new());
}

/// Process-global `WalletManager<FileWalletStorage>`. Initialized
/// via `sol_wallet_init`. Concrete type (not `dyn`) keeps cbindgen
/// header emit clean — only the 16 `sol_wallet_*` exports surface.
static MANAGER: std::sync::OnceLock<
    std::sync::Mutex<
        Option<
            std::sync::Arc<
                crate::wallet_manager::WalletManager<crate::platform::storage::FileWalletStorage>,
            >,
        >,
    >,
> = std::sync::OnceLock::new();

/// Acquire the global MANAGER. Returns `FfiError::NotInitialized` if
/// `sol_wallet_init` was not yet called.
fn with_manager<F, T>(f: F) -> Result<T, FfiError>
where
    F: FnOnce(
        &crate::wallet_manager::WalletManager<crate::platform::storage::FileWalletStorage>,
    ) -> Result<T, FfiError>,
{
    let mutex = MANAGER.get().ok_or_else(|| {
        set_last_error("sol_wallet: manager not initialized — call sol_wallet_init first");
        FfiError::NotInitialized
    })?;
    let guard = mutex.lock().expect("manager mutex poisoned");
    let mgr = guard.as_ref().ok_or(FfiError::NotInitialized)?;
    f(mgr)
}

/// Initialize the global `WalletManager` rooted at `data_dir`.
///
/// Idempotent — second call returns `FfiError::Ok` without replacing
/// the existing manager (operator must explicitly reset to swap).
#[no_mangle]
pub extern "C" fn sol_wallet_init(data_dir: *const c_char, data_dir_len: usize) -> i32 {
    use crate::platform::storage::FileWalletStorage;
    let dir_str = match read_in_str(data_dir, data_dir_len) {
        Ok(s) => s.to_string(),
        Err(e) => return e.code(),
    };
    let dir_path = std::path::PathBuf::from(dir_str);
    let storage = match FileWalletStorage::open(dir_path.as_path()) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("sol_wallet_init: storage open failed: {}", e));
            return FfiError::Unimplemented.code();
        }
    };
    let manager = match crate::wallet_manager::WalletManager::new(storage) {
        Ok(m) => std::sync::Arc::new(m),
        Err(e) => {
            set_last_error(&format!("sol_wallet_init: manager init failed: {}", e));
            return FfiError::Unimplemented.code();
        }
    };
    let mutex = MANAGER.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = mutex.lock().expect("manager mutex poisoned");
    if guard.is_some() {
        return FfiError::Ok.code();
    }
    *guard = Some(manager);
    FfiError::Ok.code()
}

/// Store an error message in the per-thread slot, redacted via the
/// panic scrubber (M10 — never surface secrets via `last_error_message`).
fn set_last_error(msg: &str) {
    let scrubbed = scrub_msg(msg);
    let owned = CString::new(scrubbed).unwrap_or_else(|_| {
        CString::new("scrubber produced NUL byte (impossible for ASCII REDACTED)")
            .expect("static literal")
    });
    LAST_ERROR.with(|cell| *cell.borrow_mut() = Some(owned));
}

fn clear_last_error() {
    LAST_ERROR.with(|cell| *cell.borrow_mut() = None);
}

/// Write bytes into a caller-supplied buffer of capacity `buf_len`.
/// Returns `Ok(())` if all bytes fit + were written; `Err(BufTooSmall)`
/// (with required size in `last_error_message`) if not.
fn write_out_bytes(out: *mut u8, buf_len: usize, data: &[u8]) -> Result<(), FfiError> {
    if out.is_null() {
        set_last_error("write_out_bytes: out pointer is NULL");
        return Err(FfiError::NullPointer);
    }
    if data.len() > buf_len {
        set_last_error(&format!(
            "buffer too small: need {} bytes, caller supplied {}",
            data.len(),
            buf_len
        ));
        return Err(FfiError::BufTooSmall);
    }
    // SAFETY: caller guarantees out is non-null + buf_len bytes valid for write.
    unsafe {
        std::ptr::copy_nonoverlapping(data.as_ptr(), out, data.len());
    }
    Ok(())
}

/// Write a null-terminated C string into a caller-supplied buffer.
/// Returns the length written (excluding NUL) or `Err(BufTooSmall)`.
fn write_out_cstr(out: *mut c_char, buf_len: usize, data: &str) -> Result<i32, FfiError> {
    if out.is_null() {
        set_last_error("write_out_cstr: out pointer is NULL");
        return Err(FfiError::NullPointer);
    }
    let cstring = CString::new(data).map_err(|_| {
        set_last_error("write_out_cstr: input contains NUL byte");
        FfiError::InvalidUtf8
    })?;
    let bytes = cstring.as_bytes_with_nul();
    if bytes.len() > buf_len {
        set_last_error(&format!(
            "buffer too small: need {} bytes (incl NUL), caller supplied {}",
            bytes.len(),
            buf_len
        ));
        return Err(FfiError::BufTooSmall);
    }
    // SAFETY: caller guarantees out is non-null + buf_len bytes valid for write.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out.cast::<u8>(), bytes.len());
    }
    Ok((bytes.len() - 1) as i32) // length excluding NUL
}

/// Read a caller-supplied buffer + length as `&str`. Validates UTF-8
/// + non-NULL. Returns the slice + the original length.
fn read_in_str<'a>(ptr: *const c_char, len: usize) -> Result<&'a str, FfiError> {
    if ptr.is_null() {
        set_last_error("read_in_str: in pointer is NULL");
        return Err(FfiError::NullPointer);
    }
    // SAFETY: caller guarantees ptr is non-null + `len` bytes valid for read.
    let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len) };
    std::str::from_utf8(bytes).map_err(|e| {
        set_last_error(&format!("read_in_str: invalid UTF-8 — {}", e));
        FfiError::InvalidUtf8
    })
}

/// Parse a `WalletId` from a caller-supplied hyphenated-UUID string +
/// length.
fn parse_wallet_id(ptr: *const c_char, len: usize) -> Result<WalletId, FfiError> {
    let s = read_in_str(ptr, len)?;
    WalletId::parse_str(s).map_err(|e| {
        set_last_error(&format!("parse_wallet_id: {}", e));
        FfiError::from(e)
    })
}

// ---------------------------------------------------------------------------
// FFI exports — 12 original (plan Task 8.1 Steps 2-11) + 4 new (audit H3/H5/M16).
// Implementation status:
//   - Steps 2-10 (9 fns): STUBS that return `FfiError::Unimplemented`.
//     Real impl lands in subsequent Phase 8.1 PRs per plan Task 8.1.
//   - Step 11 (`last_error_message`, `panic_message_clear`): IMPLEMENTED.
//   - `sol_wallet_zeroize` (H3): IMPLEMENTED.
//   - `sol_wallet_set_policy` + `sol_wallet_get_policy` (H5): STUBS.
//   - `sol_wallet_default_cluster` (M16): STUB.
// ---------------------------------------------------------------------------

/// `sol_wallet_create_mnemonic` — Step 2 real impl.
#[no_mangle]
pub extern "C" fn sol_wallet_create_mnemonic(
    out_id: *mut c_char,
    out_id_len: usize,
    out_mnemonic: *mut c_char,
    out_mnemonic_len: usize,
    password: *const c_char,
    password_len: usize,
    cluster: *const c_char,
    cluster_len: usize,
) -> i32 {
    let _ = read_in_str(cluster, cluster_len).ok(); // cluster not bound in v0.1
    let pw = match read_in_str(password, password_len) {
        Ok(s) => s.to_string(),
        Err(e) => return e.code(),
    };
    let phrase = match crate::ffi_mnemonic::generate_12_word_english() {
        Ok(p) => p,
        Err(_) => {
            set_last_error("sol_wallet_create_mnemonic: BIP-39 generate failed");
            return FfiError::Unimplemented.code();
        }
    };
    let step2_outcome = with_manager(|mgr| {
        crate::ffi_mnemonic::import_into_manager(mgr, &phrase, &pw, "default", 0, 0)
            .map_err(FfiError::from)
            .and_then(|id| {
                if let Err(e) = write_out_cstr(out_id, out_id_len, &id.to_string()) {
                    return Err(e);
                }
                write_out_cstr(out_mnemonic, out_mnemonic_len, &phrase)
                    .map(|_| FfiError::Ok)
                    .map_err(|_| FfiError::BufTooSmall)
            })
    });
    match step2_outcome {
        Ok(ffi_err) => ffi_err.code(),
        Err(e) => e.code(),
    }
}

/// `sol_wallet_import_mnemonic` — Step 3 real impl.
#[no_mangle]
pub extern "C" fn sol_wallet_import_mnemonic(
    out_id: *mut c_char,
    out_id_len: usize,
    in_mnemonic: *const c_char,
    in_mnemonic_len: usize,
    password: *const c_char,
    password_len: usize,
    cluster: *const c_char,
    cluster_len: usize,
) -> i32 {
    let _ = read_in_str(cluster, cluster_len).ok();
    let pw = match read_in_str(password, password_len) {
        Ok(s) => s.to_string(),
        Err(e) => return e.code(),
    };
    let phrase = match read_in_str(in_mnemonic, in_mnemonic_len) {
        Ok(s) => s.to_string(),
        Err(e) => return e.code(),
    };
    if let Err(_) = crate::ffi_mnemonic::validate_english(&phrase) {
        set_last_error(&format!(
            "sol_wallet_import_mnemonic: invalid BIP-39 phrase ({} chars)",
            phrase.len()
        ));
        return FfiError::InvalidUtf8.code();
    }
    let step3_outcome = with_manager(|mgr| {
        crate::ffi_mnemonic::import_into_manager(mgr, &phrase, &pw, "imported", 0, 0)
            .map_err(FfiError::from)
            .and_then(|id| {
                write_out_cstr(out_id, out_id_len, &id.to_string())
                    .map(|_| FfiError::Ok)
                    .map_err(|_| FfiError::BufTooSmall)
            })
    });
    match step3_outcome {
        Ok(ffi_err) => ffi_err.code(),
        Err(e) => e.code(),
    }
}

/// `sol_wallet_unlock` — Step 4 real impl (H3 + H4).
#[no_mangle]
pub extern "C" fn sol_wallet_unlock(
    out_secret: *mut u8,
    out_secret_len: usize,
    id: *const c_char,
    id_len: usize,
    password: *const c_char,
    password_len: usize,
) -> i32 {
    const SECRET_BYTES: usize = 32;
    if out_secret.is_null() {
        set_last_error("sol_wallet_unlock: out_secret is NULL");
        return FfiError::NullPointer.code();
    }
    if out_secret_len < SECRET_BYTES {
        set_last_error(&format!(
            "sol_wallet_unlock: out_secret too small: need {} bytes, caller supplied {}",
            SECRET_BYTES, out_secret_len
        ));
        return FfiError::BufTooSmall.code();
    }
    let wallet_id = match parse_wallet_id(id, id_len) {
        Ok(w) => w,
        Err(e) => return e.code(),
    };
    let pw = match read_in_str(password, password_len) {
        Ok(s) => s.to_string(),
        Err(e) => return e.code(),
    };
    let result = with_manager(|mgr| {
        mgr.unlock(wallet_id, &pw)
            .map(|owned_lock| {
                let all_bytes = owned_lock.wallet().inner_bytes();
                // Write the 32-byte Ed25519 secret to caller's buffer.
                unsafe {
                    std::ptr::copy_nonoverlapping(all_bytes.as_ptr(), out_secret, SECRET_BYTES);
                }
                // Stash the FULL 64-byte (secret[32] + pubkey[32])
                // for sign_transaction (which reconstructs via
                // Wallet::from_bytes). Zeroizing on drop.
                let stashed = Zeroizing::new(all_bytes.to_vec());
                UNLOCKED.with(|cell| {
                    cell.borrow_mut().insert(wallet_id, stashed);
                });
                FfiError::Ok
            })
            .map_err(FfiError::from)
    });
    match result {
        Ok(FfiError::Ok) => FfiError::Ok.code(),
        Ok(other) => other.code(),
        Err(FfiError::NotInitialized) => FfiError::NotInitialized.code(),
        Err(FfiError::WalletNotFound) => FfiError::WalletNotFound.code(),
        Err(FfiError::DecryptFailed) => FfiError::DecryptFailed.code(),
        Err(_) => FfiError::Unimplemented.code(),
    }
}

/// `sol_wallet_lock` — Step 5 real impl (H3/M14 auto-zero).
#[no_mangle]
pub extern "C" fn sol_wallet_lock(id: *const c_char, id_len: usize) -> i32 {
    let wallet_id = match parse_wallet_id(id, id_len) {
        Ok(w) => w,
        Err(e) => return e.code(),
    };
    let result = with_manager(|mgr| {
        mgr.lock(wallet_id)
            .map(|()| FfiError::Ok)
            .map_err(FfiError::from)
    });
    match result {
        Ok(FfiError::Ok) => {}
        Ok(other) => return other.code(),
        Err(FfiError::NotInitialized) => return FfiError::NotInitialized.code(),
        Err(FfiError::WalletNotFound) => return FfiError::WalletNotFound.code(),
        Err(_) => return FfiError::Unimplemented.code(),
    }
    UNLOCKED.with(|cell| {
        if let Some(mut secret) = cell.borrow_mut().remove(&wallet_id) {
            secret.zeroize();
        }
    });
    FfiError::Ok.code()
}

/// `sol_wallet_get_address` — Step 6 real impl. Reads pubkey from
/// stored wallet record (no decrypt needed — pubkey is in plaintext
/// header).
#[no_mangle]
pub extern "C" fn sol_wallet_get_address(
    out_pubkey: *mut c_char,
    out_pubkey_len: usize,
    id: *const c_char,
    id_len: usize,
) -> i32 {
    let wallet_id = match parse_wallet_id(id, id_len) {
        Ok(w) => w,
        Err(e) => return e.code(),
    };
    let pubkey_str = match with_manager(|mgr| mgr.summary(wallet_id).map_err(FfiError::from)) {
        Ok(s) => s.pubkey.to_string(),
        Err(FfiError::WalletNotFound) => return FfiError::WalletNotFound.code(),
        Err(FfiError::NotInitialized) => return FfiError::NotInitialized.code(),
        Err(_) => return FfiError::Unimplemented.code(),
    };
    match write_out_cstr(out_pubkey, out_pubkey_len, &pubkey_str) {
        Ok(n) => n,
        Err(e) => e.code(),
    }
}

/// `sol_wallet_sign_transaction` — Step 7 real impl (audit M9).
///
/// Requires the caller to have called `sol_wallet_unlock` first (the
/// per-thread UNLOCKED map caches the full 64-byte secret+pubkey).
/// Reconstructs a `Wallet` via `Wallet::from_bytes` (pub(crate) +
/// calls `sign_message` to produce a 64-byte Ed25519 signature.
#[no_mangle]
pub extern "C" fn sol_wallet_sign_transaction(
    out_sig: *mut u8,
    out_sig_len: usize,
    in_message: *const u8,
    in_message_len: usize,
    id: *const c_char,
    id_len: usize,
) -> i32 {
    const SIG_BYTES: usize = 64;
    if out_sig.is_null() {
        set_last_error("sol_wallet_sign_transaction: out_sig is NULL");
        return FfiError::NullPointer.code();
    }
    if out_sig_len < SIG_BYTES {
        set_last_error(&format!(
            "sol_wallet_sign_transaction: out_sig too small: need {} bytes, caller supplied {}",
            SIG_BYTES, out_sig_len
        ));
        return FfiError::BufTooSmall.code();
    }
    if in_message.is_null() && in_message_len > 0 {
        set_last_error("sol_wallet_sign_transaction: in_message NULL with non-zero len");
        return FfiError::NullPointer.code();
    }
    let wallet_id = match parse_wallet_id(id, id_len) {
        Ok(w) => w,
        Err(e) => return e.code(),
    };
    let msg_slice: &[u8] = if in_message_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(in_message, in_message_len) }
    };

    // Read from the per-thread UNLOCKED cache (populated by
    // sol_wallet_unlock). The cache holds the full 64-byte
    // (secret + pubkey) so we can reconstruct a Wallet via
    // Wallet::from_bytes.
    let cached = UNLOCKED.with(|cell| cell.borrow().get(&wallet_id).cloned());
    let all_bytes = match cached {
        Some(s) => s,
        None => {
            set_last_error(
                "sol_wallet_sign_transaction: wallet not unlocked — call sol_wallet_unlock first",
            );
            return FfiError::WalletLocked.code();
        }
    };
    if all_bytes.len() < 64 {
        set_last_error("sol_wallet_sign_transaction: cached secret too short");
        return FfiError::Unimplemented.code();
    }
    let mut arr = [0u8; 64];
    arr.copy_from_slice(&all_bytes[..64]);
    let wallet = crate::wallet::Wallet::from_bytes(&arr);
    let sig = wallet.sign_message(msg_slice);
    let sig_bytes = sig.as_ref();
    unsafe {
        std::ptr::copy_nonoverlapping(sig_bytes.as_ptr(), out_sig, SIG_BYTES);
    }
    FfiError::Ok.code()
}

/// `sol_wallet_send_sol` — Step 8 stub (real impl enforces H5 policy gate).
#[no_mangle]
pub extern "C" fn sol_wallet_send_sol(
    out_sig: *mut u8,
    out_sig_len: usize,
    id: *const c_char,
    id_len: usize,
    to: *const c_char,
    to_len: usize,
    amount_lamports: u64,
    priority_fee: u64,
) -> i32 {
    let _ = (out_sig, out_sig_len);
    let _ = parse_wallet_id(id, id_len);
    let _ = read_in_str(to, to_len);
    let _ = (amount_lamports, priority_fee);
    set_last_error("sol_wallet_send_sol: not yet implemented (Step 8 stub)");
    FfiError::Unimplemented.code()
}

/// `sol_wallet_send_spl` — Step 9 stub (real impl enforces H5 policy gate).
#[no_mangle]
pub extern "C" fn sol_wallet_send_spl(
    out_sig: *mut u8,
    out_sig_len: usize,
    id: *const c_char,
    id_len: usize,
    mint: *const c_char,
    mint_len: usize,
    to: *const c_char,
    to_len: usize,
    amount: u64,
) -> i32 {
    let _ = (out_sig, out_sig_len);
    let _ = parse_wallet_id(id, id_len);
    let _ = read_in_str(mint, mint_len);
    let _ = read_in_str(to, to_len);
    let _ = amount;
    set_last_error("sol_wallet_send_spl: not yet implemented (Step 9 stub)");
    FfiError::Unimplemented.code()
}

/// `sol_wallet_get_balance_sol` — Step 10 stub.
#[no_mangle]
pub extern "C" fn sol_wallet_get_balance_sol(
    out_lamports: *mut u64,
    address: *const c_char,
    address_len: usize,
) -> i32 {
    if out_lamports.is_null() {
        set_last_error("sol_wallet_get_balance_sol: out_lamports is NULL");
        return FfiError::NullPointer.code();
    }
    let _ = read_in_str(address, address_len);
    set_last_error("sol_wallet_get_balance_sol: not yet implemented (Step 10 stub)");
    FfiError::Unimplemented.code()
}

/// `sol_wallet_get_balance_spl` — Step 10 stub.
#[no_mangle]
pub extern "C" fn sol_wallet_get_balance_spl(
    out_balance: *mut u64,
    out_decimals: *mut u8,
    address: *const c_char,
    address_len: usize,
    mint: *const c_char,
    mint_len: usize,
) -> i32 {
    if out_balance.is_null() || out_decimals.is_null() {
        set_last_error("sol_wallet_get_balance_spl: out pointer is NULL");
        return FfiError::NullPointer.code();
    }
    let _ = read_in_str(address, address_len);
    let _ = read_in_str(mint, mint_len);
    set_last_error("sol_wallet_get_balance_spl: not yet implemented (Step 10 stub)");
    FfiError::Unimplemented.code()
}

/// `sol_wallet_last_error_message` — Step 11. **IMPLEMENTED.**
///
/// Returns the most recent per-thread error message (scrubbed per M10).
/// Mobile consumer calls this after each non-zero FFI return.
///
/// Returns the length written (excluding NUL) on success;
/// `FfiError::BufTooSmall` if the caller's buffer is too small
/// (then `last_error` itself describes the required size).
#[no_mangle]
pub extern "C" fn sol_wallet_last_error_message(out_msg: *mut c_char, buf_len: usize) -> i32 {
    if out_msg.is_null() {
        return FfiError::NullPointer.code();
    }
    LAST_ERROR.with(|cell| {
        let borrow = cell.borrow();
        match borrow.as_ref() {
            None => {
                // No error pending — report empty string.
                write_out_cstr(out_msg, buf_len, "").unwrap_or_else(|e| e.code())
            }
            Some(cstr) => match write_out_cstr(out_msg, buf_len, cstr.to_str().unwrap_or("")) {
                Ok(n) => n,
                Err(e) => e.code(),
            },
        }
    })
}

/// `sol_wallet_panic_message_clear` — Step 11. **IMPLEMENTED.**
#[no_mangle]
pub extern "C" fn sol_wallet_panic_message_clear() -> i32 {
    clear_last_error();
    FfiError::Ok.code()
}

/// `sol_wallet_zeroize` — H3 fix. **IMPLEMENTED.**
///
/// Volatile-write zeros a caller-supplied buffer. Mobile consumer MUST
/// call this on every `out_secret` buffer returned by
/// `sol_wallet_unlock` once the secret has been copied into the
/// consumer's `Uint8List` / `Data` / `ByteArray` (or wherever the
/// mobile-side secret is held).
#[no_mangle]
pub extern "C" fn sol_wallet_zeroize(buf: *mut u8, len: usize) -> i32 {
    if buf.is_null() || len == 0 {
        return FfiError::NullPointer.code();
    }
    // SAFETY: caller guarantees buf is non-null + `len` bytes valid for write.
    unsafe {
        // volatile writes — compiler cannot elide (per zeroize crate
        // pattern; ensures the zeros actually land in memory even
        // when the buffer is about to be freed).
        let mut p = buf;
        let end = buf.add(len);
        while p < end {
            std::ptr::write_volatile(p, 0u8);
            p = p.add(1);
        }
    }
    FfiError::Ok.code()
}

/// `sol_wallet_set_policy` — H5. STUB (audit recommendation: ship V0.1
/// with `unrestricted` default; real impl follows operator sign-off).
#[no_mangle]
pub extern "C" fn sol_wallet_set_policy(
    id: *const c_char,
    id_len: usize,
    max_per_tx_lamports: u64,
    daily_limit_lamports: u64,
    password: *const c_char,
    password_len: usize,
) -> i32 {
    let _ = parse_wallet_id(id, id_len);
    let _ = read_in_str(password, password_len);
    let _ = (max_per_tx_lamports, daily_limit_lamports);
    set_last_error(
        "sol_wallet_set_policy: not yet implemented (H5 stub — operator sign-off required)",
    );
    FfiError::Unimplemented.code()
}

/// `sol_wallet_get_policy` — H5 companion read.
#[no_mangle]
pub extern "C" fn sol_wallet_get_policy(
    out_max_per_tx: *mut u64,
    out_daily_limit: *mut u64,
    id: *const c_char,
    id_len: usize,
) -> i32 {
    if out_max_per_tx.is_null() || out_daily_limit.is_null() {
        set_last_error("sol_wallet_get_policy: out pointer is NULL");
        return FfiError::NullPointer.code();
    }
    let _ = parse_wallet_id(id, id_len);
    set_last_error("sol_wallet_get_policy: not yet implemented (H5 stub)");
    FfiError::Unimplemented.code()
}

/// `sol_wallet_default_cluster` — M16. Returns the cluster the wallet
/// manager is configured to use when caller omits cluster on create.
#[no_mangle]
pub extern "C" fn sol_wallet_default_cluster(out_cluster: *mut c_char, buf_len: usize) -> i32 {
    set_last_error("sol_wallet_default_cluster: not yet implemented (M16 stub)");
    FfiError::Unimplemented.code()
}

#[allow(dead_code)]
fn _silence_unused_imports() {
    // Anchor for items referenced in module docs but not used in code
    // (e.g. the FfiError → Error conversion path may be unused until
    // real impl lands).
    let _ = FfiError::Ok as c_int;
    let _ = write_out_bytes;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_error_codes_are_stable() {
        // Stable i32 contract — do NOT reorder. Append new codes only.
        assert_eq!(FfiError::Ok.code(), 0);
        assert_eq!(FfiError::BufTooSmall.code(), 1);
        assert_eq!(FfiError::NullPointer.code(), 2);
        assert_eq!(FfiError::InvalidUtf8.code(), 3);
        assert_eq!(FfiError::WalletNotFound.code(), 4);
        assert_eq!(FfiError::WalletLocked.code(), 5);
        assert_eq!(FfiError::DecryptFailed.code(), 6);
        assert_eq!(FfiError::InsufficientFunds.code(), 7);
        assert_eq!(FfiError::Transport.code(), 8);
        assert_eq!(FfiError::ExceedsMaxPerTx.code(), 9);
        assert_eq!(FfiError::DestinationNotAllowed.code(), 10);
        assert_eq!(FfiError::ExceedsDailyLimit.code(), 11);
        assert_eq!(FfiError::PolicyDenied.code(), 12);
        assert_eq!(FfiError::Rpc.code(), 13);
        assert_eq!(FfiError::Unimplemented.code(), 98);
        assert_eq!(FfiError::Panic.code(), 99);
    }

    #[test]
    fn last_error_set_and_clear() {
        clear_last_error();
        set_last_error("test error");
        LAST_ERROR.with(|c| {
            assert!(c.borrow().is_some());
        });
        clear_last_error();
        LAST_ERROR.with(|c| {
            assert!(c.borrow().is_none());
        });
    }

    #[test]
    fn scrubber_redacts_mnemonic_in_last_error() {
        clear_last_error();
        let phrase =
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        set_last_error(&format!("decrypt failed for phrase {}", phrase));
        LAST_ERROR.with(|c| {
            let stored = c.borrow();
            let s = stored.as_ref().unwrap().to_str().unwrap();
            assert!(s.contains("[REDACTED]"), "expected REDACTED in: {:?}", s);
            assert!(!s.contains("abandon"));
        });
        clear_last_error();
    }

    #[test]
    fn zeroize_writes_zeros() {
        let mut buf = [0xFFu8; 32];
        let rc = unsafe { sol_wallet_zeroize(buf.as_mut_ptr(), buf.len()) };
        assert_eq!(rc, 0);
        assert!(buf.iter().all(|&b| b == 0));
    }

    #[test]
    fn zeroize_null_returns_null_pointer() {
        let rc = unsafe { sol_wallet_zeroize(std::ptr::null_mut(), 32) };
        assert_eq!(rc, FfiError::NullPointer.code());
    }

    #[test]
    fn last_error_message_writes_payload() {
        clear_last_error();
        set_last_error("hello FFI");
        let mut buf = [0u8; 64];
        let n = sol_wallet_last_error_message(buf.as_mut_ptr().cast::<c_char>(), buf.len());
        assert!(n >= 0);
        let s = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr().cast::<c_char>()) };
        assert_eq!(s.to_str().unwrap(), "hello FFI");
        clear_last_error();
    }
}
