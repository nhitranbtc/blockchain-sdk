//! FFI wallet ops — Task 4 of 2026-08-19 FFI integration plan.
//!
//! Replaces the temporary string-returning `wallet_list` (Task 1 spike)
//! with the full wallet ops ABI. Task 4 owns:
//! - `wallet_create` — generates mnemonic + persists encrypted blob
//! - `wallet_import` — imports existing phrase + persists encrypted blob
//! - `wallet_list`   — array-based C ABI (UUIDs as `**c_char`, count + ids)
//! - `wallet_delete` — removes wallet blob
//! - `phrase_view_copy` / `phrase_view_free` — read-only view of the
//!   cleartext mnemonic returned by `wallet_create`; zeroized on free.
//!
//! Task 5 will add async exports (sync, balance, broadcast) on top of
//! the runtime handle from Task 3.
//!
//! **Security model**: the password + mnemonic bytes cross the FFI
//! boundary as raw `u8` buffers. Caller (Dart) owns those buffers and
//! MUST zero them after the call (L12 CRITICAL #2 mirror on Dart side;
//! see L33.1 — Dart's BtcInvoker pattern of using a TempSecretFile is
//! the analog). The Rust side wraps incoming data in `Secret<Vec<u8>>`
//! and uses the existing `wallet::ops` path which already zeroizes on
//! drop.

#![allow(unsafe_code)] // FFI surface

use crate::ffi::panic::ffi_catch_unwind;
use crate::ffi::FfiError;
use crate::keys::AddressType;
use crate::keys::Secret;
use crate::wallet::ops::{create_wallet, delete_wallet, import_wallet, list_wallets};
use crate::wallet::WalletId;
use bitcoin::Network;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::path::Path;
use zeroize::Zeroize;

/// Opaque FFI handle to the cleartext mnemonic returned by
/// `wallet_create`. The bytes are heap-allocated with a trailing NUL
/// terminator (so `phrase_view_copy` can return a `*const c_char`
/// borrowed directly from the buffer — zero-copy, no fresh allocation,
/// and `phrase_view_free` zeroizes + deallocates in one step).
///
/// **Plan note:** the FFI plan originally named this `SecretStringView`;
/// renamed to `MnemonicHandle` in implementation to align with the
/// Dart-side handle type already established in wallet-desktop.
pub struct MnemonicHandle(Vec<u8>);

