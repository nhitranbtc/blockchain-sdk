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
use bitcoin_wallet_core::ffi::{runtime_drop, runtime_new};

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
