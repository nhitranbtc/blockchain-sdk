//! FFI surface smoke tests (Task 18 / Issue #224).
//!
//! Regression guard for the C ABI exported by
//! `bitcoin-wallet-core` (cdylib consumed by wallet-desktop via
//! `dart:ffi`). These tests exercise the smallest FFI calls that
//! don't require wallet state, network, or DB — they prove the
//! symbols are exported with the right signatures and don't crash
//! under cross-FFI-call safety.
//!
//! **Why Rust API calls (not `extern "C"` declarations):** the rlib
//! contains the C-ABI symbols in its object files but not in its
//! Rust symbol table, so an `extern "C" { fn ffi_version() }` block
//! in this test fails to link with "undefined symbol". The Rust
//! API (`bitcoin_wallet_core::ffi::*`) calls the same `pub unsafe
//! extern "C" fn` functions through their Rust signatures —
//! signature drift on either side fails to compile.
//!
//! Esplora-needing FFI calls (`wallet_sync`, `wallet_balance`,
//! `wallet_send`) are NOT exercised here — those require a live or
//! mocked Esplora and are L29 operator-driven per the existing
//! convention (live testnet smoke is operator-driven, not CI).
//!
//! If a future refactor renames, removes, or changes the signature
//! of any of these FFI exports, this test fails to compile and
//! forces the `wallet-desktop` Dart-side bindings
//! (`wallet_ops_bindings.dart`, `bdk_extras_bindings.dart`, etc.)
//! to update in lockstep.

use std::ffi::CStr;
use std::ptr;

use bitcoin_wallet_core::ffi::wallet::{ffi_version, ffi_version_free};
use bitcoin_wallet_core::ffi::{runtime_drop, runtime_new, wallet_load, wallet_load_free};

// ---------------------------------------------------------------------------
// ffi_version + ffi_version_free
// ---------------------------------------------------------------------------

/// Smoke: `ffi_version()` returns a non-null, UTF-8 parseable C string
/// that looks like SemVer (e.g. "0.2.0"). Pinned shape, not value.
#[test]
fn ffi_version_returns_non_null_utf8() {
    // SAFETY: FFI call with no preconditions; `ffi_version` allocates
    // a fresh CString via `into_raw()`.
    let raw_ptr = unsafe { ffi_version() };
    assert!(!raw_ptr.is_null(), "ffi_version returned null");

    // SAFETY: `raw_ptr` is a valid CString allocated by the Rust
    // side above. Read it as a borrowed CStr (does not take
    // ownership).
    let version_str = unsafe { CStr::from_ptr(raw_ptr) };
    let version = version_str
        .to_str()
        .expect("ffi_version must return valid UTF-8");

    // Pin the SemVer shape, not the exact version (releases bump it).
    assert!(
        version.split('.').count() >= 2 && version.chars().next().unwrap().is_ascii_digit(),
        "ffi_version must look like SemVer, got: {version:?}"
    );

    // SAFETY: passes the same pointer back to its allocator
    // (`CString::from_raw` in `ffi_version_free`).
    unsafe { ffi_version_free(raw_ptr) };
}

/// Smoke: `ffi_version_free` must accept null without panicking —
/// the Dart side treats null as "no message to free".
#[test]
fn ffi_version_free_accepts_null() {
    // SAFETY: documented contract — null is a no-op.
    unsafe { ffi_version_free(ptr::null_mut()) };
}

// ---------------------------------------------------------------------------
// runtime_new + runtime_drop
// ---------------------------------------------------------------------------

/// Smoke: `runtime_new()` returns a non-null `RuntimeHandle` pointer.
/// The handle is a Box-allocated tokio runtime; if `Box::new` fails
/// (out-of-memory), null is returned and the Dart side surfaces a
/// `FfiError::Unknown` per `runtime_or_unknown`.
#[test]
fn runtime_new_returns_non_null_handle() {
    // SAFETY: FFI call with no preconditions.
    let handle = unsafe { runtime_new() };
    assert!(!handle.is_null(), "runtime_new returned null");

    // SAFETY: handle was allocated by `runtime_new` via
    // `Box::into_raw`. `runtime_drop` reclaims via `Box::from_raw`.
    // Passing it back here is the documented RAII handoff.
    unsafe { runtime_drop(handle) };
}

/// Smoke: `runtime_drop` must accept null without panicking — same
/// null-tolerance contract as `ffi_version_free`.
#[test]
fn runtime_drop_accepts_null() {
    // SAFETY: documented contract — null is a no-op.
    unsafe { runtime_drop(ptr::null_mut()) };
}

/// Smoke: full round-trip — new runtime + immediate drop must not
/// leak (Rust's Box drop runs on `runtime_drop`).
#[test]
fn runtime_new_drop_round_trip() {
    let handle = unsafe { runtime_new() };
    assert!(!handle.is_null());
    unsafe { runtime_drop(handle) };
    // Second drop would be UB — but we only call drop once.
    // This test verifies the documented single-owner semantics.
}

// ---------------------------------------------------------------------------
// wallet_load + wallet_load_free (Task 14 / Issue #220 Sub-split A)
// ---------------------------------------------------------------------------

/// Regression guard (Task 14 #220): `wallet_load_free` must accept
/// null without panicking — same null-tolerance contract as the
/// other `_free` FFI exports.
#[test]
fn wallet_load_free_accepts_null() {
    // SAFETY: documented contract — null is a no-op.
    unsafe { wallet_load_free(std::ptr::null_mut()) };
}

/// Regression guard (Task 14 #220): `wallet_load` returns null
/// (NOT a panic) when given a non-existent base directory. The
/// caller (Dart side) interprets null + `FfiError::Storage` from
/// `ffi_last_error_message` as "wallet file not found at this
/// base_dir" — surfaced to the user as "Cannot unlock wallet".
///
/// **TDD red (2026-08-21):** this test fails to compile until
/// `wallet_load` is exported from the Rust FFI surface (see
/// `bdk_extras.rs`). The export takes `(base_dir: *const c_char,
/// wallet_id: *const c_char, mnemonic: *const c_char,
/// network: u8) -> *mut c_void` and frees via `wallet_load_free`.
#[test]
fn wallet_load_nonexistent_dir_returns_null() {
    use std::ffi::CString;
    let base_dir = CString::new("/nonexistent/wallet/store").unwrap();
    let wallet_id = CString::new("00000000-0000-0000-0000-000000000000").unwrap();
    // Empty mnemonic — caller is expected to validate first. This
    // test exercises the "wallet file missing" branch, not the
    // "bad mnemonic" branch (which is `FfiError::InvalidMnemonic`).
    let mnemonic = CString::new("").unwrap();

    // SAFETY: all CString pointers are valid for the duration of
    // the call. Returns null on failure (no panic).
    let handle = unsafe {
        wallet_load(
            base_dir.as_ptr(),
            wallet_id.as_ptr(),
            mnemonic.as_ptr(),
            0, /* network placeholder */
        )
    };

    // Must not panic; must not leak. Either null (failure path,
    // expected) or non-null (somehow succeeded, must be freed).
    if !handle.is_null() {
        // SAFETY: handle ownership transferred to caller via the
        // successful `wallet_load` return.
        unsafe { wallet_load_free(handle) };
    }
    // Asserting null is the expected outcome — see doc above.
    assert!(
        handle.is_null(),
        "wallet_load with non-existent base_dir must return null"
    );
}