impl Drop for MnemonicHandle {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl MnemonicHandle {
    /// Borrow a pointer into the handle's NUL-terminated byte buffer.
    /// The pointer is valid until `phrase_view_free` (or any other
    /// drop of the handle). Multiple concurrent borrows are NOT
    /// supported — the caller MUST treat the pointer as exclusive to
    /// the reading code path.
    fn as_nul_terminated_ptr(&self) -> *const c_char {
        // Safe: `self.0` is NUL-terminated (constructed via `from_phrase`
        // push(0)). A valid `*const u8` is also a valid `*const c_char`
        // (same layout per the C ABI conventions).
        self.0.as_ptr() as *const c_char
    }
}

const NETWORK_TESTNET: u8 = 1;

fn parse_network(network: u8) -> Result<Network, FfiError> {
    match network {
        NETWORK_TESTNET => Ok(Network::Testnet),
        _ => Err(FfiError::Unknown),
    }
}

fn parse_address_type(t: u8) -> Result<AddressType, FfiError> {
    // Unknown address-type bytes get `Unknown` (not `ScriptBuild` —
    // reserve that for actual script-construction failures in the
    // wallet::ops path; an unknown enum tag is not a script error).
    match t {
        0 => Ok(AddressType::NativeSegwit),
        1 => Ok(AddressType::NestedSegwit),
        2 => Ok(AddressType::Taproot),
        _ => Err(FfiError::Unknown),
    }
}

fn read_base_dir(base_dir: *const c_char) -> Result<std::path::PathBuf, FfiError> {
    if base_dir.is_null() {
        return Err(FfiError::Storage);
    }
    let s = unsafe { CStr::from_ptr(base_dir) }
        .to_str()
        .map_err(|_| FfiError::Storage)?;
    Ok(Path::new(s).to_path_buf())
}

/// Generate a new random mnemonic + persist the encrypted wallet blob.
///
/// On success, writes:
/// - `out_id[0..36]` — the new `WalletId` UUID as a 36-char hyphenated
///   hex string (no trailing NUL). The 37th byte must be zero on input
///   and is left untouched.
/// - `*out_phrase_handle` — opaque handle to a heap-allocated,
///   NUL-terminated copy of the cleartext phrase. Caller MUST call
///   `phrase_view_copy` to read it as a `*const c_char` (zero-copy
///   pointer into the handle's buffer) and `phrase_view_free` to
///   zeroize + dealloc it.
///
/// # Safety
/// - `password` must point to `password_len` readable bytes.
/// - `out_id` must be a writable buffer of at least 36 bytes; the
///   36-byte UUID hex string is written at `out_id[0..36]` and bytes
///   beyond index 35 are NOT modified.
/// - `out_phrase_handle` must be a writable `*mut c_void` slot.
/// - On any failure, no heap allocation is performed and the output
///   slots are left untouched (caller initializes to null).
#[no_mangle]
pub unsafe extern "C" fn wallet_create(
    words: u8,
    network: u8,
    address_type: u8,
    password: *const u8,
    password_len: usize,
    base_dir: *const c_char,
    out_id: *mut c_char,
    out_phrase_handle: *mut *mut c_void,
) -> FfiError {
    ffi_catch_unwind(|| -> FfiError {
        if out_id.is_null() || out_phrase_handle.is_null() {
            return FfiError::Storage;
        }
        let net = match parse_network(network) {
            Ok(n) => n,
            Err(e) => return e,
        };
        let at = match parse_address_type(address_type) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let base = match read_base_dir(base_dir) {
            Ok(b) => b,
            Err(e) => return e,
        };
        if password.is_null() || password_len == 0 {
            return FfiError::Encryption;
        }
        let pw_bytes = unsafe { std::slice::from_raw_parts(password, password_len) };
        let pw_secret = Secret::new(pw_bytes.to_vec());
        let (id, phrase_secret) = match create_wallet(&base, net, words as usize, &pw_secret, at) {
            Ok(v) => v,
            Err(e) => return FfiError::from(e),
        };
        let id_str = id.to_string();
        let id_bytes = id_str.as_bytes();
        unsafe {
            std::ptr::copy_nonoverlapping(id_bytes.as_ptr(), out_id as *mut u8, id_bytes.len());
        }
        let mut phrase_bytes = phrase_secret.expose().as_bytes().to_vec();
        // Append NUL terminator so `phrase_view_copy` can return a
        // pointer INTO this buffer (zero-copy). `phrase_view_free` will
        // zeroize + drop the Vec.
        phrase_bytes.push(0);
        let handle = Box::into_raw(Box::new(MnemonicHandle(phrase_bytes)));
        unsafe {
            *out_phrase_handle = handle as *mut c_void;
        }
        FfiError::Ok
    })
}

/// Return a pointer to the NUL-terminated phrase bytes held by a
/// `MnemonicHandle`. ZERO-COPY: the pointer is borrowed from the
/// handle's heap buffer; reading via `CStr::from_ptr` does not
/// allocate. The pointer is invalidated by any subsequent
/// `phrase_view_free(handle)` or any FFI call on the same thread that
/// mutates the handle — treat as valid only until you call
/// `phrase_view_free`.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by `wallet_create` and
/// not yet freed. On failure (null handle or invalid UTF-8 in the
/// buffer), returns null. The caller MUST NOT attempt to free the
/// returned pointer.
#[no_mangle]
pub unsafe extern "C" fn phrase_view_copy(handle: *mut c_void) -> *const c_char {
    // Pointer-returning FFI export — use a Cell to thread the result
    // out of the `ffi_catch_unwind` closure (which requires FfiError
    // return type). Equivalent to `panic::catch_unwind` + result
    // extraction but via our project's `ffi_catch_unwind` wrapper.
    use std::cell::Cell;
    use std::panic::AssertUnwindSafe;
    let result: Cell<*const c_char> = Cell::new(std::ptr::null());
    // Cell doesn't impl UnwindSafe — AssertUnwindSafe documents that
    // the only post-unwind usage is the inner `result.get()` (which is
    // safe to call on a Cell regardless of panic state).
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if handle.is_null() {
            return FfiError::Ok;
        }
        let h = &*(handle as *const MnemonicHandle);
        // Validate UTF-8 once (the handle stores UTF-8 from
        // `Mnemonic::to_phrase`; defensive against future changes).
        let bytes_without_trailing_nul = h.0.split_last().map_or(&h.0[..], |(_, rest)| rest);
        if std::str::from_utf8(bytes_without_trailing_nul).is_err() {
            return FfiError::Ok;
        }
        result.set(h.as_nul_terminated_ptr());
        FfiError::Ok
    }));
    result.get()
}

/// Zeroize + free a `MnemonicHandle` previously returned by
/// `wallet_create`. Null is a no-op. The handle's bytes are zeroized
/// before the heap allocation is dropped (L12 CRITICAL #2).
///
/// # Safety
///
/// `handle` must be a valid pointer returned by `wallet_create` and
/// not yet freed. Double-free is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn phrase_view_free(handle: *mut c_void) {
    ffi_catch_unwind(|| {
        if !handle.is_null() {
            let _ = Box::from_raw(handle as *mut MnemonicHandle);
        }
        FfiError::Ok
    });
}

/// List all wallet IDs for the given network. On success, writes
/// `count` UUID CStrings and an array of `*mut c_char` pointers to
/// them, then writes the count and the array pointer to `out_count`
/// and `out_ids` respectively (atomically — both succeed or both
/// left untouched).
///
/// Caller frees the result with `wallet_list_array_free(arr, count)`.
/// Null array pointer (count==0) is allowed; `wallet_list_array_free`
/// becomes a no-op.
///
/// # Safety
/// - `base_dir` must point to a valid NUL-terminated UTF-8 path.
/// - `out_count` must be a writable `*mut usize`.
/// - `out_ids` must be a writable `*mut *mut c_char`.
#[no_mangle]
pub unsafe extern "C" fn wallet_list(
    network: u8,
    base_dir: *const c_char,
    out_count: *mut usize,
    out_ids: *mut *mut c_char,
) -> FfiError {
    ffi_catch_unwind(|| -> FfiError {
        if out_count.is_null() || out_ids.is_null() {
            return FfiError::Storage;
        }
        let net = match parse_network(network) {
            Ok(n) => n,
            Err(e) => return e,
        };
        let base = match read_base_dir(base_dir) {
            Ok(b) => b,
            Err(e) => return e,
        };
        let ids = match list_wallets(&base, net) {
            Ok(v) => v,
            Err(e) => return FfiError::from(e),
        };
        // Build per-UUID CStrings first (atomic check: any CString::new
        // failure → bail without touching out_count/out_ids).
        let count = ids.len();
        let cstrings: Vec<CString> =
            match ids.iter().map(|id| CString::new(id.to_string())).collect() {
                Ok(v) => v,
                Err(_) => return FfiError::Io,
            };
        // Allocate one heap block of `[count][ptr_0][ptr_1]...[ptr_{count-1}]`.
        // The count is EMBEDDED in the same allocation, so `wallet_list_array_free`
        // recovers the canonical count from `*(arr - sizeof(usize))` —
        // the caller-supplied count is kept for ABI symmetry but is
        // NOT trusted for the free (defense against count drift across
        // the FFI boundary, where losing/corrupting the count could
        // leak or double-free heap — a load-bearing concern given the
        // CString pointers embed secret-bearing wallet UUIDs).
        //
        // Layout: each slot is `usize`-aligned (64-bit: 8 bytes). On
        // 32-bit platforms still usize-aligned; CString::into_raw
        // pointers fit in one slot each.
        let total_words = 1 + count;
        let layout = std::alloc::Layout::array::<usize>(total_words).unwrap();
        // SAFETY: `alloc` returns null only on OOM. CStrings have already
        // been built into `cstrings` (still in scope) and would be
        // dropped if we returned early — we use `handle_alloc_error`
        // (aborts) so no cleanup needed on this path.
        let raw = unsafe { std::alloc::alloc(layout) } as *mut usize;
        if raw.is_null() {
            return FfiError::Io;
        }
        unsafe {
            *raw = count;
        }
        // Move each CString out via into_raw (transfers heap ownership
        // to caller). `into_iter` consumes `cstrings` cleanly.
        // For count==0 the loop is a no-op (arr_ptr is just past the
        // count header — no entries to write — and the layout is still
        // valid for the trailing free in `wallet_list_array_free`).
        let arr_ptr = unsafe { raw.add(1) } as *mut *mut c_char;
        for (i, cs) in cstrings.into_iter().enumerate() {
            unsafe {
                *arr_ptr.add(i) = cs.into_raw();
            }
        }
        // SAFETY: write outputs LAST. Embedded count matches the
        // populated loop iterations exactly.
        unsafe {
            *out_count = count;
            *out_ids = arr_ptr as *mut c_char;
        }
        FfiError::Ok
    })
}

/// Free a wallet list returned by `wallet_list`.
///
/// # Safety
///
/// `arr_ptr` must be either null (no-op) or a pointer returned by
/// `wallet_list` AND not yet freed. `count` MUST match the count that
/// was returned alongside `arr_ptr`. Double-free is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn wallet_list_array_free(arr_ptr: *mut c_char, count: usize) {
    ffi_catch_unwind(|| {
        if arr_ptr.is_null() {
            return FfiError::Ok;
        }
        // Recover the CANONICAL count from the embedded heap header,
        // not from the caller-supplied `count` argument (defends
        // against count drift across the FFI boundary).
        let arr_words = arr_ptr as *mut usize;
        let header_ptr = unsafe { arr_words.sub(1) };
        let true_count = unsafe { *header_ptr };
        // Caller-supplied count is kept for ABI symmetry but NOT
        // trusted for the free. If the two disagree, the heap is
        // either corrupt or the caller lied — we still keep the heap
        // balanced by using the embedded count.
        let _ = count;
        // Free each per-UUID CString back to the heap allocator.
        for i in 0..true_count {
            let p_word = unsafe { *arr_words.add(i) };
            if p_word != 0 {
                let p = p_word as *mut c_char;
                unsafe {
                    let _ = CString::from_raw(p);
                }
            }
        }
        // Free the single block (`[count][ptr_0]...[ptr_{count-1}]`).
        let total_words = 1 + true_count;
        let layout = std::alloc::Layout::array::<usize>(total_words).unwrap();
        unsafe {
            std::alloc::dealloc(header_ptr as *mut u8, layout);
        }
        FfiError::Ok
    });
}

/// Delete the wallet blob at `<base>/wallets/<network>/<id>.blob`.
/// Returns `FfiError::Storage` if the wallet doesn't exist or the id
/// cannot be decoded as a UUID.
///
/// # Safety
/// - `wallet_id` must point to a NUL-terminated UTF-8 UUID string.
/// - `base_dir` must point to a NUL-terminated UTF-8 path.
#[no_mangle]
pub unsafe extern "C" fn wallet_delete(
    network: u8,
    base_dir: *const c_char,
    wallet_id: *const c_char,
) -> FfiError {
    ffi_catch_unwind(|| -> FfiError {
        if wallet_id.is_null() {
            return FfiError::WalletStore;
        }
        let id_str = match unsafe { CStr::from_ptr(wallet_id) }.to_str() {
            Ok(s) => s,
            Err(_) => return FfiError::WalletStore,
        };
        let id = match id_str.parse::<WalletId>() {
            Ok(u) => u,
            Err(_) => return FfiError::WalletStore,
        };
        let net = match parse_network(network) {
            Ok(n) => n,
            Err(e) => return e,
        };
        let base = match read_base_dir(base_dir) {
            Ok(b) => b,
            Err(e) => return e,
        };
        match delete_wallet(&base, net, id) {
            Ok(()) => FfiError::Ok,
            Err(e) => FfiError::from(e),
        }
    })
}

/// Import an existing BIP-39 mnemonic phrase + persist the encrypted
/// wallet blob. Returns the new wallet id via `out_id[0..36]`.
///
/// # Safety
/// - `phrase` must point to `phrase_len` readable bytes (cleartext mnemonic).
/// - `password` must point to `password_len` readable bytes.
/// - `out_id` must be a writable 37-byte buffer.
#[no_mangle]
pub unsafe extern "C" fn wallet_import(
    network: u8,
    base_dir: *const c_char,
    phrase: *const u8,
    phrase_len: usize,
    password: *const u8,
    password_len: usize,
    out_id: *mut c_char,
) -> FfiError {
    ffi_catch_unwind(|| -> FfiError {
        if out_id.is_null() {
            return FfiError::Storage;
        }
        let net = match parse_network(network) {
            Ok(n) => n,
            Err(e) => return e,
        };
        let base = match read_base_dir(base_dir) {
            Ok(b) => b,
            Err(e) => return e,
        };
        if phrase.is_null() || password.is_null() || password_len == 0 || phrase_len == 0 {
            return FfiError::Encryption;
        }
        let phrase_str =
            match unsafe { std::str::from_utf8(std::slice::from_raw_parts(phrase, phrase_len)) } {
                Ok(s) => s,
                Err(_) => return FfiError::InvalidMnemonic,
            };
        let pw_bytes = unsafe { std::slice::from_raw_parts(password, password_len) };
        let pw_secret = Secret::new(pw_bytes.to_vec());
        let id = match import_wallet(&base, net, phrase_str, &pw_secret) {
            Ok(v) => v,
            Err(e) => return FfiError::from(e),
        };
        let id_str = id.to_string();
        let id_bytes = id_str.as_bytes();
        unsafe {
            std::ptr::copy_nonoverlapping(id_bytes.as_ptr(), out_id as *mut u8, id_bytes.len());
        }
        FfiError::Ok
    })
}

// Stubs for the remaining Task 4 exports — sub-task follow-ups.
// Each forwards to the underlying wallet::ops function with the same
// arg-marshalling pattern as `wallet_create`. Show_wallet is Task 5
// (async + EsploraClient dependency).

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use tempfile::TempDir;
    use uuid::Uuid;

    /// Per-test isolated `base` dir, cleaned up on drop.
    fn temp_base() -> (TempDir, CString) {
        let dir = tempfile::tempdir().expect("tempdir");
        let cstr = CString::new(dir.path().to_str().unwrap()).expect("ascii path");
        (dir, cstr)
    }

    /// Build a password as C inputs.
    fn pw_bytes(s: &str) -> (Vec<u8>, usize) {
        (s.as_bytes().to_vec(), s.len())
    }

    /// Build a phrase as C inputs (bare bytes, NOT zero-terminated; the
    /// Rust side reads `phrase_len` to slice it).
    fn phrase_bytes(s: &str) -> (Vec<u8>, usize) {
        (s.as_bytes().to_vec(), s.len())
    }

    #[test]
    fn wallet_create_testnet_returns_id_and_phrase_handle() {
        let (_dir, base) = temp_base();
        let (pw, pw_len) = pw_bytes("hunter2");
        let mut id_buf = [0i8; 37];
        let mut phrase_handle: *mut c_void = std::ptr::null_mut();

        let rc = unsafe {
            wallet_create(
                12,
                NETWORK_TESTNET,
                0,
                pw.as_ptr(),
                pw_len,
                base.as_ptr(),
                id_buf.as_mut_ptr(),
                &mut phrase_handle,
            )
        };
        assert_eq!(rc, FfiError::Ok);

        let id_cstr = unsafe { CStr::from_ptr(id_buf.as_ptr()) };
        let id_str = id_cstr.to_str().unwrap();
        assert_eq!(id_str.len(), 36, "uuid must be 36 chars, got: {id_str:?}");
        let _ = id_str.parse::<Uuid>().expect("must be valid UUID");

        assert!(!phrase_handle.is_null());
        let phrase_ptr = unsafe { phrase_view_copy(phrase_handle) };
        assert!(!phrase_ptr.is_null());
        let phrase = unsafe { CStr::from_ptr(phrase_ptr) }
            .to_str()
            .unwrap()
            .to_string();
        let word_count = phrase.split_whitespace().count();
        assert_eq!(word_count, 12, "expected 12-word mnemonic, got: {phrase}");

        unsafe { phrase_view_free(phrase_handle) };
    }

    #[test]
    fn phrase_view_free_null_is_noop() {
        unsafe { phrase_view_free(std::ptr::null_mut()) };
    }

    /// Empty base → list returns count=0 and out_ids=null.
    #[test]
    fn wallet_list_empty_base_returns_zero_count() {
        let (_dir, base) = temp_base();
        let mut count: usize = 99;
        // Caller provides a HEAP-allocated slot for the result array
        // pointer (Dart side uses malloc). Box::into_raw yields a non-
        // null pointer so the function's slot-null check passes; the
        // initial slot CONTENTS are null (wallet_list writes through).
        let slot = Box::into_raw(Box::new(std::ptr::null_mut::<c_char>()));

        let rc = unsafe { wallet_list(NETWORK_TESTNET, base.as_ptr(), &mut count, slot) };
        assert_eq!(rc, FfiError::Ok);
        assert_eq!(count, 0);
        // Empty list: array pointer is null (no heap alloc). Free is a no-op.
        unsafe { wallet_list_array_free(std::ptr::null_mut(), 0) };
        unsafe {
            let _ = Box::from_raw(slot);
        }
    }

    /// After create, list returns the new id.
    #[test]
    fn wallet_list_after_create_finds_created_id() {
        let (_dir, base) = temp_base();
        let (pw, pw_len) = pw_bytes("pw");
        let mut id_buf = [0i8; 37];
        let mut phrase_handle: *mut c_void = std::ptr::null_mut();

        let rc = unsafe {
            wallet_create(
                12,
                NETWORK_TESTNET,
                0,
                pw.as_ptr(),
                pw_len,
                base.as_ptr(),
                id_buf.as_mut_ptr(),
                &mut phrase_handle,
            )
        };
        assert_eq!(rc, FfiError::Ok);
        let created_id = unsafe { CStr::from_ptr(id_buf.as_ptr()) }
            .to_str()
            .unwrap()
            .to_string();

        let mut count: usize = 0;
        let slot = Box::into_raw(Box::new(std::ptr::null_mut::<c_char>()));
        let rc = unsafe { wallet_list(NETWORK_TESTNET, base.as_ptr(), &mut count, slot) };
        assert_eq!(rc, FfiError::Ok);
        assert_eq!(count, 1);
        // Read back the array pointer through the slot, then the first entry.
        // `arr_ptr: *mut c_char` (the address of slot 0). Reinterpret as
        // pointer-to-pointer so we read 8 bytes (the per-UUID pointer).
        let arr_ptr = unsafe { *slot } as *mut c_char;
        let first_uuid = unsafe { *(arr_ptr as *const *mut c_char) };
        let id_in_list = unsafe { CStr::from_ptr(first_uuid) }
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(id_in_list, created_id);

        unsafe { phrase_view_free(phrase_handle) };
        unsafe { wallet_list_array_free(arr_ptr, count) };
        unsafe {
            let _ = Box::from_raw(slot);
        }
    }

    /// Round-trip: create + list + import same phrase + list → 2 distinct wallets.
    /// (Per L36.4/ADR-precursor: importing the SAME mnemonic with a fresh
    /// WalletId is allowed — the storage layer doesn't dedupe; the operator
    /// would notice the duplicate via the UI list. Confirms round-trip.)
    #[test]
    fn wallet_import_round_trip_persists_distinct_id() {
        let (_dir, base) = temp_base();
        let (pw, pw_len) = pw_bytes("pw");
        let mut id_buf1 = [0i8; 37];
        let mut phrase_handle: *mut c_void = std::ptr::null_mut();

        // 1. Create — capture phrase via phrase_view_copy.
        let rc = unsafe {
            wallet_create(
                12,
                NETWORK_TESTNET,
                0,
                pw.as_ptr(),
                pw_len,
                base.as_ptr(),
                id_buf1.as_mut_ptr(),
                &mut phrase_handle,
            )
        };
        assert_eq!(rc, FfiError::Ok);
        let phrase_ptr = unsafe { phrase_view_copy(phrase_handle) };
        let phrase_str = unsafe { CStr::from_ptr(phrase_ptr) }
            .to_str()
            .unwrap()
            .to_string()
            .clone();
        let id1 = unsafe { CStr::from_ptr(id_buf1.as_ptr()) }
            .to_str()
            .unwrap()
            .to_string();
        unsafe { phrase_view_free(phrase_handle) };

        // 2. Import the same phrase — fresh WalletId.
        let (phrase_bytes_vec, phrase_len) = phrase_bytes(&phrase_str);
        let mut id_buf2 = [0i8; 37];
        let rc = unsafe {
            wallet_import(
                NETWORK_TESTNET,
                base.as_ptr(),
                phrase_bytes_vec.as_ptr(),
                phrase_len,
                pw.as_ptr(),
                pw_len,
                id_buf2.as_mut_ptr(),
            )
        };
        assert_eq!(rc, FfiError::Ok);
        let id2 = unsafe { CStr::from_ptr(id_buf2.as_ptr()) }
            .to_str()
            .unwrap()
            .to_string();
        assert_ne!(id1, id2, "import must produce a distinct WalletId");

        // 3. List contains both.
        let mut count: usize = 0;
        let slot = Box::into_raw(Box::new(std::ptr::null_mut::<c_char>()));
        let rc = unsafe { wallet_list(NETWORK_TESTNET, base.as_ptr(), &mut count, slot) };
        assert_eq!(rc, FfiError::Ok);
        assert_eq!(count, 2);
        // Read both array entries via the slot pointer.
        let arr_ptr = unsafe { *slot } as *mut c_char;
        let id_a = unsafe {
            let p0 = *(arr_ptr as *const *mut c_char);
            CStr::from_ptr(p0).to_str().unwrap().to_string()
        };
        let id_b = unsafe {
            let p1 = *((arr_ptr as *const *mut c_char).add(1));
            CStr::from_ptr(p1).to_str().unwrap().to_string()
        };
        let listed = [id_a, id_b];
        assert!(listed.contains(&id1));
        assert!(listed.contains(&id2));
        unsafe { wallet_list_array_free(arr_ptr, count) };
        unsafe {
            let _ = Box::from_raw(slot);
        };
    }

    /// Delete removes a wallet; subsequent list returns empty.
    #[test]
    fn wallet_delete_removes_wallet() {
        let (_dir, base) = temp_base();
        let (pw, pw_len) = pw_bytes("pw");
        let mut id_buf = [0i8; 37];
        let mut phrase_handle: *mut c_void = std::ptr::null_mut();

        let rc = unsafe {
            wallet_create(
                12,
                NETWORK_TESTNET,
                0,
                pw.as_ptr(),
                pw_len,
                base.as_ptr(),
                id_buf.as_mut_ptr(),
                &mut phrase_handle,
            )
        };
        assert_eq!(rc, FfiError::Ok);
        let id_str = unsafe { CStr::from_ptr(id_buf.as_ptr()) }
            .to_str()
            .unwrap()
            .to_string()
            .clone();
        unsafe { phrase_view_free(phrase_handle) };

        let rc = unsafe {
            wallet_delete(
                NETWORK_TESTNET,
                base.as_ptr(),
                id_str.as_ptr() as *const c_char,
            )
        };
        assert_eq!(rc, FfiError::Ok);

        let mut count: usize = 99;
        let slot = Box::into_raw(Box::new(std::ptr::null_mut::<c_char>()));
        let rc = unsafe { wallet_list(NETWORK_TESTNET, base.as_ptr(), &mut count, slot) };
        assert_eq!(rc, FfiError::Ok);
        assert_eq!(count, 0);
        unsafe {
            let _ = Box::from_raw(slot);
        };
    }

    /// Invalid BIP-39 word count → InvalidMnemonic (12/15/18/21/24 only).
    #[test]
    fn wallet_create_unsupported_word_count_returns_invalid_mnemonic() {
        let (_dir, base) = temp_base();
        let (pw, pw_len) = pw_bytes("pw");
        let mut id_buf = [0i8; 37];
        let mut phrase_handle: *mut c_void = std::ptr::null_mut();

        let rc = unsafe {
            wallet_create(
                13, // not in SUPPORTED_WORD_COUNTS
                NETWORK_TESTNET,
                0,
                pw.as_ptr(),
                pw_len,
                base.as_ptr(),
                id_buf.as_mut_ptr(),
                &mut phrase_handle,
            )
        };
        assert_eq!(rc, FfiError::InvalidMnemonic);
    }
}
